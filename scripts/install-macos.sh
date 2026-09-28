#!/bin/bash
# Synapsis Installer - macOS (Intel + Apple Silicon)
set -e

# Resolve the latest release unless explicitly pinned with SYNAPSIS_VERSION,
# so installers always fetch the newest binary instead of a stale tag.
if [ -z "${SYNAPSIS_VERSION:-}" ]; then
    VERSION="$(command -v curl >/dev/null 2>&1 && curl -fsSL "https://api.github.com/repos/methodwhite/synapsis/releases/latest" 2>/dev/null | grep -m1 '"tag_name"' | sed -E 's/.*"v([^"]+)".*/\1/')"
    VERSION="${VERSION:-0.12.0}"
else
    VERSION="${SYNAPSIS_VERSION}"
fi
REPO="methodwhite/synapsis"
INSTALL_DIR="${HOME}/.local/bin"

echo "Synapsis v${VERSION} Installer (macOS)"
echo "======================================="
echo ""

ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  TARGET="x86_64-apple-darwin" ;;
    arm64)   TARGET="aarch64-apple-darwin" ;;
    *)       echo "Unsupported architecture: $ARCH" && exit 1 ;;
esac

echo "Platform: macOS (${ARCH}) -> ${TARGET}"
echo "Install dir: ${INSTALL_DIR}"
echo ""

mkdir -p "${INSTALL_DIR}"

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/v${VERSION}/synapsis-${TARGET}.tar.gz"
INSTALLED=0

# ── Download ─────────────────────────────────────────────
if command -v curl &>/dev/null; then
    echo "Downloading synapsis v${VERSION}..."
    TMPDIR=$(mktemp -d)
    trap "rm -rf ${TMPDIR}" EXIT
    if curl -fsSL "${DOWNLOAD_URL}" -o "${TMPDIR}/synapsis.tar.gz"; then
        tar xzf "${TMPDIR}/synapsis.tar.gz" -C "${TMPDIR}"
        cp "${TMPDIR}"/synapsis/synapsis* "${INSTALL_DIR}/" 2>/dev/null || true
        chmod +x "${INSTALL_DIR}"/synapsis* 2>/dev/null || true
        xattr -d com.apple.quarantine "${INSTALL_DIR}"/synapsis* 2>/dev/null || true
        INSTALLED=1
    fi
fi

# ── Build from source ────────────────────────────────────
if [ "$INSTALLED" = "0" ]; then
    echo "No pre-built binary found. Building from source..."
    if ! command -v rustc &>/dev/null; then
        echo "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "${HOME}/.cargo/env"
    fi
    if ! xcode-select -p &>/dev/null; then
        echo "Installing Xcode Command Line Tools..."
        xcode-select --install
        echo "After Xcode CLI tools install completes, re-run this script."
        exit 1
    fi
    TMPDIR=$(mktemp -d)
    trap "rm -rf ${TMPDIR}" EXIT
    git clone --depth 1 "https://github.com/${REPO}.git" "${TMPDIR}/synapsis"
    cd "${TMPDIR}/synapsis"
    cargo build --release
    cp target/release/synapsis target/release/synapsis-mcp target/release/synapsis-server "${INSTALL_DIR}/"
    chmod +x "${INSTALL_DIR}/synapsis" "${INSTALL_DIR}/synapsis-mcp" "${INSTALL_DIR}/synapsis-server"
    INSTALLED=1
fi

# ── PATH check ────────────────────────────────────────────
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo "Add to ~/.zshrc or ~/.bash_profile: export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

# ── MCP auto-config ──────────────────────────────────────
MCP_CONFIG='{"mcpServers":{"synapsis":{"command":"synapsis-mcp","args":[]}}}'

if command -v claude &>/dev/null; then
    mkdir -p "${HOME}/.claude" 2>/dev/null
    CFG="${HOME}/.claude/settings.json"
    if [ -f "$CFG" ]; then
        python3 -c "
import json
with open('$CFG') as f: c = json.load(f)
c.setdefault('mcpServers',{})['synapsis'] = {'command':'synapsis-mcp','args':[]}
with open('$CFG','w') as f: json.dump(c,f,indent=2)
" 2>/dev/null && echo "  ✅ Claude Code configured" || true
    else
        echo "$MCP_CONFIG" > "$CFG" 2>/dev/null && echo "  ✅ Claude Code configured"
    fi
fi

for app in cursor windsurf; do
    for dir in "${HOME}/.config/${app}" "${HOME}/.${app}"; do
        [ -d "$dir" ] || continue
        MCPF="${dir}/mcp.json"; [ -f "$MCPF" ] && break
        echo "$MCP_CONFIG" > "$MCPF" 2>/dev/null && echo "  ✅ ${app^} configured" && break
    done
done

if command -v opencode &>/dev/null; then
    MCPF="${HOME}/.opencode.json"
    [ ! -f "$MCPF" ] && echo "$MCP_CONFIG" > "$MCPF" 2>/dev/null && echo "  ✅ OpenCode configured"
fi

echo ""
echo "✅ Synapsis v${VERSION} installed!"
echo "   Run: synapsis-mcp"
exit 0
