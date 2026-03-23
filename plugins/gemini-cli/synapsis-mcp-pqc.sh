#!/bin/bash
# Synapsis MCP - Gemini CLI Integration with PQC Support

SYNAPSIS_SERVER="${SYNAPSIS_SERVER:-127.0.0.1:7438}"
PROJECT_KEY="${SYNAPSIS_PROJECT:-gemini-cli}"
SESSION_ID=""
SESSION_FILE="/tmp/synapsis-gemini-session"
PQC_ENABLED=false

check_pqc() {
    local response=$(echo '{"jsonrpc":"2.0","method":"pqc_status","params":{},"id":1}' | nc -q1 $SYNAPSIS_SERVER 2>/dev/null)
    if echo "$response" | grep -q '"enabled":true'; then
        PQC_ENABLED=true
        echo "PQC support enabled"
    else
        echo "PQC not available"
    fi
}

load_session() {
    if [ -f "$SESSION_FILE" ]; then
        SESSION_ID=$(cat "$SESSION_FILE")
    else
        register_session
    fi
}

register_session() {
    local agent_type="gemini-cli"
    local instance=$(hostname)-$$
    local response=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"session_register\",\"params\":{\"agent_type\":\"$agent_type\",\"project\":\"$PROJECT_KEY\"},\"id\":1}" | nc -q1 $SYNAPSIS_SERVER 2>/dev/null)
    SESSION_ID=$(echo "$response" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
    if [ -n "$SESSION_ID" ]; then
        echo "$SESSION_ID" > "$SESSION_FILE"
        echo "Registered: $SESSION_ID"
    fi
}

heartbeat() {
    local task="$1"
    if [ -n "$SESSION_ID" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"method\":\"agent_heartbeat\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"task\":\"$task\"}},\"id\":2}" | nc -q1 $SYNAPSIS_SERVER > /dev/null 2>&1
    fi
}

save_context() {
    local title="$1"
    local content="$2"
    load_session
    heartbeat "saving-context"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"chunk_create\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\",\"title\":\"$title\",\"content\":\"$content\"}},\"id\":3}" | nc -q1 $SYNAPSIS_SERVER
}

search() {
    local query="$1"
    local limit="${2:-10}"
    load_session
    heartbeat "searching"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"memory_search_fts\",\"params\":{\"arguments\":{\"query\":\"$query\",\"project\":\"$PROJECT_KEY\",\"limit\":$limit}},\"id\":4}" | nc -q1 $SYNAPSIS_SERVER
}

get_context() {
    load_session
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"global_context_get\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\"}},\"id\":5}" | nc -q1 $SYNAPSIS_SERVER
}

acquire_lock() {
    local lock_key="$1"
    local ttl="${2:-300}"
    load_session
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"lock_acquire\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"lock_key\":\"$lock_key\",\"ttl\":$ttl}},\"id\":6}" | nc -q1 $SYNAPSIS_SERVER
}

release_lock() {
    local lock_key="$1"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"lock_release\",\"params\":{\"arguments\":{\"lock_key\":\"$lock_key\"}},\"id\":7}" | nc -q1 $SYNAPSIS_SERVER
}

claim_task() {
    local task_type="$1"
    load_session
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"task_claim\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"task_type\":\"$task_type\"}},\"id\":8}" | nc -q1 $SYNAPSIS_SERVER
}

export_context() {
    load_session
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"context_export\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\"}},\"id\":9}" | nc -q1 $SYNAPSIS_SERVER
}

case "$1" in
    register) register_session ;;
    heartbeat) load_session; heartbeat "$2" ;;
    save) save_context "$2" "$3" ;;
    search) search "$2" "$3" ;;
    context) get_context ;;
    lock-acquire) acquire_lock "$2" "$3" ;;
    lock-release) release_lock "$2" ;;
    claim) claim_task "$2" ;;
    export) export_context ;;
    pqc-status) check_pqc ;;
    *) echo "Synapsis MCP for Gemini CLI (PQC Ready)"
       echo "Usage: $0 {register|heartbeat|save|search|context|lock-acquire|lock-release|claim|export|pqc-status} [args]" ;;
esac
