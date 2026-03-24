#!/bin/bash
cd "$(dirname "$0")"

echo "Testing MCP TCP server mode (insecure)"

# Kill any existing server on port 7439
lsof -ti:7439 | xargs kill -9 2>/dev/null

# Start MCP server in TCP mode (insecure)
echo "Starting server: ./target/debug/synapsis-mcp --tcp --insecure"
./target/debug/synapsis-mcp --tcp --insecure &
SERVER_PID=$!

# Wait for server to start
sleep 3

echo "Server PID: $SERVER_PID"

# Test connection with initialize request
echo "Testing connection with initialize request..."
REQUEST='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"tcp-test"}}}'
RESPONSE=$(echo "$REQUEST" | nc -w2 127.0.0.1 7439 2>/dev/null)

if [ -n "$RESPONSE" ]; then
    echo "✅ TCP server responded:"
    echo "$RESPONSE" | head -1
    
    # Check if response contains expected fields
    if echo "$RESPONSE" | grep -q '"jsonrpc":"2.0"' && echo "$RESPONSE" | grep -q '"result"'; then
        echo "✅ TCP server is working correctly"
    else
        echo "❌ TCP server response missing expected fields"
    fi
else
    echo "❌ TCP server did not respond"
fi

# Kill server
echo "Killing server (PID: $SERVER_PID)"
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

echo "Test complete"