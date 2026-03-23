#!/bin/bash
# Synapsis MCP - JetBrains IDE Integration Script

SYNAPSIS_SERVER="127.0.0.1:7438"
PROJECT_KEY="${1:-jetbrains-project}"
SESSION_ID=""

# Auto-register session
register_session() {
    local agent_type="jetbrains-ide"
    local instance=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || echo "unknown")
    local response=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"session_register\",\"params\":{\"agent_type\":\"$agent_type\",\"project\":\"$PROJECT_KEY\"},\"id\":1}" | nc -w1 $SYNAPSIS_SERVER)
    SESSION_ID=$(echo $response | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
    echo "Registered session: $SESSION_ID"
}

# Send heartbeat
heartbeat() {
    local task="$1"
    if [ -n "$SESSION_ID" ]; then
        echo "{\"jsonrpc\":\"2.0\",\"method\":\"agent_heartbeat\",\"params\":{\"arguments\":{\"session_id\":\"$SESSION_ID\",\"task\":\"$task\"}},\"id\":2}" | nc -w1 $SYNAPSIS_SERVER > /dev/null
    fi
}

# Save context
save_context() {
    local content="$1"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"chunk_create\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\",\"title\":\"IDE Context\",\"content\":\"$content\"}},\"id\":3}" | nc -w1 $SYNAPSIS_SERVER
}

# Search memory
search() {
    local query="$1"
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"memory_search_fts\",\"params\":{\"arguments\":{\"query\":\"$query\",\"project\":\"$PROJECT_KEY\",\"limit\":10}},\"id\":4}" | nc -w1 $SYNAPSIS_SERVER
}

# Get global context
get_global_context() {
    echo "{\"jsonrpc\":\"2.0\",\"method\":\"global_context_get\",\"params\":{\"arguments\":{\"project\":\"$PROJECT_KEY\"}},\"id\":5}" | nc -w1 $SYNAPSIS_SERVER
}

# Main
case "$1" in
    register) register_session ;;
    heartbeat) heartbeat "$2" ;;
    save) save_context "$2" ;;
    search) search "$2" ;;
    context) get_global_context ;;
    *) echo "Usage: $0 {register|heartbeat|save|search|context} [args]" ;;
esac
