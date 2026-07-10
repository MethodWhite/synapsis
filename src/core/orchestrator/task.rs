use crate::core::lock_utils::*;
use crate::core::uuid::Uuid;

use super::Orchestrator;
use super::timestamp_now;
use super::types::*;

impl Orchestrator {
    pub fn create_reviewable_task(
        &self,
        description: &str,
        required_skills: Vec<String>,
        priority: u8,
        parent: Option<&str>,
    ) -> String {
        let id = format!("task-{}", Uuid::new_v4().to_hex_string());
        let now = timestamp_now();
        let task = Task {
            id: id.clone(),
            description: description.to_string(),
            required_skills,
            priority,
            assigned_to: None,
            status: TaskStatus::Pending,
            created_at: now,
            parent_task: parent.map(String::from),
            review_required: true,
            reviewed_by: None,
            review_status: None,
            coordinated: false,
            sync_group: None,
        };
        self.tasks.lock_safe().insert(id.clone(), task);
        id
    }

    pub fn complete_task_for_review(&self, task_id: &str, agent_id: &str) -> bool {
        let mut tasks = self.tasks.lock_safe();
        if let Some(task) = tasks.get_mut(task_id) {
            if !task.review_required {
                task.status = TaskStatus::Completed;
                return true;
            }
            task.status = TaskStatus::AwaitingReview;
            task.review_status = Some(ReviewStatus::Pending);
            drop(tasks);
            self.log_message(
                agent_id,
                None,
                MessageType::ReviewRequest,
                serde_json::json!({
                    "task_id": task_id, "action": "review_required", "by": agent_id,
                }),
            );
            true
        } else {
            false
        }
    }

    pub fn approve_task(&self, task_id: &str, reviewer_id: &str) -> bool {
        let mut tasks = self.tasks.lock_safe();
        if let Some(task) = tasks.get_mut(task_id) {
            if task.review_status != Some(ReviewStatus::Pending) {
                return false;
            }
            task.status = TaskStatus::Completed;
            task.review_status = Some(ReviewStatus::Approved);
            task.reviewed_by = Some(reviewer_id.to_string());
            drop(tasks);
            self.log_message(
                reviewer_id,
                None,
                MessageType::ReviewApprove,
                serde_json::json!({
                    "task_id": task_id, "action": "approved",
                }),
            );
            true
        } else {
            false
        }
    }

    pub fn reject_task(&self, task_id: &str, reviewer_id: &str, reason: &str) -> bool {
        let mut tasks = self.tasks.lock_safe();
        if let Some(task) = tasks.get_mut(task_id) {
            if task.review_status != Some(ReviewStatus::Pending) {
                return false;
            }
            task.status = TaskStatus::Assigned;
            task.review_status = Some(ReviewStatus::Rejected);
            task.reviewed_by = Some(reviewer_id.to_string());
            drop(tasks);
            self.log_message(
                reviewer_id,
                None,
                MessageType::ReviewReject,
                serde_json::json!({
                    "task_id": task_id, "action": "rejected", "reason": reason,
                }),
            );
            true
        } else {
            false
        }
    }

    pub fn create_coordinated_task(
        &self,
        description: &str,
        sync_group: &str,
        priority: u8,
    ) -> String {
        let id = format!("coord-{}", Uuid::new_v4().to_hex_string());
        let now = timestamp_now();
        let task = Task {
            id: id.clone(),
            description: description.to_string(),
            required_skills: vec!["coordinated".into()],
            priority,
            assigned_to: None,
            status: TaskStatus::Pending,
            created_at: now,
            parent_task: None,
            review_required: false,
            reviewed_by: None,
            review_status: None,
            coordinated: true,
            sync_group: Some(sync_group.to_string()),
        };
        self.tasks.lock_safe().insert(id.clone(), task);
        id
    }

    pub fn agent_sync(&self, agent_id: &str, sync_group: &str) -> Vec<String> {
        self.log_message(
            agent_id,
            None,
            MessageType::SyncPoint,
            serde_json::json!({
                "sync_group": sync_group, "action": "synced",
            }),
        );
        let agents = self.agents.lock_safe();
        agents
            .values()
            .filter(|a| {
                a.current_task
                    .as_deref()
                    .map(|t| t.contains(sync_group))
                    .unwrap_or(false)
            })
            .map(|a| a.id.clone())
            .collect()
    }

    pub fn create_task(
        &self,
        description: &str,
        required_skills: Vec<String>,
        priority: u8,
        parent: Option<&str>,
    ) -> String {
        let id = format!("task-{}", Uuid::new_v4().to_hex_string());
        let now = timestamp_now();
        let task = Task {
            id: id.clone(),
            description: description.to_string(),
            required_skills,
            priority,
            assigned_to: None,
            status: TaskStatus::Pending,
            created_at: now,
            parent_task: parent.map(String::from),
            review_required: false,
            reviewed_by: None,
            review_status: None,
            coordinated: false,
            sync_group: None,
        };
        self.tasks.lock_safe().insert(id.clone(), task);
        id
    }

    pub fn list_tasks(&self) -> Vec<(String, String)> {
        let tasks = self.tasks.lock_safe();
        let mut result: Vec<(String, String)> = tasks
            .iter()
            .map(|(id, t)| (id.clone(), format!("{:?}", t.status)))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    pub fn assign_task(&self, task_id: &str, agent_id: &str) -> bool {
        let mut agents = self.agents.lock_safe();
        let mut tasks = self.tasks.lock_safe();

        let agent_type = agents
            .get(agent_id)
            .map(|a| a.agent_type.clone())
            .unwrap_or_default();
        if !self.resource_manager.can_accept_task(&agent_type) {
            self.log_message("resource_manager", Some(agent_id), MessageType::Coordination, serde_json::json!({
                "action": "task_throttled", "task_id": task_id, "agent_id": agent_id, "reason": "system_resources_exceeded"
            }));
            return false;
        }

        let task_desc = if let Some(task) = tasks.get_mut(task_id) {
            task.status = TaskStatus::Assigned;
            task.assigned_to = Some(agent_id.to_string());
            Some(task.description.clone())
        } else {
            None
        };

        if let Some(desc) = task_desc {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.status = AgentStatus::Busy;
                agent.current_task = Some(task_id.to_string());
                agent.workload += 1;
                self.resource_manager
                    .update_agent_task_count(agent_id, agent.workload as usize);
            }
            let priority = tasks.get(task_id).map(|t| t.priority).unwrap_or(0);
            drop(agents);
            drop(tasks);
            self.log_message("orchestrator", Some(agent_id), MessageType::TaskResponse, serde_json::json!({
                "action": "task_assigned", "task_id": task_id, "description": desc, "priority": priority
            }));
            true
        } else {
            false
        }
    }

    pub fn complete_task(&self, task_id: &str, success: bool) {
        // Lock agents first, then tasks to avoid deadlock with assign_task (agents→tasks order)
        let mut agents = self.agents.lock_safe();
        let mut tasks = self.tasks.lock_safe();
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = if success {
                TaskStatus::Completed
            } else {
                TaskStatus::Failed
            };
            if let Some(ref aid) = task.assigned_to {
                if let Some(agent) = agents.get_mut(aid) {
                    agent.status = AgentStatus::Idle;
                    agent.current_task = None;
                    agent.workload = agent.workload.saturating_sub(1);
                    self.resource_manager
                        .update_agent_task_count(aid, agent.workload as usize);
                }
            }
        }
    }

    pub fn delegate_task(&self, task_id: &str, from_agent: &str) -> Option<String> {
        let task = { self.tasks.lock_safe().get(task_id).cloned() }?;
        if let Some(best_agent) = self.find_best_agent(&task.required_skills) {
            if best_agent != from_agent {
                self.assign_task(task_id, &best_agent);
                let context = self.get_agent_context(from_agent);
                self.log_message(
                    from_agent,
                    Some(&best_agent),
                    MessageType::Delegation,
                    serde_json::json!({
                        "task_id": task_id, "description": task.description,
                        "parent_context": context, "delegated_by": from_agent,
                    }),
                );
                return Some(best_agent);
            }
        }
        None
    }

    pub fn delegate_to_sub_orchestrator(
        &self,
        task_id: &str,
        sub_orch_id: &str,
        from_agent: &str,
    ) -> bool {
        let agents = self.agents.lock_safe();
        if !agents
            .get(sub_orch_id)
            .map(|a| a.is_sub_orchestrator)
            .unwrap_or(false)
        {
            return false;
        }
        drop(agents);
        let desc = {
            self.tasks
                .lock_safe()
                .get(task_id)
                .map(|t| t.description.clone())
        };
        if let Some(ref d) = desc {
            self.assign_task(task_id, sub_orch_id);
            self.log_message(from_agent, Some(sub_orch_id), MessageType::Delegation, serde_json::json!({
                "task_id": task_id, "description": d, "type": "sub_orchestrator_delegation", "delegated_by": from_agent,
            }));
            true
        } else {
            false
        }
    }

    pub fn get_pending_tasks(&self) -> Vec<Task> {
        self.tasks
            .lock_safe()
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .cloned()
            .collect()
    }
}
