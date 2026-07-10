use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;

use super::Orchestrator;
use super::timestamp_now;
use super::types::*;

impl Orchestrator {
    pub fn send_agent_message(&self, from: &str, to: &str, content: &str) -> String {
        self.send_message(
            from,
            Some(to),
            MessageType::Coordination,
            serde_json::json!({
                "content": content, "type": "direct_message",
            }),
        )
    }

    pub fn broadcast_to_subtree(&self, from_orchestrator: &str, content: &str) -> Vec<String> {
        let subs = self.get_sub_agent_tree(from_orchestrator);
        let ids: Vec<String> = subs.iter().map(|a| a.id.clone()).collect();
        for sub_id in &ids {
            self.send_message(
                from_orchestrator,
                Some(sub_id),
                MessageType::Coordination,
                serde_json::json!({
                    "content": content, "type": "broadcast",
                }),
            );
        }
        ids
    }

    pub fn are_in_same_hierarchy(&self, agent_a: &str, agent_b: &str) -> bool {
        let agents = self.agents.lock_safe();
        let get_root = |id: &str| -> Option<String> {
            let mut current = id.to_string();
            loop {
                let a = agents.get(&current)?;
                match &a.parent_agent {
                    Some(p) => current = p.clone(),
                    None => return Some(current),
                }
            }
        };
        get_root(agent_a) == get_root(agent_b)
    }

    pub fn send_message(
        &self,
        from: &str,
        to: Option<&str>,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) -> String {
        let id = format!("msg-{}", Uuid::new_v4().to_hex_string());
        let msg = OrchestratorMessage {
            id: id.clone(),
            from: from.to_string(),
            to: to.map(String::from),
            message_type: msg_type,
            payload,
            timestamp: timestamp_now(),
        };
        self.messages.lock_safe().push(msg);
        id
    }

    pub fn get_agent_messages(&self, agent_id: &str, since: i64) -> Vec<OrchestratorMessage> {
        self.messages
            .lock_safe()
            .iter()
            .filter(|m| {
                m.timestamp > since && (m.to.as_deref() == Some(agent_id) || m.to.is_none())
            })
            .cloned()
            .collect()
    }

    pub fn log_message(
        &self,
        from: &str,
        to: Option<&str>,
        msg_type: MessageType,
        payload: serde_json::Value,
    ) {
        self.send_message(from, to, msg_type, payload);
    }
}
