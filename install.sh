#!/bin/sh
set -e

REPO="rohitkg98/smartgrep"
BINARY="smartgrep"
INSTALL_DIR="/usr/local/bin"

# ── detect OS and arch ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *)      echo "Unsupported macOS architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64)          TARGET="x86_64-unknown-linux-musl" ;;
      aarch64|arm64)   TARGET="aarch64-unknown-linux-musl" ;;
      *)               echo "Unsupported Linux architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# ── find latest release ───────────────────────────────────────────────────────

if command -v curl >/dev/null 2>&1; then
  LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | sed 's/.*"tag_name": *"\(.*\)".*/\1/')
elif command -v wget >/dev/null 2>&1; then
  LATEST=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | sed 's/.*"tag_name": *"\(.*\)".*/\1/')
else
  echo "Error: curl or wget is required." >&2
  exit 1
fi

if [ -z "$LATEST" ]; then
  echo "Error: could not determine latest release." >&2
  exit 1
fi

ASSET="${BINARY}-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ASSET}"

# ── download ──────────────────────────────────────────────────────────────────

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading smartgrep ${LATEST} (${TARGET})..."

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$URL" -o "${TMP}/${ASSET}"
else
  wget -qO "${TMP}/${ASSET}" "$URL"
fi

tar -xzf "${TMP}/${ASSET}" -C "$TMP"

# ── install ───────────────────────────────────────────────────────────────────

# Fall back to ~/.local/bin if /usr/local/bin isn't writable
if [ ! -w "$INSTALL_DIR" ]; then
  INSTALL_DIR="$HOME/.local/bin"
  mkdir -p "$INSTALL_DIR"
fi

install -m 755 "${TMP}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

echo "Installed smartgrep ${LATEST} to ${INSTALL_DIR}/${BINARY}"

# ── PATH hint ─────────────────────────────────────────────────────────────────

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo ""
    echo "Note: ${INSTALL_DIR} is not in your PATH."
    echo "Add this to your shell profile:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
