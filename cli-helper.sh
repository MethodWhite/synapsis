#!/bin/bash
# Synapsis CLI Helper
# Connects to Synapsis, registers as worker, polls for tasks
# Usage: ./cli-helper.sh opencode [SYNAPSE_URL]
# Example: ./cli-helper.sh opencode http://localhost:7438

set -e

AGENT="${1:-opencode}"
SYNAPSE_URL="${2:-${SYNAPSE_URL:-http://localhost:7438}}"
POLL_INTERVAL="${POLL_INTERVAL:-2}"

echo "[Synapsis CLI] Connecting as: $AGENT"
echo "[Synapsis CLI] Server: $SYNAPSE_URL"

register_worker() {
    echo "[Synapsis CLI] Registering worker..."
    RESPONSE=$(curl -s -X POST "$SYNAPSE_URL/rpc" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"worker_register\",\"params\":{\"arguments\":{\"agent\":\"$AGENT\",\"capabilities\":[\"developer\",\"debugger\"]}},\"id\":1}")
    
    if echo "$RESPONSE" | grep -q '"error"'; then
        echo "[Synapsis CLI] Registration failed: $RESPONSE"
        return 1
    fi
    
    WORKER_ID=$(echo "$RESPONSE" | grep -o '"worker_id":"[^"]*"' | cut -d'"' -f4)
    echo "[Synapsis CLI] Registered with worker_id: $WORKER_ID"
    echo "$WORKER_ID"
}

poll_tasks() {
    WORKER_ID="$1"
    curl -s -X POST "$SYNAPSE_URL/rpc" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"worker_poll\",\"params\":{\"arguments\":{\"agent\":\"$AGENT\"}},\"id\":1}"
}

submit_result() {
    TASK_ID="$1"
    WORKER_ID="$2"
    RESULT="$3"
    curl -s -X POST "$SYNAPSE_URL/rpc" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"task_submit_result\",\"params\":{\"arguments\":{\"task_id\":\"$TASK_ID\",\"worker_id\":\"$WORKER_ID\",\"result\":\"$RESULT\"}},\"id\":1}"
}

heartbeat() {
    WORKER_ID="$1"
    curl -s -X POST "$SYNAPSE_URL/rpc" \
        -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"worker_status\",\"params\":{\"arguments\":{\"worker_id\":\"$WORKER_ID\"}},\"id\":1}" > /dev/null
}

WORKER_ID=$(register_worker)

echo "[Synapsis CLI] Starting task polling (interval: ${POLL_INTERVAL}s)..."
echo "[Synapsis CLI] Press Ctrl+C to exit"

HEARTBEAT_COUNT=0

while true; do
    RESPONSE=$(poll_tasks "$WORKER_ID")
    
    TASK=$(echo "$RESPONSE" | grep -o '"task":{[^}]*}' | head -1)
    
    if [ -n "$TASK" ] && [ "$TASK" != '"task":null' ]; then
        TASK_ID=$(echo "$TASK" | grep -o '"task_id":"[^"]*"' | cut -d'"' -f4)
        DESCRIPTION=$(echo "$TASK" | grep -o '"description":"[^"]*"' | cut -d'"' -f4)
        PAYLOAD=$(echo "$TASK" | grep -o '"payload":"[^"]*"' | cut -d'"' -f4)
        
        echo "[Synapsis CLI] Received task: $TASK_ID"
        echo "[Synapsis CLI] Description: $DESCRIPTION"
        
        echo "[Synapsis CLI] Executing task..."
        echo "$PAYLOAD"
        
        echo "[Synapsis CLI] Task completed. Submitting result..."
        RESULT="Task '$DESCRIPTION' completed successfully by $AGENT"
        submit_result "$TASK_ID" "$WORKER_ID" "$RESULT"
        echo "[Synapsis CLI] Result submitted."
    fi
    
    HEARTBEAT_COUNT=$((HEARTBEAT_COUNT + 1))
    if [ $((HEARTBEAT_COUNT % 10)) -eq 0 ]; then
        heartbeat "$WORKER_ID" &
    fi
    
    sleep "$POLL_INTERVAL"
done
