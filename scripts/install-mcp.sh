#!/bin/bash
# Synapsis MCP Quick Install
# Detects opencode and installs the MCP config
set -euo pipefail

# Resolve the MCP binary: prefer the installed PATH entry (stable across
# machines), fall back to this repo's release build. Never hardcode a
# machine-specific path.
if command -v synapsis-mcp &>/dev/null; then
    MCP_BINARY="synapsis-mcp"
elif [ -x "$(dirname "${BASH_SOURCE[0]}")/../target/release/synapsis-mcp" ]; then
    MCP_BINARY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target/release/synapsis-mcp"
else
    echo "!! synapsis-mcp not found in PATH and no release build in this repo" >&2
    echo "!! Build Synapsis first: cargo build --release --bin synapsis-mcp" >&2
    exit 1
fi

CONFIG_DIR="${HOME}/.config/opencode"

echo "Synapsis MCP Install"
echo "  Binary: ${MCP_BINARY}"
echo ""

JQ=$(command -v jq || true)
if [ -z "$JQ" ]; then
    echo "!! jq is required (install with: sudo pacman -S jq)" >&2
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

PYTHON_BIN=$(command -v python3 || command -v python || true)
if [ -z "$PYTHON_BIN" ]; then
    echo "!! python3 is required" >&2
    exit 1
fi

"$PYTHON_BIN" -c "
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
