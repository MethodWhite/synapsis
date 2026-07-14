#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_NAME="$(basename "$PROJECT_DIR")"

CARGO_CONFIG_DIR="$PROJECT_DIR/.cargo"
CARGO_CONFIG="$CARGO_CONFIG_DIR/config.toml"

# MethodWhite repos that are commonly checked out as siblings
SIBLING_REPOS=(
  "synapsis-core"
  "prusia-vault"
  "ztf"
  "arca"
)

echo "🔧 dev-setup.sh — $PROJECT_NAME"
echo "  Project dir: $PROJECT_DIR"

mkdir -p "$CARGO_CONFIG_DIR"

# Start with net config
cat > "$CARGO_CONFIG" << 'EOF'
[net]
git-fetch-with-cli = true
EOF

# Add patches for any sibling repos that exist
PATCH_COUNT=0
for repo in "${SIBLING_REPOS[@]}"; do
  SIBLING_PATH="$PROJECT_DIR/../$repo"
  if [ -d "$SIBLING_PATH/.git" ]; then
    echo "  ✓ Found sibling: $repo"
    cat >> "$CARGO_CONFIG" << EOF

[patch."https://github.com/MethodWhite/$repo"]
$repo = { path = "../$repo" }
EOF
    PATCH_COUNT=$((PATCH_COUNT + 1))
  fi
done

if [ "$PATCH_COUNT" -eq 0 ]; then
  echo "  ℹ No sibling repos found — using git dependencies directly"
fi

echo "  ✓ Written: $CARGO_CONFIG"
