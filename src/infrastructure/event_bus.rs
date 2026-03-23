//! Synapsis Event Bus - Push Notifications for Multi-Agent

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub enum EventType {
    TaskCreated, TaskCompleted, TaskFailed,
    AgentJoined, AgentLeft,
    ContextUpdated, LockAcquired, LockReleased,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub payload: String,
    pub timestamp: i64,
    pub source_agent: Option<String>,
}

pub struct EventBus {
    clients: Arc<RwLock<HashMap<String, String>>>,
    event_history: Arc<RwLock<Vec<Event>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn subscribe(&self, session_id: &str, _events: Vec<EventType>) {
        let mut clients = self.clients.write().unwrap();
        clients.insert(session_id.to_string(), "subscribed".to_string());
        eprintln!("[EventBus] Client subscribed: {}", &session_id);
    }

    pub fn unsubscribe(&self, session_id: &str) {
        let mut clients = self.clients.write().unwrap();
        clients.remove(session_id);
        eprintln!("[EventBus] Client unsubscribed: {}", session_id);
    }

    pub fn publish(&self, event: Event) {
        let mut history = self.event_history.write().unwrap();
        history.push(event.clone());
        if history.len() > 1000 { history.remove(0); }
        eprintln!("[EventBus] Published: {:?}", event.event_type);
    }

    pub fn get_history(&self, since: i64) -> Vec<Event> {
        let history = self.event_history.read().unwrap();
        history.iter().filter(|e| e.timestamp > since).cloned().collect()
    }
}

impl Default for EventBus {
    fn default() -> Self { Self::new() }
}
