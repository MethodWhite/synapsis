//! Agents Registry Tests for Synapsis
//!
//! Unit tests for the AgentRegistry covering agent registration,
//! message passing, task management, and multi-agent coordination.

use std::env;
use synapsis::infrastructure::agents::{
    Agent, AgentId, AgentMessage, AgentRegistry, AgentRole, AgentState, MessageType, Task, TaskId,
    TaskPriority, TaskStatus,
};

fn test_registry() -> AgentRegistry {
    env::set_var("XDG_DATA_HOME", "/tmp/synapsis-agents-test");
    std::fs::create_dir_all("/tmp/synapsis-agents-test/synapsis/agents").ok();
    let registry = AgentRegistry::new();
    registry.init().ok();
    registry
}

fn cleanup_test_dir() {
    std::fs::remove_dir_all("/tmp/synapsis-agents-test").ok();
}

mod agents_tests {
    use super::*;

    #[test]
    fn test_register_agent() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new(
            "TestAgent".to_string(),
            AgentRole::Coder,
            "A test agent".to_string(),
        );

        let id = registry.register(agent);
        assert!(!id.0.is_empty(), "Agent ID should not be empty");

        let retrieved = registry.get(&id);
        assert!(retrieved.is_some(), "Agent should be retrievable");
        assert_eq!(retrieved.unwrap().name, "TestAgent");

        cleanup_test_dir();
    }

    #[test]
    fn test_register_multiple_agents() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Agent::new(
            "Agent1".to_string(),
            AgentRole::Coder,
            "Coder agent".to_string(),
        ));
        registry.register(Agent::new(
            "Agent2".to_string(),
            AgentRole::Reviewer,
            "Reviewer agent".to_string(),
        ));
        registry.register(Agent::new(
            "Agent3".to_string(),
            AgentRole::Orchestrator,
            "Orchestrator agent".to_string(),
        ));

        let count = registry.count();
        assert_eq!(count, 3, "Should have 3 registered agents");

        cleanup_test_dir();
    }

    #[test]
    fn test_agent_roles() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Agent::new(
            "a1".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        registry.register(Agent::new(
            "a2".to_string(),
            AgentRole::Researcher,
            "d".to_string(),
        ));
        registry.register(Agent::new(
            "a3".to_string(),
            AgentRole::Security,
            "d".to_string(),
        ));
        registry.register(Agent::new(
            "a4".to_string(),
            AgentRole::DevOps,
            "d".to_string(),
        ));

        let coders = registry.list(Some(AgentRole::Coder));
        let researchers = registry.list(Some(AgentRole::Researcher));

        assert_eq!(coders.len(), 1, "Should have 1 coder");
        assert_eq!(researchers.len(), 1, "Should have 1 researcher");

        cleanup_test_dir();
    }

    #[test]
    fn test_agent_from_str() {
        assert_eq!(AgentRole::from_str("coder"), AgentRole::Coder);
        assert_eq!(AgentRole::from_str("orchestrator"), AgentRole::Orchestrator);
        assert_eq!(AgentRole::from_str("reviewer"), AgentRole::Reviewer);
        assert_eq!(AgentRole::from_str("unknown"), AgentRole::General);
        assert_eq!(AgentRole::from_str("RUST"), AgentRole::General);
    }

    #[test]
    fn test_agent_update_state() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("StateAgent".to_string(), AgentRole::Coder, "d".to_string());
        let id = registry.register(agent);

        assert!(
            registry.update_state(&id, AgentState::Working),
            "Should update state"
        );

        let retrieved = registry.get(&id).unwrap();
        assert_eq!(retrieved.state, AgentState::Working);

        cleanup_test_dir();
    }

    #[test]
    fn test_agent_state_transitions() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("StateTest".to_string(), AgentRole::Coder, "d".to_string());
        let id = registry.register(agent);

        registry.update_state(&id, AgentState::Working);
        registry.update_state(&id, AgentState::Waiting);
        registry.update_state(&id, AgentState::Idle);

        let retrieved = registry.get(&id).unwrap();
        assert_eq!(retrieved.state, AgentState::Idle);

        cleanup_test_dir();
    }

    #[test]
    fn test_send_message_between_agents() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent1 = Agent::new(
            "Sender".to_string(),
            AgentRole::Orchestrator,
            "d".to_string(),
        );
        let agent2 = Agent::new("Receiver".to_string(), AgentRole::Coder, "d".to_string());

        let id1 = registry.register(agent1);
        let id2 = registry.register(agent2);

        let msg_id = registry.send_message(
            id1.clone(),
            id2.clone(),
            "Hello agent!".to_string(),
            MessageType::Task,
        );

        assert!(msg_id.is_some(), "Message should be sent");

        let messages = registry.get_messages(&id2, 10);
        assert_eq!(messages.len(), 1, "Receiver should have 1 message");
        assert_eq!(messages[0].content, "Hello agent!");

        cleanup_test_dir();
    }

    #[test]
    fn test_send_message_invalid_receiver() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("Valid".to_string(), AgentRole::Coder, "d".to_string());
        let valid_id = registry.register(agent);
        let invalid_id = AgentId("nonexistent".to_string());

        let msg_id = registry.send_message(
            valid_id,
            invalid_id,
            "To nobody".to_string(),
            MessageType::Task,
        );

        assert!(msg_id.is_none(), "Should fail with invalid receiver");

        cleanup_test_dir();
    }

    #[test]
    fn test_message_types() {
        cleanup_test_dir();
        let registry = test_registry();

        let a1 = registry.register(Agent::new(
            "a1".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        let a2 = registry.register(Agent::new(
            "a2".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));

        registry.send_message(
            a1.clone(),
            a2.clone(),
            "Task msg".to_string(),
            MessageType::Task,
        );
        registry.send_message(
            a2.clone(),
            a1.clone(),
            "Response msg".to_string(),
            MessageType::Response,
        );
        registry.send_message(
            a1.clone(),
            a2.clone(),
            "Status update".to_string(),
            MessageType::Status,
        );
        registry.send_message(
            a2.clone(),
            a1.clone(),
            "Error msg".to_string(),
            MessageType::Error,
        );

        let messages = registry.get_messages(&a1, 10);
        assert_eq!(messages.len(), 3, "Should have 3 messages to a1");

        cleanup_test_dir();
    }

    #[test]
    fn test_create_task() {
        cleanup_test_dir();
        let registry = test_registry();

        let task_id = registry.create_task(
            "Implement feature".to_string(),
            "Add new functionality".to_string(),
            TaskPriority::High,
        );

        assert!(!task_id.0.is_empty(), "Task ID should not be empty");

        let tasks = registry.get_tasks(None);
        assert_eq!(tasks.len(), 1, "Should have 1 task");
        assert_eq!(tasks[0].title, "Implement feature");
        assert_eq!(tasks[0].priority, TaskPriority::High);

        cleanup_test_dir();
    }

    #[test]
    fn test_assign_task() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("Worker".to_string(), AgentRole::Coder, "d".to_string());
        let agent_id = registry.register(agent);

        let task_id = registry.create_task(
            "Task to assign".to_string(),
            "Description".to_string(),
            TaskPriority::Normal,
        );

        assert!(
            registry.assign_task(&task_id, &agent_id),
            "Should assign task"
        );

        let tasks = registry.get_tasks(None);
        assert_eq!(tasks[0].status, TaskStatus::Assigned);
        assert_eq!(tasks[0].assigned_to, Some(agent_id));

        cleanup_test_dir();
    }

    #[test]
    fn test_complete_task() {
        cleanup_test_dir();
        let registry = test_registry();

        let task_id = registry.create_task(
            "Task to complete".to_string(),
            "Description".to_string(),
            TaskPriority::Normal,
        );

        assert!(
            registry.complete_task(&task_id, "Done!".to_string()),
            "Should complete task"
        );

        let tasks = registry.get_tasks(None);
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[0].result.as_deref(), Some("Done!"));
        assert!(
            tasks[0].completed_at.is_some(),
            "Should have completion time"
        );

        cleanup_test_dir();
    }

    #[test]
    fn test_task_status_transitions() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = registry.register(Agent::new(
            "W".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        let task_id = registry.create_task("t".to_string(), "d".to_string(), TaskPriority::Normal);

        let tasks = registry.get_tasks(None);
        assert_eq!(tasks[0].status, TaskStatus::Pending);

        registry.assign_task(&task_id, &agent);
        let tasks = registry.get_tasks(None);
        assert_eq!(tasks[0].status, TaskStatus::Assigned);

        registry.complete_task(&task_id, "result".to_string());
        let tasks = registry.get_tasks(None);
        assert_eq!(tasks[0].status, TaskStatus::Completed);

        cleanup_test_dir();
    }

    #[test]
    fn test_filter_tasks_by_status() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = registry.register(Agent::new(
            "W".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));

        let t1 = registry.create_task("t1".to_string(), "d".to_string(), TaskPriority::Normal);
        registry.assign_task(&t1, &agent);

        let t2 = registry.create_task("t2".to_string(), "d".to_string(), TaskPriority::Normal);
        registry.complete_task(&t2, "done".to_string());

        let t3 = registry.create_task("t3".to_string(), "d".to_string(), TaskPriority::Normal);

        let pending = registry.get_tasks(Some(TaskStatus::Pending));
        let assigned = registry.get_tasks(Some(TaskStatus::Assigned));
        let completed = registry.get_tasks(Some(TaskStatus::Completed));

        assert_eq!(pending.len(), 1, "Should have 1 pending");
        assert_eq!(assigned.len(), 1, "Should have 1 assigned");
        assert_eq!(completed.len(), 1, "Should have 1 completed");

        cleanup_test_dir();
    }

    #[test]
    fn test_task_priority_levels() {
        cleanup_test_dir();
        let registry = test_registry();

        let low = registry.create_task("Low".to_string(), "d".to_string(), TaskPriority::Low);
        let normal =
            registry.create_task("Normal".to_string(), "d".to_string(), TaskPriority::Normal);
        let high = registry.create_task("High".to_string(), "d".to_string(), TaskPriority::High);
        let critical = registry.create_task(
            "Critical".to_string(),
            "d".to_string(),
            TaskPriority::Critical,
        );

        let tasks = registry.get_tasks(None);
        assert_eq!(tasks.len(), 4);

        cleanup_test_dir();
    }

    #[test]
    fn test_agent_with_model() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("AI-Agent".to_string(), AgentRole::General, "d".to_string())
            .with_model("llama3:8b");

        let id = registry.register(agent);
        let retrieved = registry.get(&id).unwrap();

        assert_eq!(retrieved.model.as_deref(), Some("llama3:8b"));

        cleanup_test_dir();
    }

    #[test]
    fn test_agent_with_skills() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("MultiSkill".to_string(), AgentRole::Coder, "d".to_string())
            .with_skills(vec!["rust".to_string(), "python".to_string()]);

        let id = registry.register(agent);
        let retrieved = registry.get(&id).unwrap();

        assert_eq!(retrieved.skills.len(), 2);
        assert!(retrieved.skills.contains(&"rust".to_string()));

        cleanup_test_dir();
    }

    #[test]
    fn test_get_agent_by_name() {
        cleanup_test_dir();
        let registry = test_registry();

        registry.register(Agent::new(
            "UniqueAgent".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));

        let found = registry.get_by_name("UniqueAgent");
        assert!(found.is_some(), "Should find agent by name");

        let not_found = registry.get_by_name("Nonexistent");
        assert!(not_found.is_none(), "Should not find nonexistent agent");

        cleanup_test_dir();
    }

    #[test]
    fn test_get_active_agents_count() {
        cleanup_test_dir();
        let registry = test_registry();

        let a1 = registry.register(Agent::new(
            "a1".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        let a2 = registry.register(Agent::new(
            "a2".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        let a3 = registry.register(Agent::new(
            "a3".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));

        registry.update_state(&a1, AgentState::Working);
        registry.update_state(&a2, AgentState::Working);

        assert_eq!(
            registry.get_active_count(),
            2,
            "Should have 2 active agents"
        );

        cleanup_test_dir();
    }

    #[test]
    fn test_unregister_agent() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("Removable".to_string(), AgentRole::Coder, "d".to_string());
        let id = registry.register(agent);

        assert_eq!(registry.count(), 1, "Should have 1 agent");

        let removed = registry.unregister(&id);
        assert!(removed.is_some(), "Should return removed agent");
        assert_eq!(registry.count(), 0, "Should have 0 agents");

        cleanup_test_dir();
    }

    #[test]
    fn test_message_ordering() {
        cleanup_test_dir();
        let registry = test_registry();

        let a1 = registry.register(Agent::new(
            "a1".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));
        let a2 = registry.register(Agent::new(
            "a2".to_string(),
            AgentRole::Coder,
            "d".to_string(),
        ));

        for i in 0..5 {
            registry.send_message(
                a1.clone(),
                a2.clone(),
                format!("Msg {}", i),
                MessageType::Task,
            );
        }

        let messages = registry.get_messages(&a2, 10);
        assert_eq!(messages.len(), 5, "Should have 5 messages");

        cleanup_test_dir();
    }

    #[test]
    fn test_task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1.0, id2.0, "Each TaskId should be unique");
    }

    #[test]
    fn test_agent_id_from_string() {
        let id = AgentId::from_string("test-id".to_string());
        assert_eq!(id.as_str(), "test-id");
    }

    #[test]
    fn test_agent_activity_update() {
        cleanup_test_dir();
        let registry = test_registry();

        let agent = Agent::new("Active".to_string(), AgentRole::Coder, "d".to_string());
        let id = registry.register(agent);

        let before = registry.get(&id).unwrap().last_active;
        std::thread::sleep(std::time::Duration::from_millis(10));

        registry.update_state(&id, AgentState::Working);

        let after = registry.get(&id).unwrap().last_active;
        assert!(after.0 >= before.0, "Activity timestamp should be updated");

        cleanup_test_dir();
    }

    #[test]
    fn test_persistence_across_instances() {
        cleanup_test_dir();

        {
            let registry = test_registry();
            registry.register(Agent::new(
                "Persisted".to_string(),
                AgentRole::Coder,
                "d".to_string(),
            ));
            registry.create_task("Task1".to_string(), "d".to_string(), TaskPriority::Normal);
        }

        {
            let registry = test_registry();
            let agents = registry.list(None);
            let tasks = registry.get_tasks(None);

            assert_eq!(agents.len(), 1, "Agents should persist");
            assert_eq!(tasks.len(), 1, "Tasks should persist");
        }

        cleanup_test_dir();
    }
}
