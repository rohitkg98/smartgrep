#!/bin/sh
#
# Check install.sh's platform detection: fake `uname`/`sysctl` on PATH and
# assert which release target it picks (SMARTGREP_PRINT_TARGET=1). POSIX sh.
#
# Usage: scripts/test-install-targets.sh [path/to/install.sh]
set -eu

INSTALL_SH="${1:-install.sh}"
SHIM="$(mktemp -d)"
trap 'rm -rf "$SHIM"' EXIT
FAILED=0

cat > "$SHIM/uname" <<'EOF'
#!/bin/sh
case "$1" in
  -s) echo "$FAKE_S" ;;
  -m) echo "$FAKE_M" ;;
  -o) [ -n "$FAKE_O" ] && echo "$FAKE_O" || exit 1 ;;
esac
EOF
cat > "$SHIM/sysctl" <<'EOF'
#!/bin/sh
[ -n "$FAKE_TRANSLATED" ] && echo "$FAKE_TRANSLATED" || exit 1
EOF
chmod +x "$SHIM/uname" "$SHIM/sysctl"

# expect <uname -s> <uname -m> <uname -o> <proc_translated> <expected target | FAIL>
expect() {
  got=$(FAKE_S="$1" FAKE_M="$2" FAKE_O="$3" FAKE_TRANSLATED="$4" PATH="$SHIM:$PATH" \
    SMARTGREP_PRINT_TARGET=1 sh "$INSTALL_SH" 2>/dev/null) || got=FAIL
  if [ "$got" = "$5" ]; then
    echo "ok    $1 $2 ${3:+($3) }-> $got"
  else
    echo "FAIL  $1 $2 ${3:+($3) }-> $got (expected $5)"
    FAILED=1
  fi
}

expect Darwin  arm64       ""        ""  aarch64-apple-darwin
expect Darwin  x86_64      ""        "0" x86_64-apple-darwin
expect Darwin  x86_64      ""        ""  x86_64-apple-darwin
expect Darwin  x86_64      ""        "1" aarch64-apple-darwin   # Rosetta
expect Linux   x86_64      GNU/Linux ""  x86_64-unknown-linux-musl
expect Linux   aarch64     GNU/Linux ""  aarch64-unknown-linux-musl
expect Linux   arm64       GNU/Linux ""  aarch64-unknown-linux-musl
expect Linux   armv7l      GNU/Linux ""  armv7-unknown-linux-musleabihf
expect Linux   armv8l      GNU/Linux ""  armv7-unknown-linux-musleabihf
expect Linux   armv6l      GNU/Linux ""  arm-unknown-linux-musleabihf
expect Linux   i686        GNU/Linux ""  i686-unknown-linux-musl
expect Linux   i386        GNU/Linux ""  i686-unknown-linux-musl
expect Linux   riscv64     GNU/Linux ""  riscv64gc-unknown-linux-musl
expect Linux   ppc64le     GNU/Linux ""  powerpc64le-unknown-linux-musl
expect Linux   s390x       GNU/Linux ""  s390x-unknown-linux-gnu
expect Linux   loongarch64 GNU/Linux ""  loongarch64-unknown-linux-musl
expect Linux   mips        GNU/Linux ""  FAIL
expect Linux   aarch64     Android   ""  aarch64-unknown-linux-musl
expect Linux   armv7l      Android   ""  armv7-unknown-linux-musleabihf
expect Linux   armv8l      Android   ""  armv7-unknown-linux-musleabihf
expect Linux   s390x       Android   ""  FAIL
expect FreeBSD amd64       ""        ""  x86_64-unknown-freebsd
expect FreeBSD arm64       ""        ""  FAIL
expect OpenBSD amd64       ""        ""  FAIL
expect MINGW64_NT-10.0 x86_64 Msys   ""  FAIL

# Unmapped platforms must point at the cargo fallback.
if FAKE_S=Linux FAKE_M=mips FAKE_O= FAKE_TRANSLATED= PATH="$SHIM:$PATH" sh "$INSTALL_SH" 2>&1 \
    | grep -q "cargo install --git https://github.com/rohitkg98/smartgrep"; then
  echo "ok    unsupported platform suggests cargo install --git"
else
  echo "FAIL  unsupported platform does not suggest cargo install --git"
  FAILED=1
fi

[ "$FAILED" -eq 0 ] && echo "all install.sh target checks passed" || exit 1
