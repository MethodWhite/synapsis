#!/bin/bash
# Synapsis Secure TCP Server - Start Script
# Implements challenge-response authentication (HMAC-SHA256)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_SCRIPT="$SCRIPT_DIR/secure_tcp_server.py"

# Default API key (CHANGE IN PRODUCTION!)
DEFAULT_API_KEY="synapsis-$(hostname)-$(date +%Y%m%d)-secure-key"

# Load API keys from environment or use default
if [ -z "$SYNAPSIS_API_KEYS" ]; then
    if [ -f "$HOME/.synapsis_api_keys" ]; then
        export SYNAPSIS_API_KEYS=$(cat "$HOME/.synapsis_api_keys")
        echo "[Security] Loaded API keys from ~/.synapsis_api_keys"
    else
        export SYNAPSIS_API_KEYS="$DEFAULT_API_KEY"
        echo "[Security] Using default API key (CHANGE IN PRODUCTION!)"
        echo "           Save your own key to ~/.synapsis_api_keys"
    fi
fi

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis Secure TCP Server                              ║"
echo "║  Challenge-Response Auth (HMAC-SHA256)                   ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Check Python
if ! command -v python3 &> /dev/null; then
    echo "❌ Python3 is required but not installed"
    exit 1
fi

# Check DB
DB_PATH="$HOME/.local/share/synapsis/synapsis.db"
if [ ! -f "$DB_PATH" ]; then
    echo "⚠️  Synapsis database not found, will be created on first use"
fi

echo "✅ Script: $SERVER_SCRIPT"
echo "✅ Database: $DB_PATH"
echo "✅ API Keys: ${SYNAPSIS_API_KEYS:0:20}..."
echo ""
echo "Starting secure TCP server on 127.0.0.1:7438"
echo "Press Ctrl+C to stop"
echo ""

# Start server
exec python3 "$SERVER_SCRIPT"
