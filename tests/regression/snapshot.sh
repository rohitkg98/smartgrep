#!/usr/bin/env bash
#
# Run the regression script and write an OS-independent snapshot of its output
# and of every index it built into OUT_DIR (default: regression-snapshot/).
# CI diffs the snapshots from each OS: smartgrep's output and index content
# must be byte-identical for the same tree on Linux, macOS, Windows and FreeBSD.
#
# Only the checkout location is normalized (everything up to `tests/regression/`
# is stripped); anything else that differs is a portability bug.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUT_DIR="${1:-regression-snapshot}"
mkdir -p "$OUT_DIR"

strip_prefix() {
    sed -E 's#[^ "'"'"'(]*tests/regression/#tests/regression/#g'
}

bash "$SCRIPT_DIR/run.sh" 2>&1 | strip_prefix > "$OUT_DIR/run.txt"

# Indexes: key order of the lookup maps is hash-dependent, so sort keys.
# (jq on Windows may emit CRLF; that's jq, not smartgrep.)
canon() {
    jq -S . "$1" | tr -d '\r'
}
for p in rust_project go_project java_project ts_project python_project; do
    canon "$SCRIPT_DIR/$p/.smartgrep/index.json" > "$OUT_DIR/index-$p.json"
done
canon "$REPO_DIR/.smartgrep/index.json" > "$OUT_DIR/index-repo.json"
