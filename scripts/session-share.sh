#!/bin/bash
# Synapsis Session Share — Cross-platform session sharing CLI (Rust-native)
# Connects to Synapsis MCP server via JSON-RPC over HTTP.
set -euo pipefail

SYNAPSIS_URL="${SYNAPSIS_URL:-http://127.0.0.1:7438}"
MCP_BINARY="${MCP_BINARY:-/home/methodwhite/Proyectos/synapsis/target/release/synapsis-mcp}"

mcp_request() {
    local method="$1" data="$2"
    local payload; payload=$(cat <<EOF
{"jsonrpc":"2.0","method":"tools/call","params":{"name":"$method","arguments":${data:-{}}},"id":$$}
EOF
)
    curl -sf -X POST "$SYNAPSIS_URL/message" \
        -H "Content-Type: application/json" \
        -d "$payload" 2>/dev/null | python3 -c "
import sys,json
try:
    r=json.load(sys.stdin)
    for c in r.get('result',{}).get('content',[]):
        if c.get('type')=='text': print(c['text'])
except: print('ERROR' if not r.get('result') else r)
" 2>/dev/null || echo "{\"error\":\"Connection failed\"}"
}

cmd_status() {
    local info
    info=$(synapsis-autoconfig --dry-run 2>/dev/null | head -5)
    echo "Synapsis Session Bridge"
    echo "  MCP Binary: $MCP_BINARY"
    echo "  Endpoint:   $SYNAPSIS_URL/message"
    $MCP_BINARY --version 2>/dev/null && echo "  Status: ACTIVE" || echo "  Status: INACTIVE (binary not found)"
}

cmd_list()    { mcp_request "shared_sessions_list" "{}"; }
cmd_by_project() { mcp_request "shared_sessions_by_project" "{\"project\":\"$1\"}"; }
cmd_broadcast()  { mcp_request "shared_sessions_broadcast" "{\"session_id\":\"session-share\",\"observation\":\"$1\"}"; }

cmd_start() {
    local project="$1" dir="${2:-.}"
    mcp_request "mem_session_start" "{\"project\":\"$project\",\"directory\":\"$dir\",\"agent_id\":\"session-share\"}"
}

cmd_end() {
    local sid="$1" summary="${2:-}"
    if [ -n "$summary" ]; then
        mcp_request "mem_session_end" "{\"session_id\":\"$sid\",\"summary\":\"$summary\"}"
    else
        mcp_request "mem_session_end" "{\"session_id\":\"$sid\"}"
    fi
}

cmd_summary() { mcp_request "mem_session_summary" "{\"session_id\":\"$1\"}"; }

case "${1:-help}" in
    status)     cmd_status;;
    list)       cmd_list;;
    by-project) shift; cmd_by_project "$1";;
    broadcast)  shift; cmd_broadcast "$*";;
    start)      shift; cmd_start "$1" "${2:-.}";;
    end)        shift; cmd_end "$1" "${2:-}";;
    summary)    shift; cmd_summary "$1";;
    *)          echo "Usage: session-share.sh <status|list|by-project|broadcast|start|end|summary>"; exit 1;;
esac
