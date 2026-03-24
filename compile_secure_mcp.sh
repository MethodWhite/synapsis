#!/bin/bash
# Compilation script for Secure MCP Server
# Usage: ./compile_secure_mcp.sh [--release]

set -e

echo "🔧 Synapsis Secure MCP Compilation Script"
echo "=========================================="

# Check for Rust
if ! command -v rustc &> /dev/null; then
    echo "❌ Rust not found. Please install Rust 1.75+"
    echo "   Visit: https://rustup.rs/"
    exit 1
fi

# Check Rust version
RUST_VERSION=$(rustc --version | grep -o '[0-9]\+\.[0-9]\+' | head -1)
echo "✅ Rust version: $RUST_VERSION"

# Parse arguments
BUILD_MODE="debug"
if [[ "$1" == "--release" ]]; then
    BUILD_MODE="release"
    BUILD_FLAG="--release"
    echo "📦 Building in RELEASE mode (optimized)"
else
    echo "🐛 Building in DEBUG mode (faster compilation)"
fi

# Check dependencies
echo "📋 Checking dependencies..."
if ! cargo check --quiet 2>/dev/null; then
    echo "⚠️  Some dependencies may need updating..."
    echo "   Running cargo update..."
    cargo update
fi

# Build
echo "🔨 Building synapsis-mcp binary..."
if [[ "$BUILD_MODE" == "release" ]]; then
    cargo build --release --bin synapsis-mcp
    BINARY_PATH="./target/release/synapsis-mcp"
else
    cargo build --bin synapsis-mcp
    BINARY_PATH="./target/debug/synapsis-mcp"
fi

# Verify build
if [[ -f "$BINARY_PATH" ]]; then
    echo "✅ Build successful!"
    echo "📁 Binary: $BINARY_PATH"
    
    # Show binary info
    echo "📊 Binary information:"
    ls -lh "$BINARY_PATH"
    
    # Test help output
    echo "🧪 Testing binary..."
    if timeout 2 "$BINARY_PATH" --help 2>&1 | grep -q "secure"; then
        echo "✅ Secure features detected in binary"
    else
        echo "⚠️  Secure features may not be included"
        echo "   Try: $BINARY_PATH --help"
    fi
    
    # Update MCP config if it exists
    MCP_CONFIG="$HOME/.config/aichat/mcp.yaml"
    if [[ -f "$MCP_CONFIG" ]]; then
        echo "⚙️  Found MCP configuration at: $MCP_CONFIG"
        echo "   Current config:"
        grep -A2 "synapsis" "$MCP_CONFIG" || true
        
        echo ""
        read -p "Update config to use secure mode? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            BACKUP="$MCP_CONFIG.backup.$(date +%s)"
            cp "$MCP_CONFIG" "$BACKUP"
            echo "📋 Backup created: $BACKUP"
            
            # Update config
            sed -i 's|args: \[\]|args: \["--tcp", "--secure"\]|g' "$MCP_CONFIG"
            echo "✅ Configuration updated to use secure mode"
            echo "   Restart your MCP client to apply changes"
        fi
    fi
    
    # Create test commands
    echo ""
    echo "🚀 Test Commands:"
    echo "-----------------"
    echo "1. Start secure server:"
    echo "   $BINARY_PATH --tcp --secure --tcp-addr 127.0.0.1:7439"
    echo ""
    echo "2. Connect client:"
    echo "   $BINARY_PATH --bridge --url 127.0.0.1:7439 --secure"
    echo ""
    echo "3. Local testing (insecure):"
    echo "   $BINARY_PATH --tcp --insecure --tcp-addr 127.0.0.1:7440"
    echo ""
    echo "📚 Full documentation: see SECURE_MCP_SETUP.md"
    
else
    echo "❌ Build failed!"
    echo "   Check errors above"
    exit 1
fi

echo ""
echo "🎉 Done! Secure MCP server is ready."
echo "   Remember: Use --secure flag for production, --insecure only for debugging."