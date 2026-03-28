#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Running cargo fmt --check..."
cargo fmt --check

echo "Running cargo clippy --all-targets -- -D warnings..."
cargo clippy --all-targets -- -D warnings

echo "Linting completed successfully."