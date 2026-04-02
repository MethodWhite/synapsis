#!/bin/bash
# Synapsis macOS Installer
# PROPRIETARY - All Rights Reserved

set -e

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis macOS Installer                                ║"
echo "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Check Homebrew
if ! command -v brew &> /dev/null; then
    echo "📦 Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

# Check Rust
if ! command -v rustc &> /dev/null; then
    echo "📦 Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Build Synapsis
echo "📦 Building Synapsis..."
cd "$(dirname "$(realpath "$0" 2>/dev/null || echo "$0")")"
cargo build --release

# Install to /usr/local/bin
sudo install -m 755 target/release/synapsis /usr/local/bin/synapsis
sudo install -m 755 target/release/synapsis-mcp /usr/local/bin/synapsis-mcp

# Create aliases
echo "🔧 Creating aliases..."
cat >> ~/.zshrc << 'ZSHRC'

# Synapsis Aliases
alias synapsis='synapsis'
ZSHRC

source ~/.zshrc

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Installation Complete ✅                                ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Binaries: /usr/local/bin/synapsis, /usr/local/bin/synapsis-mcp"
echo "Aliases: synapsis"
echo ""
echo "Usage:"
echo "  synapsis --help"
echo "  synapsis mcp"
echo ""
