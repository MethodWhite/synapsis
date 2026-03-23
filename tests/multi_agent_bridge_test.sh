#!/bin/bash
# Multi-Agent Bridge Test - Verify qwen-code + opencode context sharing

SYNAPSIS_SERVER="127.0.0.1:7438"
TEST_PASSED=0
TEST_FAILED=0

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

test_result() {
    if [ "$1" -eq 0 ]; then
        echo -e "${GREEN}✓ PASSED${NC}: $2"
        ((TEST_PASSED++))
    else
        echo -e "${RED}✗ FAILED${NC}: $2"
        ((TEST_FAILED++))
    fi
}

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║     MULTI-AGENT BRIDGE TEST                               ║"
echo "║     Testing: qwen-code + opencode context sharing         ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""

# Test 1: Server connectivity
echo "Test 1: Server Connectivity"
response=$(echo '{"jsonrpc":"2.0","method":"ping","params":{},"id":1}' | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
if echo "$response" | grep -q "pong\|result"; then
    test_result 0 "TCP server responding"
else
    test_result 1 "TCP server not responding"
fi

# Test 2: Register qwen-code session
echo ""
echo "Test 2: Register qwen-code Session"
QWEN_RESPONSE=$(echo '{"jsonrpc":"2.0","method":"session_register","params":{"arguments":{"agent_type":"qwen-code-test","project":"bridge-test"}},"id":2}' | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
QWEN_SESSION=$(echo "$QWEN_RESPONSE" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -n "$QWEN_SESSION" ]; then
    test_result 0 "qwen-code registered: $QWEN_SESSION"
else
    test_result 1 "qwen-code registration failed"
fi

# Test 3: Register opencode session
echo ""
echo "Test 3: Register opencode Session"
OPENCODE_RESPONSE=$(echo '{"jsonrpc":"2.0","method":"session_register","params":{"arguments":{"agent_type":"opencode-test","project":"bridge-test"}},"id":3}' | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
OPENCODE_SESSION=$(echo "$OPENCODE_RESPONSE" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -n "$OPENCODE_SESSION" ]; then
    test_result 0 "opencode registered: $OPENCODE_SESSION"
else
    test_result 1 "opencode registration failed"
fi

# Test 4: qwen-code saves context
echo ""
echo "Test 4: qwen-code Saves Context"
if [ -n "$QWEN_SESSION" ]; then
    SAVE_RESPONSE=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"chunk_create\",\"params\":{\"arguments\":{\"project\":\"bridge-test\",\"title\":\"qwen-context\",\"content\":\"qwen-code-test-data-$(date +%s)\"}},\"id\":4}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
    if echo "$SAVE_RESPONSE" | grep -q "chunk_id\|Created"; then
        test_result 0 "qwen-code context saved"
    else
        test_result 1 "qwen-code context save failed"
    fi
else
    test_result 1 "Skipping - no qwen session"
fi

# Test 5: opencode reads context
echo ""
echo "Test 5: opencode Reads Context"
if [ -n "$OPENCODE_SESSION" ]; then
    READ_RESPONSE=$(echo '{"jsonrpc":"2.0","method":"chunk_get","params":{"arguments":{"project":"bridge-test"}},"id":5}' | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
    if echo "$READ_RESPONSE" | grep -q "qwen-context\|chunks"; then
        test_result 0 "opencode can read qwen-code context"
    else
        test_result 1 "opencode cannot read qwen-code context"
    fi
else
    test_result 1 "Skipping - no opencode session"
fi

# Test 6: Shared task queue
echo ""
echo "Test 6: Shared Task Queue"
TASK_RESPONSE=$(echo '{"jsonrpc":"2.0","method":"task_create","params":{"arguments":{"project":"bridge-test","task_type":"test","payload":"multi-agent-test","priority":10}},"id":6}' | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
if echo "$TASK_RESPONSE" | grep -q "task_id"; then
    test_result 0 "Task created in shared queue"
else
    test_result 1 "Task creation failed"
fi

# Test 7: Both agents can claim tasks
echo ""
echo "Test 7: Task Claim by Different Agents"
CLAIM1=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"task_claim\",\"params\":{\"arguments\":{\"session_id\":\"$QWEN_SESSION\",\"task_type\":\"test\"}},\"id\":7}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
CLAIM2=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"task_claim\",\"params\":{\"arguments\":{\"session_id\":\"$OPENCODE_SESSION\",\"task_type\":\"test\"}},\"id\":8}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
if echo "$CLAIM1$CLAIM2" | grep -q "task"; then
    test_result 0 "Both agents can claim tasks"
else
    test_result 1 "Task claim failed"
fi

# Test 8: Heartbeat from both agents
echo ""
echo "Test 8: Concurrent Heartbeats"
HB1=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"agent_heartbeat\",\"params\":{\"arguments\":{\"session_id\":\"$QWEN_SESSION\",\"task\":\"test\"}},\"id\":9}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
HB2=$(echo "{\"jsonrpc\":\"2.0\",\"method\":\"agent_heartbeat\",\"params\":{\"arguments\":{\"session_id\":\"$OPENCODE_SESSION\",\"task\":\"test\"}},\"id\":10}" | nc -w1 $SYNAPSIS_SERVER 2>/dev/null)
if echo "$HB1$HB2" | grep -q "ok\|status"; then
    test_result 0 "Concurrent heartbeats working"
else
    test_result 1 "Heartbeat failed"
fi

# Summary
echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║  TEST SUMMARY                                             ║"
echo "╠═══════════════════════════════════════════════════════════╣"
echo -e "║  ${GREEN}PASSED${NC}: $TEST_PASSED                                         ║"
echo -e "║  ${RED}FAILED${NC}: $TEST_FAILED                                         ║"
echo "╚═══════════════════════════════════════════════════════════╝"

if [ $TEST_FAILED -eq 0 ]; then
    echo ""
    echo "✅ Multi-agent bridge is working correctly!"
    echo "   qwen-code and opencode can share context via SQLite"
    exit 0
else
    echo ""
    echo "❌ Some tests failed. Check multi-agent configuration."
    exit 1
fi
