#!/bin/bash
set -e

# KubeStudio installer
# Usage: curl -fsSL https://raw.githubusercontent.com/Strike48/kubestudio/main/install.sh | bash

REPO="Strike48/kubestudio"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS="darwin" ;;
  linux) OS="linux" ;;
  *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

BINARY="kubestudio-${OS}-${ARCH}"

# Get latest release
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Failed to fetch latest release"
  exit 1
fi

echo "Installing KubeStudio ${LATEST}..."

# Download binary
URL="https://github.com/${REPO}/releases/download/${LATEST}/${BINARY}"
echo "Downloading from ${URL}..."

mkdir -p "$INSTALL_DIR"
curl -fsSL "$URL" -o "${INSTALL_DIR}/kubestudio"
chmod +x "${INSTALL_DIR}/kubestudio"

echo ""
echo "KubeStudio installed to ${INSTALL_DIR}/kubestudio"

# Check if install dir is in PATH
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
  echo ""
  echo "Add to your PATH:"
  echo "  export PATH=\"\$PATH:${INSTALL_DIR}\""
fi

echo ""
echo "Run 'kubestudio' to start"
