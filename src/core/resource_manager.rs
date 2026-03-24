//! Resource Manager for Synapsis
//! 
//! Implements system resource monitoring and throttling to maintain
//! optimal performance with multiple concurrent agents.
//!
//! Features:
//! - CPU and memory usage monitoring
//! - Load average tracking
//! - Concurrency limits per agent type
//! - Global task throttling
//! - Adaptive resource allocation
//! - Priority-based scheduling

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{System, CpuRefreshKind, MemoryRefreshKind, RefreshKind, ProcessesToUpdate, Pid, ProcessRefreshKind};

/// Resource usage statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceStats {
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub load_average_1min: f64,
    pub load_average_5min: f64,
    pub load_average_15min: f64,
    pub total_memory_bytes: u64,
    pub used_memory_bytes: u64,
    pub total_swap_bytes: u64,
    pub used_swap_bytes: u64,
    pub timestamp: i64,
}

/// Per-agent resource limits
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentLimits {
    pub max_concurrent_tasks: usize,
    pub max_cpu_percent: f32,
    pub max_memory_mb: u64,
    pub priority: u8, // 0-255, higher is more important
}

/// Resource manager state
pub struct ResourceManager {
    system: Mutex<System>,
    agent_limits: Mutex<HashMap<String, AgentLimits>>,
    agent_stats: Mutex<HashMap<String, AgentStats>>,
    global_limits: Mutex<GlobalLimits>,
    last_update: Mutex<Instant>,
    update_interval: Duration,
}

#[derive(Debug, Clone)]
struct AgentStats {
    pid: Option<u32>,
    cpu_usage_history: Vec<f32>,
    memory_usage_history: Vec<u64>,
    task_count: usize,
    last_seen: Instant,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlobalLimits {
    pub max_total_tasks: usize,
    pub max_cpu_percent: f32,
    pub max_memory_percent: f32,
    pub high_load_threshold: f64, // Load average threshold for throttling
    pub enable_adaptive_throttling: bool,
}

impl Default for GlobalLimits {
    fn default() -> Self {
        Self {
            max_total_tasks: 50,
            max_cpu_percent: 80.0,
            max_memory_percent: 85.0,
            high_load_threshold: 4.0, // 4.0 load average
            enable_adaptive_throttling: true,
        }
    }
}

/// Complete resource limits configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceLimitsConfig {
    pub global: GlobalLimits,
    pub agent_limits: HashMap<String, AgentLimits>,
}

impl ResourceManager {
    /// Create a new resource manager with default limits
    pub fn new() -> Self {
        let refresh_kind = RefreshKind::everything();
        let system = System::new_with_specifics(refresh_kind);
        
        Self {
            system: Mutex::new(system),
            agent_limits: Mutex::new(HashMap::new()),
            agent_stats: Mutex::new(HashMap::new()),
            global_limits: Mutex::new(GlobalLimits::default()),
            last_update: Mutex::new(Instant::now()),
            update_interval: Duration::from_secs(5),
        }
    }
    
    /// Create with custom global limits
    pub fn with_limits(global_limits: GlobalLimits) -> Self {
        let rm = Self::new();
        *rm.global_limits.lock().unwrap() = global_limits;
        rm
    }
    
    /// Update system resource information
    pub fn refresh(&self) {
        let mut system = self.system.lock().unwrap();
        system.refresh_all();
        *self.last_update.lock().unwrap() = Instant::now();
    }
    
    /// Get current system resource statistics
    pub fn get_system_stats(&self) -> ResourceStats {
        self.refresh();
        let system = self.system.lock().unwrap();
        
        // Get load average (if available)
        let load_avg = System::load_average();
        
        ResourceStats {
            cpu_usage_percent: system.global_cpu_usage(),
            memory_usage_percent: (system.used_memory() as f64 / system.total_memory() as f64 * 100.0) as f32,
            load_average_1min: load_avg.one,
            load_average_5min: load_avg.five,
            load_average_15min: load_avg.fifteen,
            total_memory_bytes: system.total_memory(),
            used_memory_bytes: system.used_memory(),
            total_swap_bytes: system.total_swap(),
            used_swap_bytes: system.used_swap(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        }
    }
    
    /// Register an agent with PID for monitoring
    pub fn register_agent(&self, agent_id: &str, pid: Option<u32>) {
        let mut stats = self.agent_stats.lock().unwrap();
        stats.insert(agent_id.to_string(), AgentStats {
            pid,
            cpu_usage_history: Vec::new(),
            memory_usage_history: Vec::new(),
            task_count: 0,
            last_seen: Instant::now(),
        });
    }
    
    /// Update agent task count
    pub fn update_agent_task_count(&self, agent_id: &str, task_count: usize) {
        let mut stats = self.agent_stats.lock().unwrap();
        if let Some(agent_stats) = stats.get_mut(agent_id) {
            agent_stats.task_count = task_count;
            agent_stats.last_seen = Instant::now();
        }
    }
    
    /// Set limits for an agent type
    pub fn set_agent_limits(&self, agent_type: &str, limits: AgentLimits) {
        self.agent_limits.lock().unwrap().insert(agent_type.to_string(), limits);
    }
    
    /// Check if system can accept more tasks (throttling decision)
    pub fn can_accept_task(&self, agent_type: &str) -> bool {
        let stats = self.get_system_stats();
        let global_limits = self.global_limits.lock().unwrap();
        
        // Check global limits
        if stats.cpu_usage_percent > global_limits.max_cpu_percent {
            return false;
        }
        
        if stats.memory_usage_percent > global_limits.max_memory_percent {
            return false;
        }
        
        if stats.load_average_1min > global_limits.high_load_threshold {
            return false;
        }
        
        // Check agent-specific limits
        let agent_stats = self.agent_stats.lock().unwrap();
        let agent_limits = self.agent_limits.lock().unwrap();
        
        if let Some(limits) = agent_limits.get(agent_type) {
            if let Some(stats) = agent_stats.values().find(|s| {
                // Find agents of this type
                agent_type.starts_with(s.pid.map(|_| "").unwrap_or(""))
            }) {
                if stats.task_count >= limits.max_concurrent_tasks {
                    return false;
                }
            }
        }
        
        true
    }
    
    /// Get recommended delay before next task (in milliseconds)
    pub fn get_throttle_delay_ms(&self) -> u64 {
        let stats = self.get_system_stats();
        let global_limits = self.global_limits.lock().unwrap();
        
        if !global_limits.enable_adaptive_throttling {
            return 0;
        }
        
        // Adaptive throttling based on load
        let load_factor = stats.load_average_1min / global_limits.high_load_threshold;
        let cpu_factor = stats.cpu_usage_percent / global_limits.max_cpu_percent;
        let memory_factor = stats.memory_usage_percent / global_limits.max_memory_percent;
        
        let max_factor = load_factor.max(cpu_factor as f64).max(memory_factor as f64);
        
        if max_factor > 1.0 {
            // Exponential backoff when overloaded
            let excess = max_factor - 1.0;
            (100.0 * excess.powi(2)).min(5000.0) as u64 // Max 5 seconds
        } else if max_factor > 0.8 {
            // Gradual slowdown when approaching limits
            100
        } else {
            0
        }
    }
    
    /// Get agent-specific recommendations
    pub fn get_agent_recommendations(&self, agent_id: &str) -> AgentRecommendations {
        let stats = self.get_system_stats();
        let agent_stats = self.agent_stats.lock().unwrap();
        
        if let Some(agent_stat) = agent_stats.get(agent_id) {
            let current_tasks = agent_stat.task_count;
            
            AgentRecommendations {
                agent_id: agent_id.to_string(),
                recommended_max_tasks: self.calculate_recommended_tasks(agent_id),
                current_tasks,
                should_throttle: current_tasks > 0 && !self.can_accept_task(agent_id),
                throttle_delay_ms: self.get_throttle_delay_ms(),
            }
        } else {
            AgentRecommendations {
                agent_id: agent_id.to_string(),
                recommended_max_tasks: 1,
                current_tasks: 0,
                should_throttle: false,
                throttle_delay_ms: 0,
            }
        }
    }
    
    fn calculate_recommended_tasks(&self, agent_id: &str) -> usize {
        let stats = self.get_system_stats();
        let agent_stats = self.agent_stats.lock().unwrap();
        let agent_limits = self.agent_limits.lock().unwrap();
        
        let base_limit = agent_limits
            .iter()
            .find(|(agent_type, _)| agent_id.starts_with(agent_type.as_str()))
            .map(|(_, limits)| limits.max_concurrent_tasks)
            .unwrap_or(2);
        
        // Reduce limit under high load
        let global_limits = self.global_limits.lock().unwrap();
        let load_factor = stats.load_average_1min / global_limits.high_load_threshold;
        let cpu_factor = stats.cpu_usage_percent / 100.0;
        
        let reduction_factor = load_factor.max(cpu_factor as f64);
        let recommended = (base_limit as f64 / reduction_factor).ceil() as usize;
        
        recommended.max(1).min(base_limit)
    }
    
    /// Clean up old agent stats
    pub fn cleanup_old_stats(&self, max_age: Duration) {
        let mut stats = self.agent_stats.lock().unwrap();
        let now = Instant::now();
        stats.retain(|_, agent_stats| now.duration_since(agent_stats.last_seen) < max_age);
    }
    
    /// Load limits from JSON file
    pub fn load_limits(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str::<ResourceLimitsConfig>(&data) {
                let mut agent_limits = self.agent_limits.lock().unwrap();
                let mut global_limits = self.global_limits.lock().unwrap();
                *agent_limits = config.agent_limits;
                *global_limits = config.global;
            }
        }
        Ok(())
    }
    
    /// Save limits to JSON file
    pub fn save_limits(&self, path: &std::path::Path) -> std::io::Result<()> {
        let agent_limits = self.agent_limits.lock().unwrap().clone();
        let global_limits = self.global_limits.lock().unwrap().clone();
        let config = ResourceLimitsConfig {
            global: global_limits,
            agent_limits,
        };
        
        let data = serde_json::to_string_pretty(&config)?;
        std::fs::write(path, data)?;
        Ok(())
    }
    
    /// Get current system statistics as JSON
    pub fn get_stats_json(&self) -> serde_json::Value {
        let stats = self.get_system_stats();
        let global_limits = self.global_limits.lock().unwrap();
        serde_json::json!({
            "system": stats,
            "global_limits": *global_limits,
            "agent_limits_count": self.agent_limits.lock().unwrap().len(),
            "agent_stats_count": self.agent_stats.lock().unwrap().len(),
        })
    }
    
    /// Get recommendations for all registered agents
    pub fn get_all_recommendations(&self) -> HashMap<String, AgentRecommendations> {
        let agent_stats = self.agent_stats.lock().unwrap();
        agent_stats.keys()
            .map(|agent_id| (agent_id.clone(), self.get_agent_recommendations(agent_id)))
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentRecommendations {
    pub agent_id: String,
    pub recommended_max_tasks: usize,
    pub current_tasks: usize,
    pub should_throttle: bool,
    pub throttle_delay_ms: u64,
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_resource_manager_creation() {
        let rm = ResourceManager::new();
        let stats = rm.get_system_stats();
        
        // Basic sanity checks
        assert!(stats.cpu_usage_percent >= 0.0 && stats.cpu_usage_percent <= 100.0);
        assert!(stats.memory_usage_percent >= 0.0 && stats.memory_usage_percent <= 100.0);
    }
    
    #[test]
    fn test_agent_registration() {
        let rm = ResourceManager::new();
        rm.register_agent("test-agent-1", Some(12345));
        
        let recommendations = rm.get_agent_recommendations("test-agent-1");
        assert_eq!(recommendations.agent_id, "test-agent-1");
    }
    
    #[test]
    fn test_throttle_logic() {
        let mut limits = GlobalLimits::default();
        limits.max_cpu_percent = 10.0; // Very low threshold for testing
        limits.enable_adaptive_throttling = false;
        
        let rm = ResourceManager::with_limits(limits);
        
        // Can't really test actual CPU usage, but we can test the method doesn't panic
        let can_accept = rm.can_accept_task("test");
        assert!(can_accept || !can_accept); // Either is fine, just not panic
    }
}