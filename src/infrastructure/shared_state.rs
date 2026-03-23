//! Synapsis Shared State Module
//!
//! Provides shared in-memory state for TCP and MCP servers.
//! Both servers share the same Database, Skills, and AgentRegistry.

use crate::core::audit_log::AuditLog;
use crate::infrastructure::agents::AgentRegistry;
use crate::infrastructure::database::Database;
use crate::infrastructure::skills::SkillRegistry;
use std::sync::Arc;

pub struct SharedState {
    pub db: Arc<Database>,
    pub skills: Arc<SkillRegistry>,
    pub agents: Arc<AgentRegistry>,
    pub audit_log: Arc<AuditLog>,
}

impl SharedState {
    pub fn new() -> Self {
        let db = Arc::new(Database::new());
        let audit_log = Arc::new(AuditLog::new(Arc::clone(&db)));

        Self {
            db: Arc::clone(&db),
            skills: Arc::new(SkillRegistry::new()),
            agents: Arc::new(AgentRegistry::new()),
            audit_log,
        }
    }

    pub fn init(&self) {
        self.skills.init().ok();
        self.skills.register_default_skills();
        self.agents.init().ok();
    }

    pub fn with_db(db: Arc<Database>) -> Self {
        let skills = Arc::new(SkillRegistry::new());
        let agents = Arc::new(AgentRegistry::new());
        let audit_log = Arc::new(AuditLog::new(Arc::clone(&db)));

        skills.init().ok();
        skills.register_default_skills();
        agents.init().ok();

        Self { db, skills, agents, audit_log }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}
