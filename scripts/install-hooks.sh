#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.githooks"

echo "Installing git hooks from $HOOKS_DIR..."
git config core.hooksPath "$HOOKS_DIR"
echo "Git hooks installed successfully."

# Check for required tools
if ! command -v gitleaks &> /dev/null; then
    echo "WARNING: gitleaks is not installed. Install it with: brew install gitleaks"
fi

if ! command -v cargo &> /dev/null; then
    echo "WARNING: cargo is not installed. Install Rust from https://rustup.rs"
fi
