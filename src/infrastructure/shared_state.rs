//! Synapsis Shared State Module
//!
//! Provides shared in-memory state for TCP and MCP servers.
//! Both servers share the same Database, Skills, and AgentRegistry.

use crate::infrastructure::agents::AgentRegistry;
use crate::infrastructure::database::Database;
use crate::infrastructure::skills::SkillRegistry;
use std::sync::Arc;

pub struct SharedState {
    pub db: Arc<Database>,
    pub skills: Arc<SkillRegistry>,
    pub agents: Arc<AgentRegistry>,
}

impl SharedState {
    pub fn new() -> Self {
        let db = Arc::new(Database::new());

        Self {
            db: Arc::clone(&db),
            skills: Arc::new(SkillRegistry::new()),
            agents: Arc::new(AgentRegistry::new()),
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

        skills.init().ok();
        skills.register_default_skills();
        agents.init().ok();

        Self { db, skills, agents }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}
