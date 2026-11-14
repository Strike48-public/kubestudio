#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.githooks"

echo "Installing git hooks from $HOOKS_DIR..."
git config core.hooksPath "$HOOKS_DIR"
chmod +x "$HOOKS_DIR"/* "$REPO_ROOT/scripts/check-pii.sh"
echo "Git hooks installed successfully."

# Seed the local (gitignored) PII name list from the example if it's missing,
# so the PII hooks have a list to scan against. The scanner fails loud without
# one; edit .pii-names.local to add the real customer/tenant names.
if [[ ! -f "$REPO_ROOT/.pii-names.local" && -f "$REPO_ROOT/.pii-names.local.example" ]]; then
    cp "$REPO_ROOT/.pii-names.local.example" "$REPO_ROOT/.pii-names.local"
    echo "Created .pii-names.local from the example - edit it to add the real names."
fi

# Make the placeholder-only state obvious: if the local list is still identical
# to the committed example, the hooks scan against placeholder names only and
# provide no real protection until an operator fills in the real list. The real
# names are secret and distributed out-of-band, so CI (with the PII_NAMES
# secret) remains the authoritative gate - see scripts/check-pii.sh.
if [[ -f "$REPO_ROOT/.pii-names.local" && -f "$REPO_ROOT/.pii-names.local.example" ]] \
    && cmp -s "$REPO_ROOT/.pii-names.local" "$REPO_ROOT/.pii-names.local.example"; then
    echo ""
    echo "WARNING: .pii-names.local still contains only the placeholder example names."
    echo "         The local hooks scan against placeholders and will NOT catch real"
    echo "         customer names until you edit .pii-names.local with the real list."
fi

# Check for required tools
if ! command -v gitleaks &> /dev/null; then
    echo "WARNING: gitleaks is not installed. Install it with: brew install gitleaks"
fi

if ! command -v cargo &> /dev/null; then
    echo "WARNING: cargo is not installed. Install Rust from https://rustup.rs"
fi
