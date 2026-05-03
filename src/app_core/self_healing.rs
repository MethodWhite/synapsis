//! Self-Healing Module - Automatic Recovery System
//! 
//! Provides automatic detection and recovery from failures

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Health status of a component
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Recovering,
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub component: String,
    pub status: HealthStatus,
    pub last_check: Instant,
    pub error: Option<String>,
    pub recovery_attempts: u32,
}

/// Self-healing manager
pub struct SelfHealingManager {
    components: Arc<RwLock<HashMap<String, HealthCheck>>>,
    max_recovery_attempts: u32,
    recovery_delay: Duration,
}

impl SelfHealingManager {
    pub fn new() -> Self {
        Self {
            components: Arc::new(RwLock::new(HashMap::new())),
            max_recovery_attempts: 3,
            recovery_delay: Duration::from_secs(5),
        }
    }
    
    /// Register a component for health monitoring
    pub fn register_component(&self, name: &str) {
        let mut components = self.components.write().unwrap();
        components.insert(name.to_string(), HealthCheck {
            component: name.to_string(),
            status: HealthStatus::Healthy,
            last_check: Instant::now(),
            error: None,
            recovery_attempts: 0,
        });
    }
    
    /// Report health status of a component
    pub fn report_health(&self, name: &str, healthy: bool, error: Option<String>) {
        let mut components = self.components.write().unwrap();
        
        if let Some(check) = components.get_mut(name) {
            check.last_check = Instant::now();
            
            if healthy {
                check.status = HealthStatus::Healthy;
                check.error = None;
                check.recovery_attempts = 0;
            } else {
                check.status = HealthStatus::Unhealthy;
                check.error = error;
                check.recovery_attempts += 1;
                
                if check.recovery_attempts >= self.max_recovery_attempts {
                    check.status = HealthStatus::Degraded;
                }
            }
        }
    }
    
    /// Attempt automatic recovery
    pub fn attempt_recovery<F>(&self, name: &str, recover_fn: F) -> Result<bool, String>
    where
        F: Fn() -> Result<(), String>
    {
        let mut components = self.components.write().unwrap();
        
        if let Some(check) = components.get_mut(name) {
            if check.status == HealthStatus::Unhealthy {
                check.status = HealthStatus::Recovering;
                
                match recover_fn() {
                    Ok(_) => {
                        check.status = HealthStatus::Healthy;
                        check.error = None;
                        check.recovery_attempts = 0;
                        Ok(true)
                    }
                    Err(e) => {
                        check.status = HealthStatus::Unhealthy;
                        check.error = Some(e);
                        Ok(false)
                    }
                }
            } else {
                Ok(check.status == HealthStatus::Healthy)
            }
        } else {
            Err(format!("Component {} not found", name))
        }
    }
    
    /// Get health status of all components
    pub fn get_health_status(&self) -> Vec<HealthCheck> {
        let components = self.components.read().unwrap();
        components.values().cloned().collect()
    }
    
    /// Check if system is overall healthy
    pub fn is_healthy(&self) -> bool {
        let components = self.components.read().unwrap();
        components.values().all(|c| c.status == HealthStatus::Healthy)
    }
    
    /// Get components needing recovery
    pub fn get_unhealthy_components(&self) -> Vec<HealthCheck> {
        let components = self.components.read().unwrap();
        components.values()
            .filter(|c| c.status != HealthStatus::Healthy)
            .cloned()
            .collect()
    }
}

impl Default for SelfHealingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_healing_manager_creation() {
        let manager = SelfHealingManager::new();
        assert!(manager.is_healthy(), "New manager should be healthy");
    }

    #[test]
    fn test_register_component() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        
        let status = manager.get_health_status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].status, HealthStatus::Healthy);
    }

    #[test]
    fn test_report_unhealthy() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        
        manager.report_health("database", false, Some("Connection failed"));
        
        let status = manager.get_health_status();
        assert_eq!(status[0].status, HealthStatus::Unhealthy);
        assert!(status[0].error.is_some());
    }

    #[test]
    fn test_automatic_recovery() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        
        // Make it unhealthy
        manager.report_health("database", false, Some("Connection failed"));
        
        // Attempt recovery
        let result = manager.attempt_recovery("database", || Ok(()));
        
        assert!(result.is_ok());
        assert!(manager.is_healthy());
    }

    #[test]
    fn test_recovery_failure() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        
        // Make it unhealthy
        manager.report_health("database", false, Some("Connection failed"));
        
        // Attempt recovery that fails
        let result = manager.attempt_recovery("database", || Err("Still failing".to_string()));
        
        assert!(result.is_ok());
        assert!(!manager.is_healthy());
    }

    #[test]
    fn test_max_recovery_attempts() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        
        // Report unhealthy multiple times
        for _ in 0..5 {
            manager.report_health("database", false, Some("Error"));
        }
        
        let status = manager.get_health_status();
        assert_eq!(status[0].status, HealthStatus::Degraded);
    }

    #[test]
    fn test_multiple_components() {
        let manager = SelfHealingManager::new();
        manager.register_component("database");
        manager.register_component("cache");
        manager.register_component("network");
        
        // Make one unhealthy
        manager.report_health("cache", false, Some("Cache miss"));
        
        assert!(!manager.is_healthy());
        
        let unhealthy = manager.get_unhealthy_components();
        assert_eq!(unhealthy.len(), 1);
        assert_eq!(unhealthy[0].component, "cache");
    }
}
