#!/bin/bash
# Synapsis MCP Quick Install
# Detects opencode and installs the MCP config
set -euo pipefail

MCP_BINARY="/home/methodwhite/Proyectos/synapsis/target/release/synapsis-mcp"
CONFIG_DIR="${HOME}/.config/opencode"

if [ ! -x "$MCP_BINARY" ]; then
    echo "!! MCP binary not found: ${MCP_BINARY}" >&2
    echo "!! Build Synapsis first: cargo build --release" >&2
    exit 1
fi

echo "Synapsis MCP Install"
echo "  Binary: ${MCP_BINARY}"
echo ""

PYTHON=$(command -v python3 || command -v python || true)
if [ -z "$PYTHON" ]; then
    echo "!! python3 is required" >&2
    exit 1
fi

# Detect opencode
if command -v opencode &>/dev/null; then
    echo "-> Detected: opencode"
else
    echo "-> opencode not found in PATH, checking config directory anyway..."
fi

# Install for opencode
echo "-> Installing MCP config for OpenCode..."
mkdir -p "$CONFIG_DIR"
CONFIG_FILE="${CONFIG_DIR}/opencode.jsonc"

if [ -f "$CONFIG_FILE" ]; then
    cp "$CONFIG_FILE" "${CONFIG_FILE}.bak"
    echo "   Backup: ${CONFIG_FILE}.bak"
fi

"$PYTHON" -c "
import json, sys

filepath = '$CONFIG_FILE'
binary = '$MCP_BINARY'

try:
    with open(filepath) as f:
        data = json.load(f)
except (FileNotFoundError, json.JSONDecodeError):
    data = {}

data.setdefault('mcpServers', {})

existing = data['mcpServers'].get('synapsis', {})
if isinstance(existing, dict) and existing.get('command') == binary:
    print('   Already up to date')
    sys.exit(0)

data['mcpServers']['synapsis'] = {'command': binary, 'args': []}

with open(filepath, 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')

print('   Config written: ' + filepath)
"

echo ""
echo "ok Synapsis MCP configured for OpenCode:"
echo "   ${CONFIG_FILE}"
echo ""
echo "Test with: opencode"
