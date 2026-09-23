#!/bin/sh
# smartgrep installer for macOS, Linux, FreeBSD and Android (Termux).
#
#   curl -fsSL https://raw.githubusercontent.com/rohitkg98/smartgrep/main/install.sh | sh
#
# Environment variables:
#   SMARTGREP_VERSION      release to install, e.g. 0.4.0 or v0.4.0 (default: latest)
#   SMARTGREP_INSTALL_DIR  install directory (default: /usr/local/bin if writable,
#                          else ~/.local/bin; $PREFIX/bin on Termux)
#   SMARTGREP_ARCHIVE      install from a local release archive instead of downloading
#                          (checksum-verified against a SHA256SUMS file next to it, if any)
#   SMARTGREP_PRINT_TARGET=1  print the detected release target and exit (for testing)
#
# Windows: use install.ps1 instead.
set -eu

REPO="rohitkg98/smartgrep"
BINARY="smartgrep"
FALLBACK="cargo install --git https://github.com/${REPO}"

say() { printf '%s\n' "$*"; }
err() { printf 'error: %s\n' "$*" >&2; }

unsupported() {
  err "$1"
  err "No prebuilt smartgrep binary for this platform. Build from source instead (needs Rust 1.70+):"
  err "  ${FALLBACK}"
  err "Or open an issue: https://github.com/${REPO}/issues"
  exit 1
}

# ── detect OS and arch ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"
# `uname -o` is not in POSIX and fails on some BSDs; only Android needs it.
KERNEL_OS="$(uname -o 2>/dev/null || true)"
IS_TERMUX=false

case "$OS" in
  Darwin)
    # A shell running under Rosetta reports x86_64 on Apple Silicon; prefer the native binary.
    if [ "$ARCH" = "x86_64" ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
      ARCH="arm64"
    fi
    case "$ARCH" in
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64)        TARGET="x86_64-apple-darwin" ;;
      *)             unsupported "unsupported macOS architecture: $ARCH" ;;
    esac
    ;;
  Linux)
    if [ "$KERNEL_OS" = "Android" ]; then
      # Termux: static musl binaries run fine; install into Termux's prefix.
      IS_TERMUX=true
    fi
    case "$ARCH" in
      x86_64|amd64)             TARGET="x86_64-unknown-linux-musl" ;;
      aarch64|arm64)            TARGET="aarch64-unknown-linux-musl" ;;
      armv7*|armv8l)            TARGET="armv7-unknown-linux-musleabihf" ;;
      armv6*)                   TARGET="arm-unknown-linux-musleabihf" ;;
      i386|i486|i586|i686)      TARGET="i686-unknown-linux-musl" ;;
      riscv64)                  TARGET="riscv64gc-unknown-linux-musl" ;;
      ppc64le)                  TARGET="powerpc64le-unknown-linux-musl" ;;
      s390x)                    TARGET="s390x-unknown-linux-gnu" ;;
      loongarch64)              TARGET="loongarch64-unknown-linux-musl" ;;
      *)                        unsupported "unsupported Linux architecture: $ARCH" ;;
    esac
    if $IS_TERMUX; then
      case "$TARGET" in
        aarch64-*|armv7-*|x86_64-*|i686-*) ;;
        *) unsupported "unsupported Android architecture: $ARCH" ;;
      esac
    fi
    ;;
  FreeBSD)
    case "$ARCH" in
      amd64|x86_64) TARGET="x86_64-unknown-freebsd" ;;
      *)            unsupported "unsupported FreeBSD architecture: $ARCH" ;;
    esac
    ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    err "on Windows, install with PowerShell instead:"
    err "  irm https://raw.githubusercontent.com/${REPO}/main/install.ps1 | iex"
    exit 1
    ;;
  *)
    unsupported "unsupported OS: $OS"
    ;;
esac

if [ "${SMARTGREP_PRINT_TARGET:-}" = "1" ]; then
  say "$TARGET"
  exit 0
fi

ASSET="${BINARY}-${TARGET}.tar.gz"

# ── helpers ───────────────────────────────────────────────────────────────────

if command -v curl >/dev/null 2>&1; then
  fetch()        { curl -fsSL "$1"; }
  fetch_to()     { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch()        { wget -qO- "$1"; }
  fetch_to()     { wget -qO "$2" "$1"; }
else
  fetch()        { err "curl or wget is required"; exit 1; }
  fetch_to()     { fetch; }
fi

# Print the SHA-256 of a file, or nothing if no tool is available.
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d' ' -f1
  elif command -v sha256 >/dev/null 2>&1; then
    sha256 -q "$1"
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$1" | sed 's/.*= *//'
  fi
}

# verify <archive> <SHA256SUMS file> <asset name>
verify() {
  expected=$(awk -v f="$3" '{ n = $2; sub(/^\*/, "", n) } n == f { print $1; exit }' "$2")
  if [ -z "$expected" ]; then
    err "SHA256SUMS has no entry for $3"
    exit 1
  fi
  actual=$(sha256_of "$1")
  if [ -z "$actual" ]; then
    say "warning: no sha256 tool found (sha256sum, shasum, sha256, openssl); skipping checksum verification"
  elif [ "$actual" != "$expected" ]; then
    err "checksum mismatch for $3"
    err "  expected $expected"
    err "  got      $actual"
    exit 1
  else
    say "Checksum verified."
  fi
}

TMP="$(mktemp -d 2>/dev/null || mktemp -d -t smartgrep)"
trap 'rm -rf "$TMP"' EXIT
trap 'exit 1' INT TERM

# ── get the archive ───────────────────────────────────────────────────────────

if [ -n "${SMARTGREP_ARCHIVE:-}" ]; then
  [ -f "$SMARTGREP_ARCHIVE" ] || { err "SMARTGREP_ARCHIVE not found: $SMARTGREP_ARCHIVE"; exit 1; }
  VERSION_LABEL="from $SMARTGREP_ARCHIVE"
  ASSET="$(basename "$SMARTGREP_ARCHIVE")"
  say "Installing smartgrep ${VERSION_LABEL} (${TARGET})..."
  cp "$SMARTGREP_ARCHIVE" "${TMP}/${ASSET}"
  SUMS="$(dirname "$SMARTGREP_ARCHIVE")/SHA256SUMS"
  if [ -f "$SUMS" ]; then
    verify "${TMP}/${ASSET}" "$SUMS" "$ASSET"
  else
    say "warning: no SHA256SUMS next to $SMARTGREP_ARCHIVE; skipping checksum verification"
  fi
else
  if [ -n "${SMARTGREP_VERSION:-}" ]; then
    TAG="v${SMARTGREP_VERSION#v}"
  else
    TAG=$(fetch "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
      | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/' || true)
  fi
  if [ -n "$TAG" ]; then
    BASE="https://github.com/${REPO}/releases/download/${TAG}"
    VERSION_LABEL="$TAG"
  else
    # API unavailable (e.g. rate-limited): GitHub redirects this to the latest release.
    BASE="https://github.com/${REPO}/releases/latest/download"
    VERSION_LABEL="(latest)"
  fi

  say "Downloading smartgrep ${VERSION_LABEL} (${TARGET})..."
  if ! fetch_to "${BASE}/${ASSET}" "${TMP}/${ASSET}"; then
    err "download failed: ${BASE}/${ASSET}"
    [ -n "${SMARTGREP_VERSION:-}" ] && err "does release ${TAG} exist? https://github.com/${REPO}/releases"
    err "if this release has no binary for ${TARGET}, build from source: ${FALLBACK}"
    exit 1
  fi
  if fetch_to "${BASE}/SHA256SUMS" "${TMP}/SHA256SUMS" 2>/dev/null; then
    verify "${TMP}/${ASSET}" "${TMP}/SHA256SUMS" "$ASSET"
  else
    say "warning: release has no SHA256SUMS; skipping checksum verification"
  fi
fi

tar -xzf "${TMP}/${ASSET}" -C "$TMP"
[ -f "${TMP}/${BINARY}" ] || { err "archive did not contain ${BINARY}"; exit 1; }

# ── install ───────────────────────────────────────────────────────────────────

if [ -n "${SMARTGREP_INSTALL_DIR:-}" ]; then
  INSTALL_DIR="$SMARTGREP_INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
elif $IS_TERMUX && [ -n "${PREFIX:-}" ]; then
  INSTALL_DIR="$PREFIX/bin"
  mkdir -p "$INSTALL_DIR"
else
  INSTALL_DIR="/usr/local/bin"
  # Fall back to ~/.local/bin if /usr/local/bin isn't writable
  if [ ! -w "$INSTALL_DIR" ]; then
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
  fi
fi

# Copy then rename, so a running smartgrep is replaced atomically instead of
# being overwritten in place ("text file busy").
cp "${TMP}/${BINARY}" "${INSTALL_DIR}/.${BINARY}.new"
chmod 755 "${INSTALL_DIR}/.${BINARY}.new"
mv -f "${INSTALL_DIR}/.${BINARY}.new" "${INSTALL_DIR}/${BINARY}"

say "Installed smartgrep ${VERSION_LABEL} to ${INSTALL_DIR}/${BINARY}"

# ── PATH hint ─────────────────────────────────────────────────────────────────

case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    say ""
    say "Note: ${INSTALL_DIR} is not in your PATH."
    say "Add this to your shell profile:"
    say "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
