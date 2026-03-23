#!/bin/bash
# Synapsis - Claude Code Session Start Hook
# Runs automatically when Claude Code starts a new session

SYNAPSIS_BIN="${SYNAPSIS_BIN:-synapsis}"

# Start Synapsis server if not running
if ! pgrep -f "synapsis serve" > /dev/null 2>&1; then
    $SYNAPSIS_BIN serve &
    sleep 1
fi

# Register session start
$SYNAPSIS_BIN session start --agent claude-code 2>/dev/null || true

# Sync any remote changes
$SYNAPSIS_BIN sync --import 2>/dev/null || true

echo "Synapsis session started"
