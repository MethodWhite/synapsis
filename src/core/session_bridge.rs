use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSession {
    pub session_id: String,
    pub agent_id: String,
    pub agent_type: String,
    pub hostname: String,
    pub project: String,
    pub platform: String,
    pub started_at: i64,
    pub last_active_at: i64,
    pub observation_count: u32,
    pub is_active: bool,
}

impl SharedSession {
    pub fn new(
        session_id: &str,
        agent_id: &str,
        agent_type: &str,
        hostname: &str,
        project: &str,
        platform: &str,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            hostname: hostname.to_string(),
            project: project.to_string(),
            platform: platform.to_string(),
            started_at: now,
            last_active_at: now,
            observation_count: 0,
            is_active: true,
        }
    }
}

pub struct SessionBridge {
    active_sessions: RwLock<Vec<SharedSession>>,
}

impl SessionBridge {
    fn new() -> Self {
        Self {
            active_sessions: RwLock::new(Vec::new()),
        }
    }

    pub fn global() -> &'static Self {
        static BRIDGE: OnceLock<SessionBridge> = OnceLock::new();
        BRIDGE.get_or_init(SessionBridge::new)
    }

    pub fn register_session(&self, session: SharedSession) {
        let mut sessions = self.active_sessions.write().unwrap();
        sessions.retain(|s| s.session_id != session.session_id);
        sessions.push(session);
    }

    pub fn unregister_session(&self, session_id: &str) {
        let mut sessions = self.active_sessions.write().unwrap();
        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            session.is_active = false;
        }
        sessions.retain(|s| s.session_id != session_id);
    }

    pub fn get_active_sessions(&self) -> Vec<SharedSession> {
        let sessions = self.active_sessions.read().unwrap();
        sessions.iter().filter(|s| s.is_active).cloned().collect()
    }

    pub fn get_sessions_by_project(&self, project: &str) -> Vec<SharedSession> {
        let sessions = self.active_sessions.read().unwrap();
        sessions
            .iter()
            .filter(|s| s.is_active && s.project == project)
            .cloned()
            .collect()
    }

    pub fn get_sessions_by_platform(&self, platform: &str) -> Vec<SharedSession> {
        let sessions = self.active_sessions.read().unwrap();
        sessions
            .iter()
            .filter(|s| s.is_active && s.platform == platform)
            .cloned()
            .collect()
    }

    pub fn broadcast_observation(&self, session_id: &str, observation: &str) -> Vec<String> {
        let project = {
            let sessions = self.active_sessions.read().unwrap();
            sessions
                .iter()
                .find(|s| s.session_id == session_id)
                .map(|s| s.project.clone())
        };

        let project = match project {
            Some(ref p) => p.clone(),
            None => return Vec::new(),
        };

        let recipients: Vec<String> = {
            let sessions = self.active_sessions.read().unwrap();
            sessions
                .iter()
                .filter(|s| s.is_active && s.project == project && s.session_id != session_id)
                .map(|s| {
                    format!(
                        "[bridge] project:{} session:{} observation:{}",
                        project, s.session_id, observation
                    )
                })
                .collect()
        };

        if !recipients.is_empty() {
            let mut sessions_write = self.active_sessions.write().unwrap();
            for s in sessions_write.iter_mut() {
                if s.is_active && s.project == project {
                    s.observation_count += 1;
                    s.last_active_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                }
            }
        }

        recipients
    }

    pub fn touch_session(&self, session_id: &str) {
        let mut sessions = self.active_sessions.write().unwrap();
        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            session.last_active_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
        }
    }
}

pub fn detect_hostname() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

pub fn detect_platform() -> String {
    if std::env::var("OPENCODE").is_ok() || std::env::var("OPENCODE_API_KEY").is_ok() {
        return "OpenCode".to_string();
    }
    if std::env::var("CURSOR").is_ok() || std::env::var("CURSOR_API_KEY").is_ok() {
        return "Cursor".to_string();
    }
    if std::env::var("GEMINI_API_KEY").is_ok() || std::env::var("GOOGLE_API_KEY").is_ok() {
        return "Gemini-CLI".to_string();
    }
    "Synapsis".to_string()
}
