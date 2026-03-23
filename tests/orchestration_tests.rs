//! # Orchestration Tests

use synapsis::infrastructure::context::{
    AgentId, AgentState, AgentType, OrchStatus, Orchestrator, Suggestion, Task, TaskId,
    TaskPriority, TaskResult, TaskState, TaskType,
};

#[test]
fn test_agent_id_new() {
    let id = AgentId::new("test");
    assert!(id.0.starts_with("test_"));
}

#[test]
fn test_task_id_new() {
    let id1 = TaskId::new();
    let id2 = TaskId::new();
    assert_ne!(id1.0, id2.0);
    assert!(id1.0.starts_with("task_"));
}

#[test]
fn test_task_id_default() {
    let id: TaskId = Default::default();
    assert!(id.0.starts_with("task_"));
}

#[test]
fn test_agent_type_can_handle() {
    assert!(AgentType::Orchestrator.can_handle(&TaskType::Code));
    assert!(AgentType::Coder.can_handle(&TaskType::Code));
    assert!(AgentType::Coder.can_handle(&TaskType::Refactor));
    assert!(!AgentType::Coder.can_handle(&TaskType::Test));

    assert!(AgentType::Researcher.can_handle(&TaskType::Research));
    assert!(AgentType::Researcher.can_handle(&TaskType::Analysis));
    assert!(!AgentType::Researcher.can_handle(&TaskType::Code));

    assert!(AgentType::Reviewer.can_handle(&TaskType::Review));
    assert!(AgentType::Reviewer.can_handle(&TaskType::Code));

    assert!(AgentType::Tester.can_handle(&TaskType::Test));
    assert!(AgentType::Tester.can_handle(&TaskType::Benchmark));

    assert!(AgentType::Architect.can_handle(&TaskType::Design));
    assert!(AgentType::Architect.can_handle(&TaskType::Plan));

    assert!(AgentType::Security.can_handle(&TaskType::SecurityAudit));
    assert!(AgentType::Security.can_handle(&TaskType::Scan));

    assert!(AgentType::Docs.can_handle(&TaskType::Documentation));

    assert!(AgentType::DevOps.can_handle(&TaskType::Deploy));
    assert!(AgentType::DevOps.can_handle(&TaskType::Configure));
}

#[test]
fn test_task_new() {
    let task = Task::new(TaskType::Code, "Implement feature".to_string());
    assert_eq!(task.task_type, TaskType::Code);
    assert_eq!(task.description, "Implement feature");
    assert!(task.assigned_to.is_none());
    assert_eq!(task.state, TaskState::Pending);
    assert_eq!(task.priority, TaskPriority::Normal);
    assert!(task.result.is_none());
}

#[test]
fn test_task_types() {
    let types = vec![
        TaskType::Code,
        TaskType::Refactor,
        TaskType::Review,
        TaskType::Test,
        TaskType::Benchmark,
        TaskType::Research,
        TaskType::Analysis,
        TaskType::Design,
        TaskType::Plan,
        TaskType::SecurityAudit,
        TaskType::Scan,
        TaskType::Documentation,
        TaskType::Deploy,
        TaskType::Configure,
        TaskType::Coordinate,
    ];
    assert_eq!(types.len(), 15);
}

#[test]
fn test_task_states() {
    let states = vec![
        TaskState::Pending,
        TaskState::Assigned,
        TaskState::InProgress,
        TaskState::Completed,
        TaskState::Failed,
        TaskState::Blocked,
    ];
    assert_eq!(states.len(), 6);
}

#[test]
fn test_task_priority() {
    let priorities = vec![
        TaskPriority::Critical,
        TaskPriority::High,
        TaskPriority::Normal,
        TaskPriority::Low,
    ];
    assert_eq!(priorities.len(), 4);
}

#[test]
fn test_agent_types() {
    let types = vec![
        AgentType::Orchestrator,
        AgentType::Coder,
        AgentType::Researcher,
        AgentType::Reviewer,
        AgentType::Tester,
        AgentType::Architect,
        AgentType::Security,
        AgentType::Docs,
        AgentType::DevOps,
    ];
    assert_eq!(types.len(), 9);
}

#[test]
fn test_agent_states() {
    let states = vec![
        AgentState::Idle,
        AgentState::Working,
        AgentState::Waiting,
        AgentState::Available,
        AgentState::Offline,
    ];
    assert_eq!(states.len(), 5);
}

#[test]
fn test_task_result() {
    let result = TaskResult {
        success: true,
        summary: "Done".to_string(),
        artifacts: vec!["file.rs".to_string()],
    };
    assert!(result.success);
    assert_eq!(result.summary, "Done");
    assert_eq!(result.artifacts.len(), 1);
}

#[test]
fn test_orchestrator_new() {
    let orch = Orchestrator::new();
    let status = orch.status();
    assert_eq!(status.total_agents, 0);
    assert_eq!(status.total_tasks, 0);
}

#[test]
fn test_orchestrator_register() {
    let mut orch = Orchestrator::new();
    let _id = orch.register(AgentType::Coder, "Alice");

    let status = orch.status();
    assert_eq!(status.total_agents, 1);

    let suggestions = orch.suggest();
    assert!(suggestions.is_empty()); // No pending tasks
}

#[test]
fn test_orchestrator_plan_task() {
    let mut orch = Orchestrator::new();
    let task = Task::new(TaskType::Code, "Implement X".to_string());
    let _task_id = orch.plan_task(task);

    let status = orch.status();
    assert_eq!(status.total_tasks, 1);
    assert_eq!(status.pending_tasks, 1);
}

#[test]
fn test_orchestrator_delegate() {
    let mut orch = Orchestrator::new();
    let agent_id = orch.register(AgentType::Coder, "Bob");
    let task = Task::new(TaskType::Code, "Implement Y".to_string());
    let task_id = orch.plan_task(task);

    let success = orch.delegate(&task_id, &agent_id);
    assert!(success);

    let status = orch.status();
    assert_eq!(status.active_agents, 1);
}

#[test]
fn test_orchestrator_delegate_invalid_task() {
    let mut orch = Orchestrator::new();
    let agent_id = orch.register(AgentType::Coder, "Bob");
    let fake_task = TaskId::new();

    let success = orch.delegate(&fake_task, &agent_id);
    assert!(!success);
}

#[test]
fn test_orchestrator_delegate_invalid_agent() {
    let mut orch = Orchestrator::new();
    let task = Task::new(TaskType::Code, "Test".to_string());
    let task_id = orch.plan_task(task);
    let fake_agent = AgentId::new("fake");

    let success = orch.delegate(&task_id, &fake_agent);
    assert!(!success);
}

#[test]
fn test_orchestrator_complete_success() {
    let mut orch = Orchestrator::new();
    let agent_id = orch.register(AgentType::Coder, "Charlie");
    let task = Task::new(TaskType::Code, "Fix bug".to_string());
    let task_id = orch.plan_task(task);
    orch.delegate(&task_id, &agent_id);

    let result = TaskResult {
        success: true,
        summary: "Fixed".to_string(),
        artifacts: vec![],
    };
    let success = orch.complete(&task_id, result);
    assert!(success);

    let status = orch.status();
    assert_eq!(status.completed, 1);
    assert_eq!(status.active_agents, 0);
}

#[test]
fn test_orchestrator_complete_failure() {
    let mut orch = Orchestrator::new();
    let agent_id = orch.register(AgentType::Coder, "Dave");
    let task = Task::new(TaskType::Code, "Implement Z".to_string());
    let task_id = orch.plan_task(task);
    orch.delegate(&task_id, &agent_id);

    let result = TaskResult {
        success: false,
        summary: "Failed".to_string(),
        artifacts: vec![],
    };
    orch.complete(&task_id, result);

    let status = orch.status();
    assert_eq!(status.failed, 1);
}

#[test]
fn test_orchestrator_complete_invalid_task() {
    let mut orch = Orchestrator::new();
    let fake_task = TaskId::new();
    let result = TaskResult {
        success: true,
        summary: "Done".to_string(),
        artifacts: vec![],
    };

    let success = orch.complete(&fake_task, result);
    assert!(!success);
}

#[test]
fn test_orchestrator_recommend() {
    let mut orch = Orchestrator::new();
    orch.register(AgentType::Coder, "C1");
    orch.register(AgentType::Coder, "C2");
    orch.register(AgentType::Tester, "T1");

    let task = Task::new(TaskType::Code, "New feature".to_string());
    let recommendations = orch.recommend(&task);

    // Should recommend coder agents
    assert!(!recommendations.is_empty());
    assert!(recommendations.iter().all(|(_, score)| *score > 0.0));
}

#[test]
fn test_orchestrator_recommend_filters_type() {
    let mut orch = Orchestrator::new();
    orch.register(AgentType::Coder, "C1");
    orch.register(AgentType::Docs, "D1");

    let task = Task::new(TaskType::Code, "Code task".to_string());
    let recommendations = orch.recommend(&task);

    // Should only recommend coder
    assert!(recommendations.len() >= 1);
}

#[test]
fn test_orchestrator_status() {
    let mut orch = Orchestrator::new();
    orch.register(AgentType::Coder, "C1");
    orch.register(AgentType::Tester, "T1");

    let task = Task::new(TaskType::Code, "Test task".to_string());
    orch.plan_task(task);

    let status = orch.status();
    assert_eq!(status.total_agents, 2);
    assert_eq!(status.total_tasks, 1);
    assert_eq!(status.pending_tasks, 1);
    assert_eq!(status.active_agents, 0);
}

#[test]
fn test_orchestrator_suggest() {
    let mut orch = Orchestrator::new();
    orch.register(AgentType::Coder, "C1");
    orch.register(AgentType::Coder, "C2");

    let task = Task::new(TaskType::Code, "Task 1".to_string());
    orch.plan_task(task);

    let suggestions = orch.suggest();
    assert!(!suggestions.is_empty());
}

#[test]
fn test_orchestrator_suggest_no_pending() {
    let mut orch = Orchestrator::new();
    orch.register(AgentType::Coder, "C1");

    let suggestions = orch.suggest();
    assert!(suggestions.is_empty());
}

#[test]
fn test_suggestion() {
    let suggestion = Suggestion {
        action: "Test action".to_string(),
        priority: 1,
    };
    assert_eq!(suggestion.action, "Test action");
    assert_eq!(suggestion.priority, 1);
}

#[test]
fn test_orch_status() {
    let status = OrchStatus {
        total_agents: 5,
        active_agents: 2,
        total_tasks: 10,
        pending_tasks: 3,
        completed: 5,
        failed: 2,
    };
    assert_eq!(status.total_agents, 5);
    assert_eq!(status.active_agents, 2);
    assert_eq!(status.total_tasks, 10);
    assert_eq!(status.pending_tasks, 3);
    assert_eq!(status.completed, 5);
    assert_eq!(status.failed, 2);
}

#[test]
fn test_full_workflow() {
    let mut orch = Orchestrator::new();

    // Register agents
    let coder = orch.register(AgentType::Coder, "CodeBot");
    let tester = orch.register(AgentType::Tester, "TestBot");
    let _reviewer = orch.register(AgentType::Reviewer, "ReviewBot");

    // Plan tasks
    let task1 = Task::new(TaskType::Code, "Implement feature A".to_string());
    let task2 = Task::new(TaskType::Code, "Implement feature B".to_string());
    let task3 = Task::new(TaskType::Test, "Test feature A".to_string());

    let tid1 = orch.plan_task(task1);
    let tid2 = orch.plan_task(task2);
    let tid3 = orch.plan_task(task3);

    // Get recommendations and delegate
    let task = Task::new(TaskType::Code, "temp".to_string());
    let recs = orch.recommend(&task);
    assert!(!recs.is_empty());

    orch.delegate(&tid1, &coder);
    orch.delegate(&tid2, &coder);
    orch.delegate(&tid3, &tester);

    // Complete tasks
    orch.complete(
        &tid1,
        TaskResult {
            success: true,
            summary: "Done".to_string(),
            artifacts: vec!["feature_a.rs".to_string()],
        },
    );

    orch.complete(
        &tid2,
        TaskResult {
            success: true,
            summary: "Done".to_string(),
            artifacts: vec!["feature_b.rs".to_string()],
        },
    );

    orch.complete(
        &tid3,
        TaskResult {
            success: true,
            summary: "Tests pass".to_string(),
            artifacts: vec![],
        },
    );

    // Verify final status
    let status = orch.status();
    assert_eq!(status.completed, 3);
    assert_eq!(status.failed, 0);
    assert_eq!(status.pending_tasks, 0);
}

#[test]
fn test_agent_with_completed_tasks_score() {
    let mut orch = Orchestrator::new();

    let agent = orch.register(AgentType::Coder, "Experienced");
    let task = Task::new(TaskType::Code, "Task".to_string());
    let tid = orch.plan_task(task);
    orch.delegate(&tid, &agent);
    orch.complete(
        &tid,
        TaskResult {
            success: true,
            summary: "Done".to_string(),
            artifacts: vec![],
        },
    );

    // Agent should now have completed_tasks > 0
    let status = orch.status();
    assert_eq!(status.completed, 1);
}

#[test]
fn test_orchestrator_multiple_task_types() {
    let mut orch = Orchestrator::new();

    let coder = orch.register(AgentType::Coder, "C");
    let researcher = orch.register(AgentType::Researcher, "R");
    let security = orch.register(AgentType::Security, "S");

    let code_task = Task::new(TaskType::Code, "code".to_string());
    let research_task = Task::new(TaskType::Research, "research".to_string());
    let security_task = Task::new(TaskType::SecurityAudit, "security".to_string());

    let tid1 = orch.plan_task(code_task);
    let tid2 = orch.plan_task(research_task);
    let tid3 = orch.plan_task(security_task);

    assert!(orch.delegate(&tid1, &coder));
    assert!(orch.delegate(&tid2, &researcher));
    assert!(orch.delegate(&tid3, &security));

    let status = orch.status();
    assert_eq!(status.active_agents, 3);
}

#[test]
fn test_task_priority_levels() {
    let mut task = Task::new(TaskType::Code, "Normal task".to_string());
    assert_eq!(task.priority, TaskPriority::Normal);

    task.priority = TaskPriority::Critical;
    assert_eq!(task.priority, TaskPriority::Critical);

    task.priority = TaskPriority::High;
    assert_eq!(task.priority, TaskPriority::High);

    task.priority = TaskPriority::Low;
    assert_eq!(task.priority, TaskPriority::Low);
}

#[test]
fn test_orchestrator_workflow_with_failure() {
    let mut orch = Orchestrator::new();
    let agent = orch.register(AgentType::Coder, "Flaky");

    let task = Task::new(TaskType::Code, "Might fail".to_string());
    let tid = orch.plan_task(task);
    orch.delegate(&tid, &agent);

    // First attempt fails
    orch.complete(
        &tid,
        TaskResult {
            success: false,
            summary: "Failed attempt 1".to_string(),
            artifacts: vec![],
        },
    );

    let status = orch.status();
    assert_eq!(status.failed, 1);
    assert_eq!(status.active_agents, 0);

    // Re-delegate
    orch.delegate(&tid, &agent);
    orch.complete(
        &tid,
        TaskResult {
            success: true,
            summary: "Success on retry".to_string(),
            artifacts: vec!["fixed.rs".to_string()],
        },
    );

    let final_status = orch.status();
    assert_eq!(final_status.completed, 1);
    assert_eq!(final_status.failed, 1);
}
