#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_NAME="$(basename "$PROJECT_DIR")"

echo "==> Generating SBOM for $PROJECT_NAME"

# Generate CycloneDX SBOM using cargo-cyclonedx
if command -v cargo-cyclonedx &>/dev/null || cargo install cargo-cyclonedx --quiet 2>/dev/null; then
    cargo cyclonedx --all --output "$PROJECT_DIR/target/sbom" 2>/dev/null && \
    echo "  ✓ CycloneDX SBOM: target/sbom/"
fi

# Generate SPDX SBOM using cargo-spdx (fallback)
if ! ls "$PROJECT_DIR"/target/sbom/*.cdx.* &>/dev/null; then
    if command -v cargo-spdx &>/dev/null || cargo install cargo-spdx --quiet 2>/dev/null; then
        cargo spdx --output "$PROJECT_DIR/target/sbom/spdx.json" 2>/dev/null && \
        echo "  ✓ SPDX SBOM: target/sbom/spdx.json"
    fi
fi

# Generate dependency tree
cargo tree --prefix depth --no-dedupe > "$PROJECT_DIR/target/sbom/deps-tree.txt" 2>/dev/null && \
echo "  ✓ Dependency tree: target/sbom/deps-tree.txt"

# Generate license summary
cargo license 2>/dev/null > "$PROJECT_DIR/target/sbom/licenses.txt" && \
echo "  ✓ License summary: target/sbom/licenses.txt"

echo "==> SBOM generation complete"
ls -la "$PROJECT_DIR/target/sbom/" 2>/dev/null || echo "  (no SBOM files generated)"
