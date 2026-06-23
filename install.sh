#!/bin/bash
# Synapsis Installer - Linux, WSL, BSD, Android (Termux)
# Pure Rust - No Python dependencies
set -e

VERSION="${SYNAPSIS_VERSION:-0.8.2}"
REPO="methodwhite/synapsis"
INSTALL_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/synapsis"

echo "Synapsis v${VERSION} Installer"
echo "================================"
echo ""

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64|amd64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
    armv7l)        TARGET="armv7-unknown-linux-gnueabihf" ;;
    *)             echo "Unsupported architecture: $ARCH" && exit 1 ;;
esac

echo "Platform: ${OS} (${ARCH}) -> ${TARGET}"
echo "Install dir: ${INSTALL_DIR}"
echo "Data dir: ${DATA_DIR}"
echo ""

# Create directories
mkdir -p "${INSTALL_DIR}"
mkdir -p "${DATA_DIR}"

# Check if Rust is installed (for source build fallback)
have_rust() { command -v rustc &>/dev/null; }

# Try download pre-built binary from GitHub releases
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/synapsis-${TARGET}.tar.gz"

install_from_release() {
    echo "Downloading synapsis v${VERSION} for ${TARGET}..."
    local tmpdir=$(mktemp -d)
    trap "rm -rf ${tmpdir}" EXIT
    
    if curl -fsSL "${DOWNLOAD_URL}" -o "${tmpdir}/synapsis.tar.gz"; then
        cd "${tmpdir}"
        tar xzf synapsis.tar.gz
        cp synapsis/synapsis synapsis/synapsis-mcp "${INSTALL_DIR}/" 2>/dev/null || true
        chmod +x "${INSTALL_DIR}/synapsis" "${INSTALL_DIR}/synapsis-mcp" 2>/dev/null || true
        echo ""
        echo "Synapsis v${VERSION} installed successfully!"
        return 0
    fi
    return 1
}

install_from_source() {
    echo ""
    echo "No pre-built binary found. Building from source..."
    
    if ! have_rust; then
        echo "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "${HOME}/.cargo/env"
    fi
    
    local tmpdir=$(mktemp -d)
    trap "rm -rf ${tmpdir}" EXIT
    
    git clone --depth 1 --branch "v${VERSION}" "https://github.com/${REPO}.git" "${tmpdir}/synapsis" 2>/dev/null || \
    git clone --depth 1 "https://github.com/${REPO}.git" "${tmpdir}/synapsis"
    
    cd "${tmpdir}/synapsis"
    cargo build --release
    
    cp target/release/synapsis target/release/synapsis-mcp "${INSTALL_DIR}/"
    chmod +x "${INSTALL_DIR}/synapsis" "${INSTALL_DIR}/synapsis-mcp"
    
    echo ""
    echo "Synapsis built and installed from source!"
}

# Try release first, fallback to source
if ! install_from_release; then
    if have_rust || [ "${BUILD_FROM_SOURCE:-}" = "1" ]; then
        install_from_source
    else
        echo ""
        echo "No pre-built binary for ${TARGET} and Rust not found."
        echo "Install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo "Then re-run this script."
        exit 1
    fi
fi

# Check PATH
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
    echo ""
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc"
    echo "  source ~/.bashrc"
fi

# Verify
echo ""
"${INSTALL_DIR}/synapsis" --version 2>/dev/null || echo "(version check skipped - binary may need re-login)"

echo ""
echo "Done! Run 'synapsis mcp' to start the MCP server."
echo "Run 'synapsis update' to check for updates."
echo ""
echo "Configure your IDE/CLI MCP client to use:"
echo "  command: ${INSTALL_DIR}/synapsis"
echo "  args: [\"mcp\"]"
