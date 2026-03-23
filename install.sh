#!/bin/bash
# Synapsis Multi-Platform Installer
# PROPRIETARY - All Rights Reserved
# Supports: Linux, Windows (WSL), macOS, BSD, Android (Termux)

set -e

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis Multi-Platform Installer                       ║"
echo "║  PROPRIETARY SOFTWARE - LICENSED, NOT SOLD               ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Detect platform
PLATFORM=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

echo "📊 Detected platform: $PLATFORM ($ARCH)"

# Install Rust based on platform
install_rust() {
    if ! command -v rustc &> /dev/null; then
        echo "📦 Installing Rust..."
        case $PLATFORM in
            linux*|android*)
                curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                ;;
            darwin*)
                if command -v brew &> /dev/null; then
                    brew install rustup
                else
                    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                fi
                ;;
            *bsd*)
                case $PLATFORM in
                    freebsd*)
                        pkg install rust
                        ;;
                    openbsd*)
                        pkg_add rust
                        ;;
                    netbsd*)
                        pkgin install rust
                        ;;
                esac
                ;;
            msys*|mingw*|cygwin*)
                echo "⚠️  For Windows native, use install.ps1 instead"
                echo "   For WSL, continue with Linux installation..."
                curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                ;;
        esac
    else
        echo "✅ Rust already installed"
    fi
}

# Build Synapsis
build_synapsis() {
    echo "📦 Building Synapsis..."
    cd /home/methodwhite/Projects/synapsis
    cargo build --release
    
    # Install to local bin
    mkdir -p ~/.local/bin
    cp target/release/synapsis ~/.local/bin/
    chmod +x ~/.local/bin/synapsis
}

# Create aliases
create_aliases() {
    echo "🔧 Creating aliases..."
    case $SHELL in
        */zsh)
            echo "alias synapsis='~/.local/bin/synapsis'" >> ~/.zshrc
            source ~/.zshrc
            ;;
        */bash)
            echo "alias synapsis='~/.local/bin/synapsis'" >> ~/.bashrc
            source ~/.bashrc
            ;;
    esac
}

# Main installation
install_rust
build_synapsis
create_aliases

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Installation Complete ✅                                ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "Binary: ~/.local/bin/synapsis"
echo "Aliases: synapsis"
echo ""
echo "Usage:"
echo "  synapsis --help"
echo "  synapsis mcp"
echo ""
