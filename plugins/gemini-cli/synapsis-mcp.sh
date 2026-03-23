#!/bin/bash
# Synapsis MCP - Gemini CLI Integration

SYNAPSIS_SERVER="127.0.0.1:7438"
PROJECT_KEY="${SYNAPSIS_PROJECT:-gemini-cli}"
SESSION_ID=""
SESSION_FILE="/tmp/synapsis-gemini-session"

# Load or create session
load_session() {
    if [ -f "$SESSION_FILE" ]; then
        SESSION_ID=$(cat "$SESSION_FILE")
    else
        register_session
    fi
}

# Register new session
register_session() {
    local agent_type="gemini-cli"
    local instance=$(hostname)-$$
    local response=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"session_register\",\"params\":{\"agent_type\":\"$agent_type\",\"project\":\"$PROJECT_KEY\"},\"id\":1}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
    SESSION_ID=$(echo "$response" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
    if [ -n "$SESSION_ID" ]; then
        echo "$SESSION_ID" > "$SESSION_FILE"
        echo "Registered: $SESSION_ID"
    fi
}

# Send heartbeat
heartbeat() {
    local task="$1"
    if [ -n "$SESSION_ID" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"method\":\"agent_heartbeat\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"task\":\"$task\"}},\"id\":2}" | nc -w1 $SYNAPSIS_SERVER > /dev/null 2>&1
    fi
}

# Save context
save_context() {
    local title="$1"
    local content="$2"
    load_session
    heartbeat "saving-context"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"chunk_create\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\",\"title\":\"$title\",\"content\":\"$content\"}},\"id\":3}" | nc -w1 $SYNAPSIS_SERVER
}

# Search memory
search() {
    local query="$1"
    local limit="${2:-10}"
    load_session
    heartbeat "searching"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"memory_search_fts\",\"params\":{\"arguments\":{\"query\":\"$query\",\"project\":\"$PROJECT_KEY\",\"limit\":$limit}},\"id\":4}" | nc -w1 $SYNAPSIS_SERVER
}

# Get global context
get_global_context() {
    load_session
    heartbeat "getting-context"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"global_context_get\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\"}},\"id\":5}" | nc -w1 $SYNAPSIS_SERVER
}

# Acquire lock
acquire_lock() {
    local lock_key="$1"
    local ttl="${2:-300}"
    load_session
    heartbeat "acquiring-lock"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"lock_acquire\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"lock_key\":\"$lock_key\",\"ttl\":$ttl}},\"id\":6}" | nc -w1 $SYNAPSIS_SERVER
}

# Release lock
release_lock() {
    local lock_key="$1"
    heartbeat "releasing-lock"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"lock_release\",\"params\":{\"arguments\":{\"lock_key\":\"$lock_key\"}},\"id\":7}" | nc -w1 $SYNAPSIS_SERVER
}

# Claim task
claim_task() {
    local task_type="$1"
    load_session
    heartbeat "claiming-task"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"task_claim\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"task_type\":\"$task_type\"}},\"id\":8}" | nc -w1 $SYNAPSIS_SERVER
}

# Main
case "$1" in
    register) register_session ;;
    heartbeat) load_session; heartbeat "$2" ;;
    save) save_context "$2" "$3" ;;
    search) search "$2" "$3" ;;
    context) get_global_context ;;
    lock-acquire) acquire_lock "$2" "$3" ;;
    lock-release) release_lock "$2" ;;
    claim) claim_task "$2" ;;
    *) echo "Synapsis MCP for Gemini CLI"
       echo "Usage: $0 {register|heartbeat|save|search|context|lock-acquire|lock-release|claim} [args]"
       echo ""
       echo "Examples:"
       echo "  $0 register                    # Register session"
       echo "  $0 heartbeat \"coding\"         # Send heartbeat"
       echo "  $0 save \"Bug Fix\" \"Fixed...\"  # Save context"
       echo "  $0 search \"authentication\"     # Search memory"
       echo "  $0 context                     # Get global context"
       echo "  $0 lock-acquire build 300     # Acquire build lock"
       echo "  $0 claim build                # Claim build task"
       ;;
esac
