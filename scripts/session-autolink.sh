#!/bin/bash
# Synapsis Session Auto-Link Daemon
#
# Detects which AI tools are currently running, registers their sessions
# with the SessionBridge, and syncs observations between tools working
# on the same project.
#
# Usage:
#   ./session-autolink.sh                    # Single scan and sync
#   ./session-autolink.sh --daemon           # Run continuously
#   ./session-autolink.sh --once             # Single run (same as no arg)
#   ./session-autolink.sh --status           # Check daemon status

set -euo pipefail

SYNAPSIS_URL="${SYNAPSIS_URL:-http://127.0.0.1:7438}"
MESSAGE_URL="${SYNAPSIS_URL}/message"
POLL_INTERVAL="${POLL_INTERVAL:-30}"
SCRIPT_NAME="$(basename "$0")"
PIDFILE="${TMPDIR:-/tmp}/synapsis-autolink.pid"

# Detect which AI tools are currently running
detect_agents() {
    local agents=()

    # opencode
    if pgrep -x "opencode" >/dev/null 2>&1; then
        agents+=("opencode")
    fi

    # cursor
    if pgrep -x "cursor" >/dev/null 2>&1 || pgrep -f "cursor-server" >/dev/null 2>&1; then
        agents+=("cursor")
    fi

    # vscode
    if pgrep -x "code" >/dev/null 2>&1 || pgrep -f "vscode-server" >/dev/null 2>&1; then
        agents+=("vscode")
    fi

    # Continue (Continue.dev MCP)
    if pgrep -f "continue" >/dev/null 2>&1; then
        agents+=("continue")
    fi

    # Claude Code / Codex CLI
    if pgrep -f "claude-code" >/dev/null 2>&1 || pgrep -f "codex" >/dev/null 2>&1; then
        agents+=("claude-code")
    fi

    # aider
    if pgrep -f "aider" >/dev/null 2>&1; then
        agents+=("aider")
    fi

    # warp terminal
    if pgrep -x "warp" >/dev/null 2>&1; then
        agents+=("warp")
    fi

    # Echo your current shell (as an agent)
    if [ -n "${SHELL:-}" ]; then
        agents+=("shell:$(basename "$SHELL")")
    fi

    printf '%s\n' "${agents[@]}"
}

# Detect project from current directory or cwd of running processes
detect_project() {
    local agent="$1"
    local project_dir

    case "$agent" in
        opencode)
            project_dir="$(pgrep -x opencode 2>/dev/null | head -1 | xargs -I{} readlink /proc/{}/cwd 2>/dev/null || echo "$PWD")"
            ;;
        cursor|vscode|code)
            local pid
            pid="$(pgrep -x "$agent" 2>/dev/null | head -1 || pgrep -f "vscode-server" 2>/dev/null | head -1 || echo "")"
            if [ -n "$pid" ]; then
                project_dir="$(readlink "/proc/$pid/cwd" 2>/dev/null || echo "$PWD")"
            else
                project_dir="$PWD"
            fi
            ;;
        *)
            project_dir="$PWD"
            ;;
    esac

    # Extract project name from directory
    basename "$project_dir" 2>/dev/null || echo "default"
}

# Generate a stable session ID for an agent+project combo
generate_session_id() {
    local agent="$1"
    local project="$2"
    echo "${agent}-${project}-$(hostname)-$(whoami)" | md5sum | cut -d' ' -f1 | cut -c1-16
}

# Send JSON-RPC request to Synapsis
mcp_call() {
    local method="$1"
    local data="$2"
    local payload
    payload="$(cat <<EOF
{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
        "name": "$method",
        "arguments": $data
    },
    "id": $(( $(date +%s%N) % 1000000 ))
}
EOF
)"
    curl -s -X POST "$MESSAGE_URL" \
        -H "Content-Type: application/json" \
        -d "$payload" 2>/dev/null
}

# Check if Synapsis is running
synapsis_alive() {
    local payload='{"jsonrpc":"2.0","method":"initialize","id":1}'
    curl -s -o /dev/null -w "%{http_code}" -X POST "$MESSAGE_URL" \
        -H "Content-Type: application/json" \
        -d "$payload" 2>/dev/null | grep -q "200"
}

# Register a session for a detected agent
register_session() {
    local agent="$1"
    local project="$2"
    local session_id="$3"

    # Start session via MCP
    local data
    data="$(cat <<EOF
{"project": "$project", "session_id": "$session_id", "directory": "."}
EOF
)"
    mcp_call "mem_session_start" "$data" > /dev/null 2>&1

    # Save a heartbeat observation
    local heartbeat_data
    heartbeat_data="$(cat <<EOF
{"title": "heartbeat:$agent", "content": "Agent $agent active on project $project at $(date -Iseconds)", "type": "discovery", "project": "$project", "session_id": "$session_id"}
EOF
)"
    mcp_call "mem_save" "$heartbeat_data" > /dev/null 2>&1

    echo "[synapsis-autolink] Registered: $agent on $project (session: $session_id)"
}

# Run a single sync cycle
sync_once() {
    if ! synapsis_alive; then
        echo "[synapsis-autolink] Synapsis MCP server not reachable at $SYNAPSIS_URL" >&2
        return 1
    fi

    local agents
    agents="$(detect_agents)"

    if [ -z "$agents" ]; then
        echo "[synapsis-autolink] No AI agents detected"
        return 0
    fi

    echo "[synapsis-autolink] Detected agents:"
    echo "$agents" | while IFS= read -r agent; do
        echo "  - $agent"
    done

    echo "$agents" | while IFS= read -r agent; do
        local project
        project="$(detect_project "$agent")"
        local session_id
        session_id="$(generate_session_id "$agent" "$project")"
        register_session "$agent" "$project" "$session_id"
    done
}

# Daemon mode: run continuously
run_daemon() {
    echo "[synapsis-autolink] Starting daemon (PID: $$)"
    echo "$$" > "$PIDFILE"

    # Initial sync
    sync_once

    while true; do
        sleep "$POLL_INTERVAL"
        sync_once
    done
}

# Check daemon status
status_daemon() {
    if [ -f "$PIDFILE" ]; then
        local pid
        pid="$(cat "$PIDFILE")"
        if kill -0 "$pid" 2>/dev/null; then
            echo "synapsis-autolink: RUNNING (PID $pid)"
            echo "Poll interval: ${POLL_INTERVAL}s"
            echo "Endpoint: $MESSAGE_URL"
            return 0
        else
            echo "synapsis-autolink: STOPPED (stale PID $pid)"
            rm -f "$PIDFILE"
            return 1
        fi
    fi

    # Check if running without PID file
    local running_pid
    running_pid="$(pgrep -f "session-autolink.sh --daemon" 2>/dev/null | head -1 || true)"
    if [ -n "$running_pid" ]; then
        echo "synapsis-autolink: RUNNING (PID $running_pid, no pidfile)"
        return 0
    fi

    echo "synapsis-autolink: STOPPED"
    return 1
}

# Main
case "${1:-}" in
    --daemon)
        run_daemon
        ;;
    --once|"")
        sync_once
        ;;
    --status)
        status_daemon
        ;;
    --help|-h)
        echo "Synapsis Session Auto-Link"
        echo ""
        echo "Usage:"
        echo "  $0                    Single scan and sync"
        echo "  $0 --once            Single scan and sync"
        echo "  $0 --daemon          Run continuously as a daemon"
        echo "  $0 --status          Check daemon status"
        echo "  $0 --help            Show this help"
        echo ""
        echo "Environment:"
        echo "  SYNAPSIS_URL     MCP server URL (default: http://127.0.0.1:7438)"
        echo "  POLL_INTERVAL    Daemon poll interval in seconds (default: 30)"
        ;;
    *)
        echo "Unknown option: $1"
        echo "Usage: $0 [--daemon|--once|--status|--help]"
        exit 1
        ;;
esac
