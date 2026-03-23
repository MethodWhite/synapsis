//! Zero-Trust Security Framework
//! 
//! Implements continuous verification and least-privilege access control
//! through a policy engine that evaluates every request.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Resource types that can be protected
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resource {
    /// Memory observations
    Observation,
    /// Agent sessions
    Session,
    /// Task queue entries
    Task,
    /// Distributed locks
    Lock,
    /// Context chunks
    Chunk,
    /// Global context
    GlobalContext,
    /// Audit logs
    AuditLog,
    /// Any resource (wildcard)
    Any,
}

/// Actions that can be performed on resources
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    Execute,
    /// Any action (wildcard)
    Any,
}

/// Conditions for policy evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conditions {
    /// Required agent type (e.g., "opencode", "qwen-code")
    pub agent_type: Option<String>,
    /// Required project key
    pub project: Option<String>,
    /// Minimum authentication level
    pub min_auth_level: Option<u8>,
    /// Time window restrictions (not yet implemented)
    pub time_window: Option<()>,
}

/// Policy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Unique policy identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Resource this policy applies to
    pub resource: Resource,
    /// Allowed actions
    pub actions: Vec<Action>,
    /// Conditions that must be met
    pub conditions: Conditions,
    /// Whether this policy is enabled
    pub enabled: bool,
}

/// Request context for policy evaluation
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Agent identifier (session ID)
    pub agent_id: String,
    /// Agent type
    pub agent_type: String,
    /// Project key
    pub project: String,
    /// Authentication level (0 = none, 1 = API key, 2 = challenge-response, 3 = PQC signature)
    pub auth_level: u8,
    /// Resource being accessed
    pub resource: Resource,
    /// Action being performed
    pub action: Action,
    /// Additional request parameters (optional)
    pub params: HashMap<String, String>,
}

/// Policy engine that evaluates requests against registered policies
pub struct PolicyEngine {
    policies: Vec<Policy>,
}

#[derive(Error, Debug)]
pub enum PolicyError {
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Policy evaluation error: {0}")]
    EvaluationError(String),
    #[error("Policy not found: {0}")]
    NotFound(String),
}

impl PolicyEngine {
    /// Create a new empty policy engine
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }

    /// Load policies from a JSON configuration file
    pub fn from_file(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let policies: Vec<Policy> = serde_json::from_str(&content)?;
        Ok(Self { policies })
    }

    /// Add a policy to the engine
    pub fn add_policy(&mut self, policy: Policy) {
        self.policies.push(policy);
    }

    /// Evaluate a request against all applicable policies
    pub fn evaluate(&self, ctx: &RequestContext) -> Result<(), PolicyError> {
        let mut allowed = false;
        let mut denial_reasons = Vec::new();

        for policy in &self.policies {
            if !policy.enabled {
                continue;
            }

            // Check resource match
            if !Self::resource_matches(&policy.resource, &ctx.resource) {
                continue;
            }

            // Check action match
            if !Self::action_matches(&policy.actions, &ctx.action) {
                continue;
            }

            // Check conditions
            if let Err(reason) = Self::check_conditions(&policy.conditions, ctx) {
                denial_reasons.push(format!("Policy '{}': {}", policy.id, reason));
                continue;
            }

            // If we reach here, policy allows the request
            allowed = true;
            break;
        }

        if allowed {
            Ok(())
        } else {
            let reason = if denial_reasons.is_empty() {
                "No applicable policy found".to_string()
            } else {
                denial_reasons.join("; ")
            };
            Err(PolicyError::AccessDenied(reason))
        }
    }

    fn resource_matches(policy_resource: &Resource, request_resource: &Resource) -> bool {
        match policy_resource {
            Resource::Any => true,
            _ => policy_resource == request_resource,
        }
    }

    fn action_matches(policy_actions: &[Action], request_action: &Action) -> bool {
        policy_actions.iter().any(|a| match a {
            Action::Any => true,
            _ => a == request_action,
        })
    }

    fn check_conditions(conditions: &Conditions, ctx: &RequestContext) -> Result<(), String> {
        if let Some(ref required_type) = conditions.agent_type {
            if required_type != &ctx.agent_type {
                return Err(format!("agent_type mismatch: expected {}, got {}", required_type, ctx.agent_type));
            }
        }

        if let Some(ref required_project) = conditions.project {
            if required_project != &ctx.project {
                return Err(format!("project mismatch: expected {}, got {}", required_project, ctx.project));
            }
        }

        if let Some(min_level) = conditions.min_auth_level {
            if ctx.auth_level < min_level {
                return Err(format!("insufficient auth level: required {}, got {}", min_level, ctx.auth_level));
            }
        }

        Ok(())
    }

    /// Get all policies (for inspection)
    pub fn policies(&self) -> &[Policy] {
        &self.policies
    }
}

/// Default policies for Synapsis zero-trust framework
pub fn default_policies() -> Vec<Policy> {
    vec![
        Policy {
            id: "allow-auth-challenge".to_string(),
            description: "Allow any agent to initiate authentication challenge".to_string(),
            resource: Resource::Session,
            actions: vec![Action::Create],
            conditions: Conditions {
                agent_type: None,
                project: None,
                min_auth_level: Some(0),
                time_window: None,
            },
            enabled: true,
        },
        Policy {
            id: "allow-session-heartbeat".to_string(),
            description: "Allow authenticated agents to send heartbeats".to_string(),
            resource: Resource::Session,
            actions: vec![Action::Update],
            conditions: Conditions {
                agent_type: None,
                project: None,
                min_auth_level: Some(1),
                time_window: None,
            },
            enabled: true,
        },
        Policy {
            id: "restrict-task-creation".to_string(),
            description: "Only authenticated agents can create tasks".to_string(),
            resource: Resource::Task,
            actions: vec![Action::Create],
            conditions: Conditions {
                agent_type: None,
                project: None,
                min_auth_level: Some(1),
                time_window: None,
            },
            enabled: true,
        },
        Policy {
            id: "restrict-observation-delete".to_string(),
            description: "Only high-auth agents can delete observations".to_string(),
            resource: Resource::Observation,
            actions: vec![Action::Delete],
            conditions: Conditions {
                agent_type: None,
                project: None,
                min_auth_level: Some(2),
                time_window: None,
            },
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_evaluation() {
        let mut engine = PolicyEngine::new();
        engine.add_policy(Policy {
            id: "test".to_string(),
            description: "test".to_string(),
            resource: Resource::Observation,
            actions: vec![Action::Read],
            conditions: Conditions {
                agent_type: Some("opencode".to_string()),
                project: None,
                min_auth_level: Some(1),
                time_window: None,
            },
            enabled: true,
        });

        let ctx = RequestContext {
            agent_id: "session1".to_string(),
            agent_type: "opencode".to_string(),
            project: "test".to_string(),
            auth_level: 1,
            resource: Resource::Observation,
            action: Action::Read,
            params: HashMap::new(),
        };

        assert!(engine.evaluate(&ctx).is_ok());

        let ctx2 = RequestContext {
            agent_type: "qwen".to_string(),
            ..ctx.clone()
        };
        assert!(engine.evaluate(&ctx2).is_err());
    }
}