#!/bin/bash
# Synapsis YOLO Mode Auto-Start

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis YOLO Mode - Starting All Systems               ║"
echo "╚══════════════════════════════════════════════════════════╝"

# Start TCP server in background
./target/release/synapsis --tcp 7438 &
TCP_PID=$!
echo "✅ TCP Server started (PID: $TCP_PID)"

# Wait for server to be ready
sleep 2

# Test connection
echo '{"jsonrpc":"2.0","method":"agents_active","params":{},"id":1}' | nc -q1 127.0.0.1 7438 > /dev/null 2>&1 && echo "✅ TCP Server responding" || echo "❌ TCP Server not responding"

# Show status
echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  YOLO Mode Active - All Agents Autonomous                ║"
echo "║  TCP: 127.0.0.1:7438                                     ║"
echo "║  WebSocket: 127.0.0.1:8080 (when implemented)            ║"
echo "║  Health Monitor: Running (30s interval)                  ║"
echo "║  Auto-Task Gen: Active                                   ║"
echo "╚══════════════════════════════════════════════════════════╝"

wait $TCP_PID
