#!/bin/sh
#
# Smoke-test a built smartgrep binary by running it against the Python
# regression project. POSIX sh so it also runs on FreeBSD, Alpine and Git Bash.
#
# Usage:
#   scripts/smoke.sh <command...>
#
# <command...> is how to invoke the binary, e.g.
#   scripts/smoke.sh ./smartgrep
#   scripts/smoke.sh docker run --rm --platform linux/s390x -v "$PWD:/w" -w /w debian:bookworm-slim ./smartgrep
#
# Paths are relative, so run it from the repository root.
set -eu

[ $# -gt 0 ] || { echo "usage: scripts/smoke.sh <command...>" >&2; exit 2; }

PROJECT="tests/regression/python_project"
FAILED=0

check() {
  desc="$1"; pattern="$2"; shift 2
  echo "\$ $*"
  if ! out=$("$@" 2>&1); then
    echo "$out"
    echo "FAIL: $desc (non-zero exit)"
    FAILED=1
    return
  fi
  echo "$out" | head -n 20
  if [ -z "$out" ]; then
    echo "FAIL: $desc (empty output)"
    FAILED=1
  elif ! printf '%s\n' "$out" | grep -q -- "$pattern"; then
    echo "FAIL: $desc (expected '$pattern' in output)"
    FAILED=1
  else
    echo "ok: $desc"
  fi
  echo
}

check "--help"          "Usage"  "$@" --help
check "ls classes"      "User"   "$@" --project-root "$PROJECT" ls classes
check "refs User"       "user"   "$@" --project-root "$PROJECT" refs User

if [ "$FAILED" -ne 0 ]; then
  echo "smoke test FAILED"
  exit 1
fi
echo "smoke test passed"
