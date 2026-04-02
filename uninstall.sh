#!/bin/bash
# Synapsis Uninstaller
# PROPRIETARY - All Rights Reserved

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis Uninstaller                                    ║"
echo "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Remove from /usr/local/bin
echo "🗑️  Removing /usr/local/bin/synapsis..."
sudo rm -f /usr/local/bin/synapsis
sudo rm -f /usr/local/bin/synapsis-mcp

# Remove from ~/.local/bin
echo "🗑️  Removing ~/.local/bin/synapsis..."
rm -f ~/.local/bin/synapsis
rm -f ~/.local/bin/synapsis-mcp

# Remove aliases
echo "🗑️  Removing aliases from shell configs..."
sed -i '/alias synapsis=/d' ~/.bashrc 2>/dev/null || true
sed -i '/alias synapsis=/d' ~/.zshrc 2>/dev/null || true

hash -r

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Uninstall Complete ✅                                   ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
