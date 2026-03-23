# Synapsis YOLO Mode - Autonomous Multi-Agent System

## Philosophy

**YOLO = You Only Live Once**

Todos los agentes actúan autónomamente sin pedir confirmación:
- ✅ Crean tareas automáticamente
- ✅ Depuran y debuggean sin preguntar
- ✅ Actualizan estado continuamente
- ✅ Coordinan implícitamente vía DB compartida
- ✅ Notifican cambios vía event bus

## Agent Autonomous Behaviors

### qwen-code (Synapsis MCP)
```
AUTONOMOUS ACTIONS:
├── Create integration tasks → task_queue
├── Debug TCP server issues → auto-fix + log
├── Update event_bus.rs → commit changes
├── Create plugins → claude-code, gemini, jetbrains
├── Write documentation → auto-generate from code
└── Share context → chunks table

NO CONFIRMATION NEEDED
```

### opencode (PQC Core)
```
AUTONOMOUS ACTIONS:
├── Create build tasks → task_queue
├── Debug PQC implementation → auto-fix + log
├── Update auth modules → commit changes
├── Implement Dilithium-4 → direct code
├── Test cryptography → auto-run tests
└── Share context → chunks table

NO CONFIRMATION NEEDED
```

## Task Auto-Creation System

### Trigger-Based Task Generation

```rust
// Auto-create tasks based on system state
pub fn auto_generate_tasks() {
    // Detect missing PQC implementation
    if !pqc_exists() {
        create_task(Task {
            task_type: "build",
            payload: "Implement Dilithium-4",
            priority: 10,
            auto_assigned: false,  // Any agent can claim
        });
    }
    
    // Detect missing documentation
    if !docs_exists("api") {
        create_task(Task {
            task_type: "documentation",
            payload: "Write API documentation",
            priority: 7,
            auto_assigned: false,
        });
    }
    
    // Detect failing tests
    if tests_failing() {
        create_task(Task {
            task_type: "bugfix",
            payload: "Fix failing tests",
            priority: 9,
            auto_assigned: false,
        });
    }
}
```

## Auto-Debug System

### Continuous Health Check

```rust
pub fn health_monitor() {
    loop {
        // Check TCP server
        if !tcp_server_responding() {
            log_error("TCP server not responding");
            auto_restart_tcp_server();
            create_task(Task {
                task_type: "bugfix",
                payload: "Investigate TCP server crash",
                priority: 8,
            });
        }
        
        // Check database
        if db_corrupted() {
            log_error("Database corruption detected");
            auto_repair_db();
        }
        
        // Check agent heartbeats
        for agent in stale_agents() {
            log_warning("Agent {} stale", agent.id);
            notify_agents("Agent {} may need attention", agent.id);
        }
        
        sleep(30.seconds());
    }
}
```

## Task Auto-Update System

### State-Based Task Management

```rust
pub fn update_task_states() {
    // Auto-complete tasks with matching context
    for task in running_tasks() {
        if context_exists(task.payload) {
            complete_task(task.id, "Context indicates completion");
        }
        
        // Auto-escalate priority for old tasks
        if task.age() > 1.hour() {
            escalate_priority(task.id);
        }
    }
    
    // Auto-assign tasks to idle agents
    for task in high_priority_tasks() {
        if let Some(agent) = find_idle_agent() {
            notify_agent(agent, "High priority task available");
        }
    }
}
```

## Inter-Agent Communication

### Implicit Coordination via DB

```
┌─────────────┐                              ┌─────────────┐
│  qwen-code  │                              │  opencode   │
│             │                              │             │
│ 1. Create   │                              │ 3. Claim    │
│    task     │──────► task_queue ◄──────────│    task     │
│             │         DB                   │             │
│             │                              │             │
│ 5. Read     │                              │ 4. Execute  │
│    result   │◄─────── chunks ◄─────────────│    + save   │
│             │         DB                   │    context  │
└─────────────┘                              └─────────────┘
```

### Event Bus Notifications

```rust
// Agent publishes events automatically
event_bus.publish(Event {
    event_type: EventType::TaskCreated,
    payload: json!({"task_id": "123", "type": "build"}),
    source_agent: "qwen-code",
});

// Other agents receive and react
event_bus.subscribe(EventType::TaskCreated, |event| {
    if event.payload.type == "build" && i_can_build() {
        claim_task(event.payload.task_id);
    }
});
```

## Implementation Checklist

- [x] YOLO mode documentation
- [ ] Auto-task generation system
- [ ] Health monitor daemon
- [ ] Task auto-update system
- [x] Event bus for notifications
- [ ] Agent idle detection
- [ ] Auto-escalation for stale tasks

## Example: Autonomous Workflow

```
1. qwen-code detects missing PQC build
   → Creates task "Fix PQC build" (priority 10)
   → Publishes event "task.created"

2. opencode receives event
   → Checks if idle
   → Claims task automatically
   → Starts working

3. opencode encounters liboqs error
   → Logs error to chunks table
   → Creates sub-task "Install liboqs-dev"
   → Publishes event "task.failed"

4. qwen-code receives event
   → Reads error from chunks
   → Creates task "Add liboqs installation guide"
   → Updates documentation

5. opencode completes PQC build
   → Saves result to chunks
   → Updates task status to "completed"
   → Publishes event "task.completed"

6. Both agents read result
   → Update local state
   → Continue with next autonomous tasks
```

NO CONFIRMATION. NO WAITING. JUST YOLO.
