#!/bin/bash
# Synapsis MCP Auto-Configuration Script
# Detects installed AI dev tools and generates MCP config for each
set -euo pipefail

MCP_BINARY="/home/methodwhite/Proyectos/synapsis/target/release/synapsis-mcp"
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

PYTHON=$(command -v python3 || command -v python || true)
if [ -z "$PYTHON" ]; then
    err "python3 is required for JSON merging"
    exit 1
fi

echo "Synapsis MCP Auto-Config"
echo "  Binary: ${MCP_BINARY}"
echo "  Mode:   $([ "$DRY_RUN" = true ] && echo 'DRY-RUN (use --apply to write)' || echo 'APPLY')"
echo ""

# ── JSON Merge Function ───────────────────────────────────
# Uses python3 to safely merge MCP server entry into a JSON config file.
# Returns 0 if file was changed, 1 if no change needed.
merge_mcp_config() {
    local filepath="$1"

    if [ "$DRY_RUN" = true ]; then
        dry "Would update: ${filepath}"
        return 0
    fi

    mkdir -p "$(dirname "$filepath")"

    # Backup existing file
    if [ -f "$filepath" ]; then
        cp "$filepath" "${filepath}.bak"
        debug "Backup created: ${filepath}.bak"
    fi

    "$PYTHON" -c "
import json, sys

filepath = '$filepath'
binary = '$MCP_BINARY'

try:
    with open(filepath) as f:
        data = json.load(f)
except (FileNotFoundError, json.JSONDecodeError):
    data = {}

data.setdefault('mcpServers', {})

existing = data['mcpServers'].get('synapsis', {})
if isinstance(existing, dict) and existing.get('command') == binary:
    sys.exit(1)

data['mcpServers']['synapsis'] = {'command': binary, 'args': []}

with open(filepath, 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')

sys.exit(0)
" && return 0 || return 1
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
