//! Port and PID Protection Module
//!
//! Intelligent port moving and process protection to prevent interception.
//! Can change ports and process identifiers dynamically without killing the server.
//!
//! Features:
//! - Port selection avoids well-known and registered ports (0-49151)
//! - Prefers dynamic/private ports (49152-65535)
//! - Checks port availability before binding
//! - Avoids ports used by legitimate user services (CLI/TUI/IDE)
//! - Process name uniqueness verification
//! - Graceful port migration when conflicts detected

use std::net::{TcpListener, TcpStream, SocketAddr};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::process;
use std::io::{self, Read, Write};
use std::fs;

use ctrlc;

// Port ranges according to IANA
const WELL_KNOWN_PORTS: std::ops::Range<u16> = 0..1024;
const REGISTERED_PORTS: std::ops::Range<u16> = 1024..49152;
const DYNAMIC_PORTS: std::ops::Range<u16> = 49152..65536;

// Protected ports that should never be used (system and Synapsis defaults)
const PROTECTED_PORTS: &[u16] = &[
    22,    // SSH
    80,    // HTTP
    443,   // HTTPS
    3306,  // MySQL
    5432,  // PostgreSQL
    6379,  // Redis
    8080,  // HTTP alternative
    8443,  // HTTPS alternative
    7438,  // Synapsis default
    7439,  // Synapsis alternative
    7440,  // Synapsis HTTP
];

/// Check if a port is available for binding
fn is_port_available(port: u16) -> bool {
    match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Check if a port is in the protected list or well-known range
fn is_port_protected(port: u16) -> bool {
    PROTECTED_PORTS.contains(&port) || WELL_KNOWN_PORTS.contains(&port)
}

/// Get currently used ports by scanning /proc/net/tcp (Linux specific)
#[cfg(target_os = "linux")]
fn get_used_ports() -> HashSet<u16> {
    let mut used_ports = HashSet::new();
    
    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        for line in content.lines().skip(1) { // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Some(addr_part) = parts.get(1) {
                    // Format: local_address:port in hex
                    if let Some(port_hex) = addr_part.split(':').nth(1) {
                        if let Ok(port) = u16::from_str_radix(port_hex, 16) {
                            used_ports.insert(port);
                        }
                    }
                }
            }
        }
    }
    
    used_ports
}

#[cfg(not(target_os = "linux"))]
fn get_used_ports() -> HashSet<u16> {
    // Fallback: try to bind to ports to check availability
    // This is expensive, so we return empty set and rely on is_port_available
    HashSet::new()
}

/// Get existing process names (Linux specific)
#[cfg(target_os = "linux")]
fn get_existing_process_names() -> HashSet<String> {
    let mut names = HashSet::new();
    
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let pid = entry.file_name();
            let pid_str = pid.to_string_lossy();
            
            // Check if it's a numeric PID directory
            if pid_str.chars().all(|c| c.is_ascii_digit()) {
                let comm_path = entry.path().join("comm");
                if let Ok(content) = fs::read_to_string(comm_path) {
                    let name = content.trim().to_string();
                    if !name.is_empty() {
                        names.insert(name);
                    }
                }
            }
        }
    }
    
    names
}

#[cfg(not(target_os = "linux"))]
fn get_existing_process_names() -> HashSet<String> {
    // Fallback: return empty set
    HashSet::new()
}



/// Server protection manager
pub struct ServerProtection {
    current_port: Arc<Mutex<u16>>,
    listener: Arc<Mutex<Option<TcpListener>>>,
    is_running: Arc<AtomicBool>,
    connections: Arc<Mutex<HashMap<u64, TcpStream>>>,
    next_connection_id: Arc<Mutex<u64>>,
    security_config: SecurityConfig,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub min_port: u16,
    pub max_port: u16,
    pub port_change_interval: Duration,
    pub max_connections_per_port: u32,
    pub enable_port_hopping: bool,
    pub enable_pid_protection: bool,
    pub stealth_mode: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            min_port: 49152, // Dynamic/private ports start
            max_port: 65535,
            port_change_interval: Duration::from_secs(300), // 5 minutes
            max_connections_per_port: 100,
            enable_port_hopping: true,
            enable_pid_protection: false, // Requires special handling
            stealth_mode: false,
        }
    }
}

/// Process identity for protection
#[derive(Debug, Clone)]
pub struct ProcessIdentity {
    pub original_pid: u32,
    pub current_pid: u32,
    pub session_id: String,
    pub process_name: String,
    pub protection_level: ProtectionLevel,
}

/// Protection level
#[derive(Debug, Clone, PartialEq)]
pub enum ProtectionLevel {
    None,
    Basic,      // Simple port changes
    Advanced,   // Port hopping + connection migration
    Stealth,    // Full stealth mode
}

impl ServerProtection {
    /// Create a new protection manager
    pub fn new(initial_port: u16) -> Self {
        Self {
            current_port: Arc::new(Mutex::new(initial_port)),
            listener: Arc::new(Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_connection_id: Arc::new(Mutex::new(0)),
            security_config: SecurityConfig::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(initial_port: u16, config: SecurityConfig) -> Self {
        Self {
            current_port: Arc::new(Mutex::new(initial_port)),
            listener: Arc::new(Mutex::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_connection_id: Arc::new(Mutex::new(0)),
            security_config: config,
        }
    }
    
    /// Start the protected server
    pub fn start(&mut self) -> io::Result<()> {
        self.is_running.store(true, Ordering::SeqCst);
        
        // Initial listener
        let port = *self.current_port.lock().unwrap();
        self.start_listener(port)?;
        
        // Start protection threads
        self.start_protection_threads();
        
        Ok(())
    }
    
    /// Start listener on specified port
    fn start_listener(&self, port: u16) -> io::Result<()> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = TcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        
        println!("[PROTECTION] Listening on port {}", port);
        
        let mut listener_guard = self.listener.lock().unwrap();
        *listener_guard = Some(listener);
        
        Ok(())
    }
    
    /// Start protection threads (port hopping, monitoring, etc.)
    fn start_protection_threads(&self) {
        let is_running = Arc::clone(&self.is_running);
        let current_port = Arc::clone(&self.current_port);
        let listener = Arc::clone(&self.listener);
        let connections = Arc::clone(&self.connections);
        let config = self.security_config.clone();
        
        // Port hopping thread
        if config.enable_port_hopping {
            let hopping_thread = thread::spawn(move || {
                let mut last_change = Instant::now();
                
                while is_running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_secs(1));
                    
                    if last_change.elapsed() >= config.port_change_interval {
                        if let Err(e) = Self::change_port_internal(
                            &current_port,
                            &listener,
                            &connections,
                            &config,
                        ) {
                            eprintln!("[PROTECTION] Failed to change port: {}", e);
                        } else {
                            last_change = Instant::now();
                        }
                    }
                }
            });
            
            // Detach thread
            drop(hopping_thread);
        }
        
        // Connection monitoring thread
        let monitor_thread = thread::spawn(move || {
            while is_running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_secs(30));
                
                // Check for suspicious connection patterns
                let connections_guard = connections.lock().unwrap();
                if connections_guard.len() as u32 > config.max_connections_per_port {
                    println!("[PROTECTION] High connection count: {}", connections_guard.len());
                    // Could trigger port change
                }
            }
        });
        
        drop(monitor_thread);
    }
    
    /// Internal port change logic
    fn change_port_internal(
        current_port: &Arc<Mutex<u16>>,
        listener: &Arc<Mutex<Option<TcpListener>>>,
        connections: &Arc<Mutex<HashMap<u64, TcpStream>>>,
        config: &SecurityConfig,
    ) -> io::Result<()> {
        // 1. Select new available port
        let Some(new_port) = Self::select_available_port(config) else {
            eprintln!("[PROTECTION] No available ports found, keeping current port");
            return Ok(());
        };
        
        // 2. Create new listener on new port
        let new_addr = SocketAddr::from(([127, 0, 0, 1], new_port));
        let new_listener = TcpListener::bind(new_addr)?;
        new_listener.set_nonblocking(true)?;
        
        println!("[PROTECTION] Port change: {} -> {}", *current_port.lock().unwrap(), new_port);
        
        // 3. Update listener
        let mut listener_guard = listener.lock().unwrap();
        *listener_guard = Some(new_listener);
        
        // 4. Update current port
        let mut port_guard = current_port.lock().unwrap();
        *port_guard = new_port;
        
        // 5. Notify existing connections (optional - can migrate or keep)
        if config.stealth_mode {
            // In stealth mode, we might want to migrate connections
            Self::notify_connections_port_change(connections, new_port);
        }
        
        Ok(())
    }
    
    /// Select a random port within range
    fn select_random_port(min: u16, max: u16) -> u16 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(min..=max)
    }
    
    /// Select an available port that's not protected or in use
    fn select_available_port(config: &SecurityConfig) -> Option<u16> {
        let used_ports = get_used_ports();
        
        // Try dynamic ports first (49152-65535)
        let mut attempts = 0;
        let max_attempts = 100;
        
        while attempts < max_attempts {
            let candidate = Self::select_random_port(config.min_port, config.max_port);
            
            // Prefer dynamic ports
            if !DYNAMIC_PORTS.contains(&candidate) && attempts < max_attempts / 2 {
                attempts += 1;
                continue;
            }
            
            // Skip protected ports
            if is_port_protected(candidate) {
                attempts += 1;
                continue;
            }
            
            // Skip ports already in use
            if used_ports.contains(&candidate) || !is_port_available(candidate) {
                attempts += 1;
                continue;
            }
            
            return Some(candidate);
        }
        
        // Fallback: any available port in range
        for port in config.min_port..=config.max_port {
            if !is_port_protected(port) && is_port_available(port) {
                return Some(port);
            }
        }
        
        None
    }
    
    /// Notify connections about port change
    fn notify_connections_port_change(
        connections: &Arc<Mutex<HashMap<u64, TcpStream>>>,
        new_port: u16,
    ) {
        let mut connections_guard = connections.lock().unwrap();
        
        for (id, stream) in connections_guard.iter_mut() {
            // Send notification to client about port change
            let notification = format!(
                "{{\"type\":\"port_change\",\"new_port\":{},\"timestamp\":{}}}\n",
                new_port,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            
            let _ = stream.write_all(notification.as_bytes());
        }
    }
    
    /// Accept new connections (to be called from main loop)
    pub fn accept_connections<F>(&self, handler: F) -> io::Result<()>
    where
        F: Fn(TcpStream) + Send + 'static,
    {
        let listener_guard = self.listener.lock().unwrap();
        
        if let Some(ref listener) = *listener_guard {
            // Try to accept connections (non-blocking)
            match listener.accept() {
                Ok((stream, addr)) => {
                    println!("[PROTECTION] New connection from {}", addr);
                    
                    // Register connection
                    let mut id_guard = self.next_connection_id.lock().unwrap();
                    let connection_id = *id_guard;
                    *id_guard += 1;
                    
                    let mut connections_guard = self.connections.lock().unwrap();
                    connections_guard.insert(connection_id, stream.try_clone()?);
                    
                    // Spawn handler thread
                    let handler_stream = stream.try_clone()?;
                    thread::spawn(move || {
                        handler(handler_stream);
                    });
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // No connections available, this is normal for non-blocking
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    /// Change port immediately (manual trigger)
    pub fn change_port(&self) -> io::Result<u16> {
        let current_port = Arc::clone(&self.current_port);
        let listener = Arc::clone(&self.listener);
        let connections = Arc::clone(&self.connections);
        
        let Some(new_port) = Self::select_available_port(&self.security_config) else {
            return Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "No available ports found",
            ));
        };
        
        let new_addr = SocketAddr::from(([127, 0, 0, 1], new_port));
        let new_listener = TcpListener::bind(new_addr)?;
        new_listener.set_nonblocking(true)?;
        
        println!("[PROTECTION] Manual port change: {} -> {}", 
                 *current_port.lock().unwrap(), new_port);
        
        // Update listener
        let mut listener_guard = listener.lock().unwrap();
        *listener_guard = Some(new_listener);
        
        // Update current port
        let mut port_guard = current_port.lock().unwrap();
        let old_port = *port_guard;
        *port_guard = new_port;
        
        Ok(new_port)
    }
    
    /// Get current port
    pub fn current_port(&self) -> u16 {
        *self.current_port.lock().unwrap()
    }
    
    /// Stop protection and cleanup
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        
        // Close all connections
        let mut connections_guard = self.connections.lock().unwrap();
        connections_guard.clear();
        
        println!("[PROTECTION] Stopped");
    }
}

/// Process identity manager for PID protection
#[derive(Clone)]
pub struct ProcessIdentityManager {
    original_pid: u32,
    original_process_name: String,
    current_identity: Arc<Mutex<ProcessIdentity>>,
    protection_enabled: Arc<AtomicBool>,
}

impl ProcessIdentityManager {
    /// Create new identity manager
    pub fn new(process_name: &str) -> Self {
        let pid = process::id();
        
        let identity = ProcessIdentity {
            original_pid: pid,
            current_pid: pid,
            session_id: Self::generate_session_id(),
            process_name: process_name.to_string(),
            protection_level: ProtectionLevel::None,
        };
        
        Self {
            original_pid: pid,
            original_process_name: process_name.to_string(),
            current_identity: Arc::new(Mutex::new(identity)),
            protection_enabled: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Generate random session ID
    fn generate_session_id() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        format!("{:x}", rng.gen::<u64>())
    }
    
    /// Generate a unique process name that doesn't conflict with existing processes
    fn generate_unique_process_name(base: &str) -> String {
        let existing_names = get_existing_process_names();
        let mut candidate = base.to_string();
        let mut suffix = 1;
        
        // Try base name first
        if !existing_names.contains(&candidate) {
            return candidate;
        }
        
        // Append numeric suffix until we find a unique name
        while suffix < 1000 {
            candidate = format!("{}-{}", base, suffix);
            if !existing_names.contains(&candidate) {
                return candidate;
            }
            suffix += 1;
        }
        
        // Fallback: add random suffix
        use rand::Rng;
        let mut rng = rand::thread_rng();
        candidate = format!("{}-{:x}", base, rng.gen::<u32>());
        candidate
    }
    
    /// Enable PID protection (simulated - actual PID cannot be changed)
    pub fn enable_pid_protection(&self) -> io::Result<()> {
        // On Unix-like systems, we can use fork() to create a new process
        // but this is complex and requires careful state transfer.
        
        // For now, we simulate PID protection by changing how the process
        // appears in listings (process name, session ID, etc.)
        
        let mut identity_guard = self.current_identity.lock().unwrap();
        identity_guard.protection_level = ProtectionLevel::Advanced;
        
        // Generate unique process name to avoid conflicts
        let base_name = format!("[{}]", identity_guard.session_id);
        let unique_name = Self::generate_unique_process_name(&base_name);
        
        // Change process name if possible
        Self::change_process_name(&unique_name)?;
        
        // Update process name in identity
        identity_guard.process_name = unique_name.clone();
        
        self.protection_enabled.store(true, Ordering::SeqCst);
        
        println!("[PROTECTION] PID protection enabled (session: {}, process: {})", 
                 identity_guard.session_id, unique_name);
        
        Ok(())
    }
    
    /// Change process name (platform-specific)
    fn change_process_name(name: &str) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            // On Linux, we can set process name via prctl
            use std::ffi::CString;
            use libc::{c_char, prctl, PR_SET_NAME};
            
            let c_name = CString::new(name).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidInput, e)
            })?;
            
            unsafe {
                prctl(PR_SET_NAME, c_name.as_ptr() as u64, 0, 0, 0);
            }
            
            Ok(())
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // Not implemented for other platforms
            println!("[PROTECTION] Process name change not supported on this platform");
            Ok(())
        }
    }
    
    /// Rotate session ID (simulates PID change)
    pub fn rotate_session(&self) -> io::Result<String> {
        let mut identity_guard = self.current_identity.lock().unwrap();
        
        let new_session_id = Self::generate_session_id();
        identity_guard.session_id = new_session_id.clone();
        
        // Generate unique process name to avoid conflicts
        let base_name = format!("[{}]", new_session_id);
        let unique_name = Self::generate_unique_process_name(&base_name);
        
        // Update process name with new session ID
        Self::change_process_name(&unique_name)?;
        
        // Update process name in identity
        identity_guard.process_name = unique_name.clone();
        
        println!("[PROTECTION] Session rotated to: {} (process: {})", 
                 new_session_id, unique_name);
        
        Ok(new_session_id)
    }
    
    /// Get current identity
    pub fn get_identity(&self) -> ProcessIdentity {
        self.current_identity.lock().unwrap().clone()
    }
    
    /// Disable protection
    pub fn disable_protection(&self) {
        self.protection_enabled.store(false, Ordering::SeqCst);
        
        let mut identity_guard = self.current_identity.lock().unwrap();
        identity_guard.protection_level = ProtectionLevel::None;
        
        // Restore original process name
        let _ = Self::change_process_name(&identity_guard.process_name);
        
        println!("[PROTECTION] Protection disabled");
    }
}

/// Integrated server with full protection
#[derive(Clone)]
pub struct ProtectedServer {
    protection: ServerProtection,
    identity: ProcessIdentityManager,
    config: SecurityConfig,
}

impl ProtectedServer {
    /// Create new protected server
    pub fn new(initial_port: u16, process_name: &str) -> Self {
        let config = SecurityConfig::default();
        
        Self {
            protection: ServerProtection::with_config(initial_port, config.clone()),
            identity: ProcessIdentityManager::new(process_name),
            config,
        }
    }
    
    /// Start with full protection
    pub fn start_with_protection<F>(&mut self, connection_handler: F) -> io::Result<()>
    where
        F: Fn(TcpStream) + Send + Clone + 'static,
    {
        println!("[PROTECTION] Starting protected server...");
        
        // Enable PID protection if configured
        if self.config.enable_pid_protection {
            self.identity.enable_pid_protection()?;
        }
        
        // Start port protection
        self.protection.start()?;
        
        // Main server loop
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);
        
        let handler = connection_handler.clone();
        let protection = self.protection.clone();
        
        thread::spawn(move || {
            while is_running_clone.load(Ordering::SeqCst) {
                // Accept connections
                if let Err(e) = protection.accept_connections(&handler) {
                    eprintln!("[PROTECTION] Error accepting connection: {}", e);
                }
                
                // Small sleep to prevent busy loop
                thread::sleep(Duration::from_millis(10));
            }
        });
        
        // Wait for signal
        ctrlc::set_handler(move || {
            println!("\n[PROTECTION] Shutting down...");
            is_running.store(false, Ordering::SeqCst);
        }).expect("Error setting Ctrl-C handler");
        
        // Keep main thread alive
        while is_running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
        }
        
        self.protection.stop();
        self.identity.disable_protection();
        
        Ok(())
    }
    
    /// Get protection instance
    pub fn protection(&self) -> &ServerProtection {
        &self.protection
    }
    
    /// Get identity manager
    pub fn identity(&self) -> &ProcessIdentityManager {
        &self.identity
    }
    

}

impl Clone for ServerProtection {
    fn clone(&self) -> Self {
        Self {
            current_port: Arc::clone(&self.current_port),
            listener: Arc::clone(&self.listener),
            is_running: Arc::clone(&self.is_running),
            connections: Arc::clone(&self.connections),
            next_connection_id: Arc::clone(&self.next_connection_id),
            security_config: self.security_config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_server_protection_creation() {
        let protection = ServerProtection::new(8080);
        assert_eq!(protection.current_port(), 8080);
    }
    
    #[test]
    fn test_process_identity_creation() {
        let identity = ProcessIdentityManager::new("test-server");
        let ident = identity.get_identity();
        
        assert_eq!(ident.original_pid, ident.current_pid);
        assert!(!ident.session_id.is_empty());
        assert_eq!(ident.process_name, "test-server");
        assert_eq!(ident.protection_level, ProtectionLevel::None);
    }
    
    #[test]
    fn test_port_selection() {
        let port = ServerProtection::select_random_port(10000, 10100);
        assert!(port >= 10000 && port <= 10100);
    }
    
    #[test]
    fn test_security_config_default() {
        let config = SecurityConfig::default();
        assert_eq!(config.min_port, 1024);
        assert_eq!(config.max_port, 65535);
        assert_eq!(config.port_change_interval, Duration::from_secs(300));
        assert_eq!(config.max_connections_per_port, 100);
        assert!(config.enable_port_hopping);
        assert!(!config.enable_pid_protection);
        assert!(!config.stealth_mode);
    }
}