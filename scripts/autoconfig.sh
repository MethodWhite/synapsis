#!/bin/bash
# Synapsis MCP Auto-Configuration Script
# Detects installed AI dev tools and generates MCP config for each
set -euo pipefail

# Resolve the MCP binary: prefer the installed PATH entry (stable across
# machines), fall back to this repo's release build.
if command -v synapsis-mcp &>/dev/null; then
    MCP_BINARY="synapsis-mcp"
elif [ -x "$(dirname "${BASH_SOURCE[0]}")/../target/release/synapsis-mcp" ]; then
    MCP_BINARY="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/target/release/synapsis-mcp"
else
    MCP_BINARY=""
fi
SCRIPT_NAME="$(basename "$0")"

# Flags
DRY_RUN=true
VERBOSE=false

# Terminal colors
if [ -t 1 ]; then
    GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'
    CYAN='\033[0;36m'; RED='\033[0;31m'; NC='\033[0m'
else
    GREEN=''; YELLOW=''; BLUE=''; CYAN=''; RED=''; NC=''
fi

info()  { echo -e "${BLUE}->${NC} $*"; return 0; }
ok()    { echo -e "${GREEN}ok${NC} $*"; return 0; }
warn()  { echo -e "${YELLOW}..${NC} $*"; return 0; }
debug() { [ "$VERBOSE" = true ] && echo -e "${CYAN}  ${NC}$*"; return 0; }
dry()   { echo -e "${YELLOW}[DRY-RUN]${NC} $*"; return 0; }
err()   { echo -e "${RED}!!${NC} $*" >&2; return 0; }

# ── Help ──────────────────────────────────────────────────
usage() {
    cat <<EOF
Synapsis MCP Auto-Configuration

Detects installed AI development tools and generates/updates their
MCP configuration to point at the Synapsis MCP server.

Usage: ${SCRIPT_NAME} [OPTIONS]

Options:
  --apply       Actually write config files (default: dry-run only)
  --verbose     Show detailed output
  --help        Show this help message

MCP binary: ${MCP_BINARY}
EOF
    exit 0
}

# ── Parse Arguments ───────────────────────────────────────
APPLY=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --apply)   APPLY=true ;;
        --verbose) VERBOSE=true ;;
        --dry-run) DRY_RUN=true ;;
        --help|-h) usage ;;
        *) err "Unknown option: $1"; usage ;;
    esac
    shift
done

if [ "$APPLY" = true ]; then
    DRY_RUN=false
fi

# ── Prerequisites ─────────────────────────────────────────
if [ ! -x "$MCP_BINARY" ]; then
    err "MCP binary not found or not executable: ${MCP_BINARY}"
    err "Build Synapsis first: cargo build --release"
    exit 1
fi

echo "Synapsis MCP Auto-Config"
echo "  Binary: ${MCP_BINARY}"
echo "  Mode:   $([ "$DRY_RUN" = true ] && echo 'DRY-RUN (use --apply to write)' || echo 'APPLY')"
echo ""

# ── JSON Merge Function ───────────────────────────────────
# Uses jq to safely merge MCP server entry into a JSON config file.
# Falls back to Rust binary if available.
merge_mcp_config() {
    local filepath="$1"

    if [ "$DRY_RUN" = true ]; then
        dry "Would update: ${filepath}"
        return 0
    fi

    mkdir -p "$(dirname "$filepath")"

    if [ -f "$filepath" ]; then
        cp "$filepath" "${filepath}.bak" 2>/dev/null || true
        debug "Backup created: ${filepath}.bak"
    fi

    # Use jq if available (native POSIX tool)
    if command -v jq &>/dev/null; then
        local tmp; tmp=$(mktemp)
        if [ -f "$filepath" ]; then
            jq --arg cmd "$MCP_BINARY" '.mcpServers.synapsis = {command: $cmd, args: []}' "$filepath" > "$tmp" 2>/dev/null && mv "$tmp" "$filepath" || rm -f "$tmp"
        else
            jq -n --arg cmd "$MCP_BINARY" '{mcpServers: {synapsis: {command: $cmd, args: []}}}' > "$filepath"
        fi
        echo "ok $1 already up to date"
        return 0
    fi

    # Fallback: use synapsis-autoconfig Rust binary
    if command -v synapsis-autoconfig &>/dev/null; then
        synapsis-autoconfig --apply
        return $?
    fi

    err "jq or synapsis-autoconfig required for JSON merging"
    return 1
}
}

# ── Tool Detection ────────────────────────────────────────
TOOLS_DETECTED=()
TOOLS_CONFIGURED=()

detect_tool() {
    local name="$1"
    local binary="$2"
    if command -v "$binary" &>/dev/null; then
        TOOLS_DETECTED+=("$name")
        debug "Detected: ${name} (${binary})"
        return 0
    fi
    return 1
}

detect_dir() {
    local name="$1"
    local dir="$2"
    if [ -d "$dir" ]; then
        TOOLS_DETECTED+=("$name")
        debug "Detected config dir: ${dir}"
        return 0
    fi
    return 1
}

configure_tool() {
    local tool="$1"
    local filepath="$2"
    local label="${3:-$tool}"

    info "Configuring ${label} ..."
    debug "  Target: ${filepath}"

    if merge_mcp_config "$filepath"; then
        if [ "$DRY_RUN" = false ]; then
            ok "${label} configured"
        fi
        TOOLS_CONFIGURED+=("$label")
    else
        if [ "$DRY_RUN" = false ]; then
            ok "${label} already up to date"
        else
            dry "${label} already up to date (no change needed)"
        fi
    fi
}

# ── Detect Tools ──────────────────────────────────────────
info "Detecting AI development tools..."

detect_tool "opencode"  "opencode"      || true
detect_tool "claude"    "claude"        || true
detect_tool "code"      "code"          || true
detect_tool "cursor"    "cursor"        || true
detect_tool "windsurf"  "windsurf"      || true
detect_tool "gemini"    "gemini"        || true
detect_tool "cline"     "cline"         || true
detect_tool "aider"     "aider"         || true
detect_tool "nvim"      "nvim"          || true
detect_tool "idea"      "idea"          || true
detect_tool "gh"        "gh"            || true

detect_dir "vscode-user-data"  "${HOME}/.vscode"          || true
detect_dir "jetbrains-config" "${HOME}/.config/JetBrains" || true
detect_dir "claude-config"    "${HOME}/.config/claude"    || true
detect_dir "gemini-config"    "${HOME}/.config/gemini"    || true

echo ""
info "Detected tools: ${TOOLS_DETECTED[*]:-(none)}"
echo ""

# ── Generate MCP Configs ──────────────────────────────────
info "Generating MCP configurations..."

# OpenCode (~/.config/opencode/opencode.jsonc)
if [[ " ${TOOLS_DETECTED[*]} " =~ " opencode " ]] || [ -d "${HOME}/.config/opencode" ]; then
    configure_tool "opencode" "${HOME}/.config/opencode/opencode.jsonc" "OpenCode"
fi

# Claude Code (~/.claude/settings.json or ~/.config/claude/settings.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " claude " ]] || [ -d "${HOME}/.claude" ] || [ -d "${HOME}/.config/claude" ]; then
    if [ -f "${HOME}/.claude/settings.json" ] || [ ! -d "${HOME}/.config/claude" ]; then
        configure_tool "claude" "${HOME}/.claude/settings.json" "Claude Code"
    fi
    if [ -d "${HOME}/.config/claude" ]; then
        configure_tool "claude-xdg" "${HOME}/.config/claude/settings.json" "Claude Code (XDG)"
    fi
fi

# Cursor (~/.cursor/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " cursor " ]] || [ -d "${HOME}/.cursor" ]; then
    configure_tool "cursor" "${HOME}/.cursor/mcp.json" "Cursor"
fi

# Windsurf (~/.windsurf/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " windsurf " ]] || [ -d "${HOME}/.windsurf" ]; then
    configure_tool "windsurf" "${HOME}/.windsurf/mcp.json" "Windsurf"
fi

# VS Code (~/.vscode/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " code " ]] || [ -d "${HOME}/.vscode" ]; then
    configure_tool "vscode" "${HOME}/.vscode/mcp.json" "VS Code"
fi

# Gemini CLI (~/.config/gemini/config.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " gemini " ]] || [ -d "${HOME}/.config/gemini" ]; then
    configure_tool "gemini" "${HOME}/.config/gemini/config.json" "Gemini CLI"
fi

# Cline (~/.config/cline/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " cline " ]] || [ -d "${HOME}/.config/cline" ]; then
    configure_tool "cline" "${HOME}/.config/cline/mcp.json" "Cline"
fi

# Aider (~/.aider/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " aider " ]] || [ -d "${HOME}/.aider" ] || [ -d "${HOME}/.config/aider" ]; then
    configure_tool "aider" "${HOME}/.aider/mcp.json" "Aider"
fi

# Neovim
if [[ " ${TOOLS_DETECTED[*]} " =~ " nvim " ]]; then
    info "Neovim detected - MCP config is plugin-managed (lazy.nvim/mason)"
    debug "  Skipping auto-config for nvim (managed by editor plugins)"
fi

# JetBrains IDEs (~/.config/JetBrains/*/options/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " idea " ]] || [ -d "${HOME}/.config/JetBrains" ]; then
    info "JetBrains IDEs detected - MCP config is per-IDE in options/"
    debug "  Checking for JetBrains IDE directories..."
    for ide_dir in "${HOME}"/.config/JetBrains/*/; do
        [ -d "$ide_dir" ] || continue
        opts_dir="${ide_dir}options"
        if [ -d "$opts_dir" ]; then
            configure_tool "jetbrains" "${opts_dir}/mcp.json" "JetBrains ($(basename "$ide_dir"))"
        fi
    done
fi

# GitHub CLI (~/.config/gh/mcp.json)
if [[ " ${TOOLS_DETECTED[*]} " =~ " gh " ]]; then
    if [ -d "${HOME}/.config/gh" ]; then
        configure_tool "github-cli" "${HOME}/.config/gh/mcp.json" "GitHub CLI"
    fi
fi

# ── Summary ───────────────────────────────────────────────
echo ""
if [ "$DRY_RUN" = true ]; then
    warn "DRY-RUN completed. Re-run with --apply to write config files."
else
    ok "Configuration complete."
    if [ ${#TOOLS_CONFIGURED[@]} -gt 0 ]; then
        for t in "${TOOLS_CONFIGURED[@]}"; do
            ok "  ${t}"
        done
    fi
fi
echo ""
echo "MCP binary: ${MCP_BINARY}"
