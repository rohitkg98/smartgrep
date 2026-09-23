#!/bin/sh
#
# Print the expected release asset names, one per line, derived from the build
# matrix in .github/workflows/release.yml (the single source of the target list):
# smartgrep-<target>.zip for Windows targets, smartgrep-<target>.tar.gz otherwise,
# plus SHA256SUMS. Used by scripts/release.sh and the release workflow.
set -eu

WORKFLOW="$(cd "$(dirname "$0")/.." && pwd)/.github/workflows/release.yml"
TARGETS=$(sed -n 's/^ *- target: *\([A-Za-z0-9_.-]*\).*/\1/p' "$WORKFLOW")
[ -n "$TARGETS" ] || { echo "no '- target:' entries found in $WORKFLOW" >&2; exit 1; }

for t in $TARGETS; do
  case "$t" in
    *windows*) echo "smartgrep-$t.zip" ;;
    *)         echo "smartgrep-$t.tar.gz" ;;
  esac
done
echo "SHA256SUMS"
