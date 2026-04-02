#!/bin/bash
# Synapsis macOS Uninstaller
# PROPRIETARY - All Rights Reserved

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis macOS Uninstaller                              ║"
echo "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

echo "🗑️  Removing /usr/local/bin/synapsis..."
sudo rm -f /usr/local/bin/synapsis
sudo rm -f /usr/local/bin/synapsis-mcp

hash -r

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Uninstall Complete ✅                                   ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
