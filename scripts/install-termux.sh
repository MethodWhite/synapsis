#!/bin/bash
# Synapsis Installer - Android (Termux)
# Pure Rust - No Python dependencies
set -e

# Resolve the latest release unless explicitly pinned with SYNAPSIS_VERSION,
# so installers always fetch the newest binary instead of a stale tag.
if [ -z "${SYNAPSIS_VERSION:-}" ]; then
    VERSION="$(command -v curl >/dev/null 2>&1 && curl -fsSL "https://api.github.com/repos/methodwhite/synapsis/releases/latest" 2>/dev/null | grep -m1 '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')"
    VERSION="${VERSION:-0.12.0}"
else
    VERSION="${SYNAPSIS_VERSION}"
fi
INSTALL_DIR="${HOME}/.local/bin"
DATA_DIR="${HOME}/.local/share/synapsis"

echo "Synapsis v${VERSION} Installer (Android/Termux)"
echo "================================================"
echo ""

ARCH=$(uname -m)
echo "Architecture: ${ARCH}"
echo ""

# Termux-specific setup
setup_termux() {
    echo "Setting up Termux environment..."
    pkg update -y
    pkg install -y git rust binutils make openssl sqlite
}

# Build from source (most reliable for Termux)
build_from_source() {
    echo "Building synapsis from source..."
    
    if ! command -v rustc &>/dev/null; then
        pkg install -y rust
    fi
    
    mkdir -p "${INSTALL_DIR}" "${DATA_DIR}"
    
    TMPDIR=$(mktemp -d)
    trap "rm -rf ${TMPDIR}" EXIT
    
    git clone --depth 1 "https://github.com/methodwhite/synapsis.git" "${TMPDIR}/synapsis"
    cd "${TMPDIR}/synapsis"
    
    cargo build --release
    
    cp target/release/synapsis target/release/synapsis-mcp "${INSTALL_DIR}/"
    chmod +x "${INSTALL_DIR}/synapsis" "${INSTALL_DIR}/synapsis-mcp"
    
    echo ""
    echo "Synapsis installed to ${INSTALL_DIR}"
}

# Detect if running in Termux
if [ -d "/data/data/com.termux" ] || [ -n "$TERMUX_VERSION" ]; then
    setup_termux
fi

build_from_source

# PATH setup
if ! echo "${PATH}" | grep -q "${INSTALL_DIR}"; then
    echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "${HOME}/.bashrc"
    echo "Added ${INSTALL_DIR} to PATH"
fi

echo ""
echo "Done! Run 'synapsis mcp' to start the MCP server."
echo "Run 'synapsis update' to check for updates."
