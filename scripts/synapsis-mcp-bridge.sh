#!/bin/bash
# Synapsis MCP Bridge - Connect any MCP client to Synapsis TCP server
# Usage: ./synapsis-mcp-bridge.sh [--url HOST:PORT]

SYNAPSIS_URL="${SYNAPSIS_URL:-127.0.0.1:7438}"

while [[ $# -gt 0 ]]; do
    case $1 in
        --url)
            SYNAPSIS_URL="$2"
            shift 2
            ;;
        --help|-h)
            echo "Synapsis MCP Bridge v1.0"
            echo "Connect MCP clients to Synapsis TCP server"
            echo ""
            echo "Usage: $0 [--url HOST:PORT]"
            echo "       SYNAPSIS_URL=host:port $0"
            exit 0
            ;;
        *)
            shift
            ;;
    esac
done

# Parse JSON response to extract session_id
parse_json() {
    local json="$1"
    local key="$2"
    echo "$json" | sed -n "s/.*\"$key\":\"\\([^\"]*\\)\".*/\\1/p" | head -1
}

# Register as MCP client
REGISTER_RESP=$(echo '{"jsonrpc":"2.0","method":"session_register","params":{"arguments":{"agent_type":"mcp-client","project":"default"}},"id":1}' | nc -w 2 "$SYNAPSIS_URL")
SESSION=$(parse_json "$REGISTER_RESP" "session_id")

if [ -z "$SESSION" ] || [ "$SESSION" = "" ]; then
    echo '{"error":"Failed to connect to Synapsis at '$SYNAPSIS_URL'"}' >&2
    exit 1
fi

echo "Connected to Synapsis as $SESSION" >&2

# Heartbeat loop in background
(
    while true; do
        echo '{"jsonrpc":"2.0","method":"agent_heartbeat","params":{"arguments":{"session_id":"'$SESSION'"}},"id":"h"}' | nc -w 1 "$SYNAPSIS_URL" > /dev/null 2>&1
        sleep 20
    done
) &
HEARTBEAT_PID=$!

# Trap to clean up
trap "kill $HEARTBEAT_PID 2>/dev/null; exit" EXIT INT TERM

# Relay MCP stdin/stdout
while IFS= read -r line; do
    if [ -n "$line" ]; then
        echo "$line" | nc -w 2 "$SYNAPSIS_URL"
    fi
done

kill $HEARTBEAT_PID 2>/dev/null
