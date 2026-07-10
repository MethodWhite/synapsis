use crate::core::lock_utils::*;
use std::collections::HashMap;
use std::sync::MutexGuard;

use crate::core::uuid::Uuid;

use super::Orchestrator;
use super::timestamp_now;
use super::types::*;

impl Orchestrator {
    pub fn register_agent(&self, agent_type: &str, skills: Vec<String>) -> String {
        let id = format!("{}-{}", agent_type, Uuid::new_v4().to_hex_string());
        let now = timestamp_now();
        let agent = Agent {
            id: id.clone(),
            agent_type: agent_type.to_string(),
            name: format!("{}_{}", agent_type, &id[..8]),
            skills: skills.clone(),
            status: AgentStatus::Idle,
            current_task: None,
            workload: 0,
            created_at: now,
            last_heartbeat: now,
            parent_agent: None,
            sub_agents: Vec::new(),
            is_sub_orchestrator: false,
        };
        self.agents.lock_safe().insert(id.clone(), agent);
        self.resource_manager.register_agent(&id, None);
        for skill in &skills {
            self.skills_index
                .lock_safe()
                .entry(skill.clone())
                .or_default()
                .push(id.clone());
        }
        self.log_message(
            &id,
            None,
            MessageType::Coordination,
            serde_json::json!({"action": "registered", "skills": skills}),
        );
        id
    }

    pub fn register_agent_with_id(&self, agent_id: &str, agent_type: &str, skills: Vec<String>) {
        let now = timestamp_now();
        let agent = Agent {
            id: agent_id.to_string(),
            agent_type: agent_type.to_string(),
            name: format!("{}_{}", agent_type, &agent_id[..8]),
            skills: skills.clone(),
            status: AgentStatus::Idle,
            current_task: None,
            workload: 0,
            created_at: now,
            last_heartbeat: now,
            parent_agent: None,
            sub_agents: Vec::new(),
            is_sub_orchestrator: false,
        };
        self.agents.lock_safe().insert(agent_id.to_string(), agent);
        for skill in &skills {
            self.skills_index
                .lock_safe()
                .entry(skill.clone())
                .or_default()
                .push(agent_id.to_string());
        }
        self.log_message(
            agent_id,
            None,
            MessageType::Coordination,
            serde_json::json!({"action": "registered", "skills": skills}),
        );
    }

    pub fn register_sub_orchestrator(&self, agent_id: &str, agent_type: &str, skills: Vec<String>) {
        let now = timestamp_now();
        let mut agents = self.agents.lock_safe();
        if agents.contains_key(agent_id) {
            if let Some(a) = agents.get_mut(agent_id) {
                a.is_sub_orchestrator = true;
            }
        } else {
            let agent = Agent {
                id: agent_id.to_string(),
                agent_type: agent_type.to_string(),
                name: format!("{}_suborch_{}", agent_type, &agent_id[..8]),
                skills: skills.clone(),
                status: AgentStatus::Idle,
                current_task: None,
                workload: 0,
                created_at: now,
                last_heartbeat: now,
                parent_agent: None,
                sub_agents: Vec::new(),
                is_sub_orchestrator: true,
            };
            agents.insert(agent_id.to_string(), agent);
        }
        drop(agents);
        for skill in &skills {
            self.skills_index
                .lock_safe()
                .entry(skill.clone())
                .or_default()
                .push(agent_id.to_string());
        }
    }

    pub fn attach_sub_agent(&self, parent_id: &str, sub_id: &str) -> bool {
        let mut agents = self.agents.lock_safe();
        let is_orch = agents
            .get(parent_id)
            .map(|a| a.is_sub_orchestrator)
            .unwrap_or(false);
        if !is_orch {
            return false;
        }
        if !agents.contains_key(sub_id) {
            return false;
        }
        if let Some(parent) = agents.get_mut(parent_id) {
            if parent.sub_agents.contains(&sub_id.to_string()) {
                return true;
            }
            parent.sub_agents.push(sub_id.to_string());
        }
        if let Some(sub) = agents.get_mut(sub_id) {
            sub.parent_agent = Some(parent_id.to_string());
        }
        true
    }

    pub fn get_sub_agent_tree(&self, agent_id: &str) -> Vec<Agent> {
        let agents = self.agents.lock_safe();
        let mut result = Vec::new();
        if let Some(parent) = agents.get(agent_id) {
            for sub_id in &parent.sub_agents {
                if let Some(sub) = agents.get(sub_id) {
                    result.push(sub.clone());
                    if sub.is_sub_orchestrator {
                        let deeper = Self::collect_sub_tree(&agents, sub_id);
                        result.extend(deeper);
                    }
                }
            }
        }
        result
    }

    fn collect_sub_tree(
        agents: &MutexGuard<'_, HashMap<String, Agent>>,
        agent_id: &str,
    ) -> Vec<Agent> {
        let mut result = Vec::new();
        if let Some(parent) = agents.get(agent_id) {
            for sub_id in &parent.sub_agents {
                if let Some(sub) = agents.get(sub_id) {
                    result.push(sub.clone());
                    if sub.is_sub_orchestrator {
                        let deeper = Self::collect_sub_tree(agents, sub_id);
                        result.extend(deeper);
                    }
                }
            }
        }
        result
    }

    pub fn send_cross_message(&self, from: &str, to: &str, content: serde_json::Value) -> String {
        self.send_message(from, Some(to), MessageType::CrossOrchestrator, content)
    }

    pub fn find_agent_in_hierarchy(
        &self,
        skills_needed: &[String],
        from_agent: &str,
    ) -> Option<String> {
        let agents = self.agents.lock_safe();
        let mut candidates: Vec<&Agent> = agents
            .values()
            .filter(|a| {
                (a.status == AgentStatus::Idle || a.status == AgentStatus::Thinking)
                    && a.id != from_agent
            })
            .filter(|a| skills_needed.iter().any(|s| a.skills.contains(s)))
            .collect();
        candidates.sort_by_key(|a| a.workload);
        candidates.first().map(|a| a.id.clone())
    }

    pub fn unregister_agent(&self, agent_id: &str) {
        let mut agents = self.agents.lock_safe();
        if let Some(agent) = agents.remove(agent_id) {
            let mut index = self.skills_index.lock_safe();
            for skill in &agent.skills {
                if let Some(agent_list) = index.get_mut(skill) {
                    agent_list.retain(|a| a != agent_id);
                }
            }
        }
    }

    pub fn heartbeat(&self, agent_id: &str, status: Option<AgentStatus>, task: Option<&str>) {
        let was_idle = {
            let agents = self.agents.lock_safe();
            agents
                .get(agent_id)
                .map(|a| a.status == AgentStatus::Idle)
                .unwrap_or(false)
        };
        let mut agents = self.agents.lock_safe();
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.last_heartbeat = timestamp_now();
            if let Some(s) = status {
                agent.status = s;
            }
            if let Some(t) = task {
                agent.current_task = Some(t.to_string());
            }
        }
        drop(agents);
        if was_idle || status == Some(AgentStatus::Idle) {
            self.proactive_assign_to(agent_id);
        }
    }

    pub fn proactive_assign_to(&self, agent_id: &str) -> Option<Task> {
        let (agent_status, agent_skills) = {
            let agents = self.agents.lock_safe();
            match agents.get(agent_id) {
                Some(a) => (a.status, a.skills.clone()),
                None => return None,
            }
        };
        if agent_status != AgentStatus::Idle {
            return None;
        }
        self.get_pending_tasks().into_iter().find(|task| {
            task.required_skills
                .iter()
                .any(|s| agent_skills.contains(s))
                && self.assign_task(&task.id, agent_id)
        })
    }

    pub fn proactive_assign_all(&self) -> Vec<(String, Task)> {
        let mut assigned = Vec::new();
        for agent in self.get_idle_agents() {
            if let Some(task) = self.proactive_assign_to(&agent.id) {
                assigned.push((agent.id.clone(), task));
            }
        }
        assigned
    }

    pub fn get_agent_task_notification(&self, agent_id: &str) -> Option<serde_json::Value> {
        let messages = self.get_agent_messages(agent_id, 0);
        messages
            .into_iter()
            .find(|m| matches!(m.message_type, MessageType::TaskResponse))
            .map(|m| m.payload)
    }

    pub fn find_best_agent(&self, skills_needed: &[String]) -> Option<String> {
        let agents = self.agents.lock_safe();
        let mut candidates: Vec<&Agent> = agents
            .values()
            .filter(|a| a.status == AgentStatus::Idle || a.status == AgentStatus::Thinking)
            .filter(|a| skills_needed.iter().any(|s| a.skills.contains(s)))
            .collect();
        candidates.sort_by_key(|a| a.workload);
        candidates.first().map(|a| a.id.clone())
    }

    pub fn get_idle_agents(&self) -> Vec<Agent> {
        self.agents
            .lock_safe()
            .values()
            .filter(|a| a.status == AgentStatus::Idle)
            .cloned()
            .collect()
    }

    pub fn get_agent_context(&self, agent_id: &str) -> Vec<serde_json::Value> {
        self.messages
            .lock_safe()
            .iter()
            .filter(|m| m.from == agent_id || m.to.as_deref() == Some(agent_id))
            .rev()
            .take(20)
            .map(|m| {
                serde_json::json!({
                    "from": m.from, "type": format!("{:?}", m.message_type),
                    "summary": format!("{:?}", m.payload).chars().take(200).collect::<String>(),
                })
            })
            .collect()
    }
}
