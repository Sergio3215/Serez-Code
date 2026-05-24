#!/bin/sh
# Serez-Code installer for Linux and macOS
# Usage: curl -fsSL https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.sh | sh
set -e

REPO="Sergio3215/serez-code"
BIN_DIR="${HOME}/.local/bin"

OS=$(uname -s)
ARCH=$(uname -m)

case "${OS}" in
  Linux*)
    ASSET="sz-linux-x64"
    ;;
  Darwin*)
    case "${ARCH}" in
      arm64)  ASSET="sz-macos-arm64" ;;
      x86_64) ASSET="sz-macos-x64"   ;;
      *)      echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: ${OS}"
    echo "On Windows use: irm https://raw.githubusercontent.com/Sergio3215/serez-code/main/install.ps1 | iex"
    exit 1
    ;;
esac

TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "${TAG}" ]; then
  echo "Could not fetch latest release. Check your internet connection."
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"

echo "Installing Serez-Code ${TAG}..."
mkdir -p "${BIN_DIR}"
curl -fsSL "${URL}" -o "${BIN_DIR}/sz"
chmod +x "${BIN_DIR}/sz"

# Remove macOS quarantine attribute so Gatekeeper doesn't block the binary
if [ "${OS}" = "Darwin" ]; then
  xattr -d com.apple.quarantine "${BIN_DIR}/sz" 2>/dev/null || true
fi

echo ""
echo "Installed: ${BIN_DIR}/sz"

# Check if BIN_DIR is already in PATH
case ":${PATH}:" in
  *":${BIN_DIR}:"*)
    echo "Ready! Run: sz --version"
    ;;
  *)
    echo ""
    echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
    echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
    echo ""
    echo "Then restart your terminal and run: sz --version"
    ;;
esac
