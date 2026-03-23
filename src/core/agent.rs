//! Agent Identity

use crate::core::uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentId {
    pub id: String,
    pub agent_type: String,
    pub session_id: String,
}

impl AgentId {
    pub fn new(agent_type: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_hex_string(),
            agent_type: agent_type.to_string(),
            session_id: Uuid::new_v4().to_hex_string(),
        }
    }
    pub fn from_env() -> Self {
        let agent_type =
            std::env::var("SYNAPSIS_AGENT_TYPE").unwrap_or_else(|_| "unknown".to_string());
        Self::new(&agent_type)
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new("default")
    }
}
