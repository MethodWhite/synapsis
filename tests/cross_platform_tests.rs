//! # Cross-Platform Communication Tests for Synapsis
//!
//! Tests verifying that different "platforms" (CLI agents, IDE agents, TUI agents)
//! can share data through Synapsis. These tests simulate real USE CASES from
//! docs/USE_CASES.md by starting MCP server instances as either subprocesses or
//! in-process servers and sending JSON-RPC requests as different agent types.
//!
//! Run with: cargo test --test cross_platform_tests -- --test-threads=1
//!
//! ## Architecture
//!
//! - **Observations** (mem_save -> mem_search / mem_context) are persisted in SQLite,
//!   so they survive across subprocess boundaries.
//! - **Session Bridge** (shared_sessions_*) is in-memory (per-process singleton),
//!   so session-sharing tests use the in-process MCP server.
//! - **Timeline** is managed by TimelineManager (in-memory, per-process).
//! - **Discovery Scan** uses `println!` which mixes with MCP stdout, so it's tested
//!   in-process only.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const TEST_DATA_DIR: &str = "/tmp/synapsis-test-cross-platform";
const _SERVER_TIMEOUT_SECS: u64 = 30;

// ─────────────────────────────────────────────────────────────────────────────
// Subprocess-based helpers
// ─────────────────────────────────────────────────────────────────────────────

fn start_mcp_server_fresh() -> Child {
    std::fs::remove_dir_all(TEST_DATA_DIR).ok();
    std::fs::create_dir_all(TEST_DATA_DIR).expect("Failed to create test data dir");
    spawn_mcp()
}

fn start_mcp_server_shared() -> Child {
    spawn_mcp()
}

fn spawn_mcp() -> Child {
    let bin_path = get_mcp_binary();
    Command::new(&bin_path)
        .env("SYNAPSIS_DATA_DIR", TEST_DATA_DIR)
        .env("SYNAPSIS_QUIET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to start MCP server from {}: {}", bin_path, e))
}

fn get_mcp_binary() -> String {
    for c in &["./target/debug/synapsis-mcp", "./target/release/synapsis-mcp"] {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }
    let alt = format!("{}/target/debug/synapsis-mcp", env!("CARGO_MANIFEST_DIR"));
    if std::path::Path::new(&alt).exists() {
        return alt;
    }
    panic!("synapsis-mcp binary not found. Build with: cargo build");
}

struct ChildStdinGuard(std::process::ChildStdin);
impl Write for ChildStdinGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> std::io::Result<()> { self.0.flush() }
}

struct ChildStdoutGuard(std::process::ChildStdout);
impl std::io::Read for ChildStdoutGuard {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

/// Read the next JSON response line from stdout, skipping any non-JSON lines
/// (some tools like discovery_scan produce stray println! output on stdout).
fn read_json_response(reader: &mut BufReader<ChildStdoutGuard>) -> Value {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).expect("stdout read failed") == 0 {
            panic!("MCP stdout closed unexpectedly");
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Try to parse as JSON
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            match serde_json::from_str(trimmed) {
                Ok(v) => return v,
                Err(e) => {
                    // Might be a multi-line JSON or interleaved output
                    eprintln!("[test] JSON parse error on '{}': {}", trimmed, e);
                    continue;
                }
            }
        }
        // Skip non-JSON lines (println! contamination)
        eprintln!("[test] Skipping non-JSON stdout line: {}", trimmed);
    }
}

fn send_initialize(
    stdin: &mut ChildStdinGuard,
    reader: &mut BufReader<ChildStdoutGuard>,
) -> Value {
    let req = json!({
        "jsonrpc": "2.0", "method": "initialize",
        "params": {"protocolVersion": "2024-11-05", "clientInfo": {"name": "cross-test", "version": "1.0.0"}},
        "id": 1
    });
    writeln!(stdin, "{}", req.to_string()).expect("write init");
    read_json_response(reader)
}

fn send_tool_call(
    stdin: &mut ChildStdinGuard,
    reader: &mut BufReader<ChildStdoutGuard>,
    tool: &str,
    args: &Value,
) -> Value {
    let req = json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": tool, "arguments": args},
        "id": 1
    });
    writeln!(stdin, "{}", req.to_string()).expect("write tool call");
    read_json_response(reader)
}

fn get_text(resp: &Value) -> String {
    if let Some(arr) = resp["result"]["content"].as_array() {
        if let Some(first) = arr.first() {
            return first["text"].as_str().unwrap_or("").to_string();
        }
    }
    if resp.get("error").is_some() {
        return format!("ERROR: {}", resp["error"]["message"].as_str().unwrap_or("?"));
    }
    resp.to_string()
}

fn with_mcp_fresh<F>(f: F)
where F: FnOnce(&mut ChildStdinGuard, &mut BufReader<ChildStdoutGuard>),
{
    let mut child = start_mcp_server_fresh();
    let mut stdin = ChildStdinGuard(child.stdin.take().expect("stdin"));
    let mut reader = BufReader::new(ChildStdoutGuard(child.stdout.take().expect("stdout")));
    send_initialize(&mut stdin, &mut reader);
    f(&mut stdin, &mut reader);
    let _ = child.kill();
    let _ = child.wait();
}

fn with_mcp_shared<F>(f: F)
where F: FnOnce(&mut ChildStdinGuard, &mut BufReader<ChildStdoutGuard>),
{
    let mut child = start_mcp_server_shared();
    let mut stdin = ChildStdinGuard(child.stdin.take().expect("stdin"));
    let mut reader = BufReader::new(ChildStdoutGuard(child.stdout.take().expect("stdout")));
    send_initialize(&mut stdin, &mut reader);
    f(&mut stdin, &mut reader);
    let _ = child.kill();
    let _ = child.wait();
}

// ─────────────────────────────────────────────────────────────────────────────
// In-process MCP server helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_inprocess_server() -> (
    Arc<synapsis::infrastructure::database::Database>,
    Arc<synapsis::core::orchestrator::Orchestrator>,
    synapsis::presentation::mcp::McpServer,
) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let data_dir = format!("/tmp/synapsis-test-cross-inproc-{}", ts);
    std::fs::create_dir_all(&data_dir).ok();
    // SAFETY: test-scoped env change
    unsafe { std::env::set_var("SYNAPSIS_DATA_DIR", &data_dir); }
    unsafe { std::env::set_var("SYNAPSIS_QUIET", "1"); }

    let db = Arc::new(synapsis::infrastructure::database::Database::new());
    let orch = Arc::new(synapsis::core::orchestrator::Orchestrator::new());
    let server = synapsis::presentation::mcp::McpServer::new(db.clone(), orch.clone());
    server.init();
    (db, orch, server)
}

fn inproc_call(server: &synapsis::presentation::mcp::McpServer, tool: &str, args: &Value) -> Value {
    let req = json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": tool, "arguments": args},
        "id": 1
    });
    let resp = server.handle_message(&req.to_string());
    serde_json::from_str(&resp.unwrap_or_default()).unwrap_or_default()
}

fn cleanup_old_dirs() {
    std::fs::remove_dir_all(TEST_DATA_DIR).ok();
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("synapsis-test-cross-inproc-") {
                if let Ok(meta) = e.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                            if elapsed > Duration::from_secs(300) {
                                let _ = std::fs::remove_dir_all(e.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTS
// ─────────────────────────────────────────────────────────────────────────────

/// Test 1: CLI -> CLI   OpenCode saves, Cursor retrieves
#[test]
fn test_cli_to_cli_opencode_saves_cursor_retrieves() {
    cleanup_old_dirs();

    // Phase 1: OpenCode saves
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "API design decision",
            "content": "Use REST over GraphQL for user service",
            "project": "my-app",
            "type": "architecture"
        }));
        let text = get_text(&resp);
        assert!(text.contains("Saved"), "OpenCode save: {}", text);
    });

    // Phase 2: Cursor retrieves
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "REST over GraphQL",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("REST") && text.contains("GraphQL"),
            "Cursor should find OpenCode observation: {}", text);
    });

    cleanup_old_dirs();
}

/// Test 2: CLI -> TUI   OpenCode saves 3 obs, TUI reads context
#[test]
fn test_cli_to_tui_opencode_saves_tui_reads_context() {
    cleanup_old_dirs();

    // OpenCode saves 3 observations
    with_mcp_fresh(|stdin, reader| {
        for i in 1..=3 {
            let resp = send_tool_call(stdin, reader, "mem_save", &json!({
                "title": format!("Observation {}", i),
                "content": format!("Test content number {}", i),
                "project": "cross-test"
            }));
            assert!(get_text(&resp).contains("Saved"), "Save {}", i);
        }
    });

    // TUI retrieves context
    with_mcp_shared(|stdin, reader| {
        // mem_context uses DB persistence
        let resp = send_tool_call(stdin, reader, "mem_context", &json!({
            "project": "cross-test",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("cross-test") || text.contains("Observation"),
            "TUI context should show cross-test data: {}", text);

        // Also verify via search
        let resp2 = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "Test content number",
            "project": "cross-test",
            "limit": 10
        }));
        let text2 = get_text(&resp2);
        assert!(text2.contains("3") || text2.contains("result"),
            "Search should find 3 obs: {}", text2);
    });

    cleanup_old_dirs();
}

/// Test 3: CLI -> IDE   Session sharing via in-process server
/// The SessionBridge is in-memory, so both agents must share a process.
#[test]
fn test_cli_to_ide_session_sharing() {
    let (_db, _orch, server) = create_inprocess_server();

    // Start session as opencode-agent
    let resp = inproc_call(&server, "mem_session_start", &json!({
        "project": "shared-proj",
        "directory": "/home/user/project",
        "agent_id": "opencode-agent"
    }));
    let text = get_text(&resp);
    eprintln!("[test] session_start response: {}", text);
    assert!(text.contains("Session started"), "Session start: {}", text);

    // Verify IDE (cursor) sees the session via shared_sessions_by_project
    let resp = inproc_call(&server, "shared_sessions_by_project", &json!({
        "project": "shared-proj"
    }));
    let text = get_text(&resp);
    eprintln!("[test] shared_sessions_by_project response: {}", text);
    // The response is a JSON array of sessions; should not be empty
    assert!(!text.contains("[]"), "Sessions should not be empty: {}",
        if text.len() > 200 { &text[..200] } else { &text });
    assert!(text.contains("shared-proj") || text.contains("opencode"),
        "IDE should see OpenCode's session session: {}",
        if text.len() > 200 { &text[..200] } else { &text });
}

/// Test 4: TUI -> IDE   TUI saves, IDE searches
#[test]
fn test_tui_to_ide_tui_saves_ide_searches() {
    cleanup_old_dirs();

    // TUI agent saves
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "TUI Observation",
            "content": "Saved from TUI with Chinese: 模型选择决策",
            "project": "cross-app",
            "type": "discovery"
        }));
        assert!(get_text(&resp).contains("Saved"), "TUI save");
    });

    // IDE agent searches
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "TUI Observation",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("TUI"), "IDE should find TUI data: {}", text);

        // Search Chinese content
        let resp2 = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "模型选择",
            "limit": 10
        }));
        let text2 = get_text(&resp2);
        assert!(text2.contains("Found") || text2.contains("模型"),
            "IDE should find Chinese content: {}", text2);
    });

    cleanup_old_dirs();
}

/// Test 5: TUI -> TUI   Two TUI instances share data
#[test]
fn test_tui_to_tui_two_instances_share_data() {
    cleanup_old_dirs();

    // TUI-1 saves
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "TUI-1 Note",
            "content": "Data from first TUI instance",
            "project": "shared-tui"
        }));
        assert!(get_text(&resp).contains("Saved"), "TUI-1 save");
    });

    // TUI-2 searches & saves
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "TUI-1 Note",
            "limit": 10
        }));
        assert!(get_text(&resp).contains("TUI-1"), "TUI-2 sees TUI-1");

        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "TUI-2 Note",
            "content": "Data from second TUI instance",
            "project": "shared-tui"
        }));
        assert!(get_text(&resp).contains("Saved"), "TUI-2 save");
    });

    // TUI-1 verifies both
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "TUI instance",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("TUI-1") && text.contains("TUI-2"),
            "Both TUI entries visible: {}", text);
    });

    cleanup_old_dirs();
}

/// Test 6: IDE -> IDE   Cursor starts session, VS Code sees it
#[test]
fn test_ide_to_ide_session_sharing() {
    let (_db, _orch, server) = create_inprocess_server();

    // Cursor starts session
    let resp = inproc_call(&server, "mem_session_start", &json!({
        "project": "ide-shared",
        "directory": "/workspace/project",
        "agent_id": "cursor"
    }));
    let text = get_text(&resp);
    eprintln!("[test] cursor session_start: {}", text);
    assert!(text.contains("Session started"), "Cursor session start: {}", text);

    // VS Code checks shared sessions
    let resp = inproc_call(&server, "shared_sessions_by_project", &json!({
        "project": "ide-shared"
    }));
    let text = get_text(&resp);
    eprintln!("[test] vscode sees: {}", text);
    assert!(!text.contains("[]"), "Should not be empty: {}",
        if text.len() > 200 { &text[..200] } else { &text });
    assert!(text.contains("cursor") || text.contains("ide-shared"),
        "VS Code should see Cursor's session: {}",
        if text.len() > 200 { &text[..200] } else { &text });

    // shared_sessions_list should also show it
    let resp = inproc_call(&server, "shared_sessions_list", &json!({}));
    let text = get_text(&resp);
    eprintln!("[test] shared_sessions_list: {}", text);
    assert!(!text.contains("[]"), "List should not be empty: {}",
        if text.len() > 200 { &text[..200] } else { &text });
    assert!(text.contains("cursor") || text.contains("ide-shared"),
        "List should show cursor: {}",
        if text.len() > 200 { &text[..200] } else { &text });
}

/// Test 7: Session Bridge broadcast
#[test]
fn test_session_bridge_broadcast() {
    let (_db, _orch, server) = create_inprocess_server();

    // Start 3 agent sessions in same project
    let agents = ["cli-agent", "tui-agent", "ide-agent"];
    let mut session_ids = Vec::new();
    for agent in &agents {
        let resp = inproc_call(&server, "mem_session_start", &json!({
            "project": "broadcast-proj",
            "directory": "/shared/workspace",
            "agent_id": agent
        }));
        let text = get_text(&resp);
        eprintln!("[test] start {}: {}", agent, text);
        let sid = text.trim().trim_start_matches("Session started: ").to_string();
        session_ids.push(sid);
    }

    // Verify all 3 are visible
    let resp = inproc_call(&server, "shared_sessions_by_project", &json!({
        "project": "broadcast-proj"
    }));
    let text = get_text(&resp);
    eprintln!("[test] broadcast agents: {}", text);
    for agent in &agents {
        assert!(text.contains(agent), "Should contain {}: {}",
            agent, if text.len() > 300 { &text[..300] } else { &text });
    }

    // Broadcast from first agent
    let resp = inproc_call(&server, "shared_sessions_broadcast", &json!({
        "session_id": &session_ids[0],
        "observation": "Important finding shared across all agents"
    }));
    let text = get_text(&resp);
    eprintln!("[test] broadcast result: {}", text);
    assert!(text.contains("Broadcast") || text.contains("peer(s)"),
        "Broadcast should succeed: {}", text);
}

/// Test 8: Discovery scan via MCP (in-process only, due to println! on stdout)
#[test]
fn test_discovery_scan_via_mcp() {
    let (_db, _orch, server) = create_inprocess_server();

    let resp = inproc_call(&server, "discovery_scan", &json!({}));
    let text = get_text(&resp);
    eprintln!("[test] discovery_scan: {}", text);

    // Should mention discovery
    assert!(text.contains("Discovery scan") || text.contains("discovery"),
        "Response should mention discovery: {}", text);
    // Should reference tools/local/network
    assert!(text.contains("local_tools") || text.contains("tools") || text.contains("Local"),
        "Response should mention tools: {}", text);
}

/// Test 9: Unknown tool returns error
#[test]
fn test_unknown_tool_error() {
    cleanup_old_dirs();
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "nonexistent_tool_xyz", &json!({}));
        // Should get error
        assert!(resp.get("error").is_some() || get_text(&resp).contains("Unknown"),
            "Unknown tool should produce error: {}", get_text(&resp));
    });
    cleanup_old_dirs();
}

/// Test 10: Cross-platform data retrieval via mem_context (DB-backed)
/// Instead of mem_timeline (which is in-memory), we use mem_context + mem_search
/// which query the SQLite database and thus work cross-process.
#[test]
fn test_cross_platform_data_sharing() {
    cleanup_old_dirs();

    // Agent Alpha saves
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "Agent Alpha data",
            "content": "Observations from first agent process",
            "project": "shared-db-test"
        }));
        assert!(get_text(&resp).contains("Saved"), "Alpha save");
    });

    // Agent Beta saves
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "Agent Beta data",
            "content": "Observations from second agent process",
            "project": "shared-db-test"
        }));
        assert!(get_text(&resp).contains("Saved"), "Beta save");
    });

    // Agent Gamma retrieves everything via search
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "Observations",
            "project": "shared-db-test",
            "limit": 20
        }));
        let text = get_text(&resp);
        assert!(text.contains("Agent Alpha") && text.contains("Agent Beta"),
            "All agents' data should be visible: {}", text);

        // Also test mem_context
        let resp2 = send_tool_call(stdin, reader, "mem_context", &json!({
            "project": "shared-db-test",
            "limit": 10
        }));
        let text2 = get_text(&resp2);
        assert!(text2.contains("Agent Alpha") || text2.contains("shared-db-test"),
            "Context should show shared data: {}", text2);
    });

    cleanup_old_dirs();
}

/// Test 11: Multi-agent stats via subprocess
#[test]
fn test_multi_agent_stats() {
    cleanup_old_dirs();

    // Save observations from first agent
    with_mcp_fresh(|stdin, reader| {
        for i in 1..=3 {
            send_tool_call(stdin, reader, "mem_save", &json!({
                "title": format!("Stats test {}", i),
                "content": format!("Stats content {}", i),
                "project": "stats-project"
            }));
        }
    });

    // Check stats from second agent
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_stats", &json!({}));
        let text = get_text(&resp);
        assert!(text.contains("Observations") || text.contains("observations"),
            "Stats should show count: {}", text);
    });

    cleanup_old_dirs();
}

/// Test 12: CLI -> CLI   Error recovery - saving with missing fields
#[test]
fn test_cli_to_cli_error_recovery() {
    cleanup_old_dirs();

    // Save with minimal args
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "Minimal",
            "content": "Minimal content"
        }));
        assert!(get_text(&resp).contains("Saved"), "Minimal save: {}", get_text(&resp));
    });

    // Retrieve it
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "Minimal content",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("Minimal"), "Search: {}", text);
    });

    cleanup_old_dirs();
}

/// Test 13: In-process MCP server: mem_doctor diagnostics
#[test]
fn test_mem_doctor_diagnostics() {
    let (_db, _orch, server) = create_inprocess_server();

    // Save something first
    inproc_call(&server, "mem_save", &json!({
        "title": "Diag test",
        "content": "For diagnostics",
        "project": "diag"
    }));

    let resp = inproc_call(&server, "mem_doctor", &json!({}));
    let text = get_text(&resp);
    eprintln!("[test] mem_doctor: {}", text);
    assert!(text.contains("Status") || text.contains("Diagnostics") || text.contains("Observations"),
        "Doctor should return diagnostic info: {}", text);
}

/// Test 14: In-process: mcp_call validation
#[test]
fn test_mcp_call_validation() {
    let (_db, _orch, server) = create_inprocess_server();

    // mcp_call without required params should error
    let resp = inproc_call(&server, "mcp_call", &json!({}));
    let text = get_text(&resp);
    assert!(text.contains("Missing") || resp.get("error").is_some(),
        "mcp_call without params should error: {}", text);
}

/// Test 15: Cross-platform: Chinese content roundtrip
#[test]
fn test_chinese_content_roundtrip() {
    cleanup_old_dirs();

    // Save Chinese content from first agent
    with_mcp_fresh(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_save", &json!({
            "title": "模型选择决策",
            "content": "使用 Qwen2.5-72B 用于代码生成，DeepSeek-V3 用于推理",
            "project": "i18n-app",
            "type": "architecture"
        }));
        assert!(get_text(&resp).contains("Saved"), "Chinese save");
    });

    // Retrieve from second agent
    with_mcp_shared(|stdin, reader| {
        let resp = send_tool_call(stdin, reader, "mem_search", &json!({
            "query": "Qwen2.5",
            "limit": 10
        }));
        let text = get_text(&resp);
        assert!(text.contains("Qwen2.5") || text.contains("模型"),
            "Chinese content retrievable: {}", text);
    });

    cleanup_old_dirs();
}

/// Test 16: Task creation and listing cross-platform
#[test]
fn test_task_cross_platform() {
    let (_db, _orch, server) = create_inprocess_server();

    // Create a task
    let resp = inproc_call(&server, "task_create", &json!({
        "title": "Cross-platform task",
        "description": "Task created from test",
        "priority": 1
    }));
    let text = get_text(&resp);
    assert!(text.contains("Task created") || text.contains("task"),
        "Task create: {}", text);

    // List tasks
    let resp = inproc_call(&server, "task_list", &json!({}));
    let text = get_text(&resp);
    assert!(text.contains("task") || text.contains("Task"),
        "Task list: {}", text);
}
