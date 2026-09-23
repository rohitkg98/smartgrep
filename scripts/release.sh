#!/usr/bin/env bash
#
# Cut a smartgrep release.
#
# Usage:
#   scripts/release.sh <X.Y.Z> [--dry-run]
#
# Steps: preflight checks (on main, clean tree, in sync with origin, new version,
# tag unused) → cargo test → regression suite → bump Cargo.toml/Cargo.lock →
# commit "bump version to X.Y.Z" → tag vX.Y.Z → push main + tag → wait for the
# Release workflow and verify the binaries were attached.
#
# The tag push triggers .github/workflows/release.yml, which builds the binaries
# and creates the GitHub release. --dry-run runs checks and tests, then stops.
#
set -euo pipefail

REPO="rohitkg98/smartgrep"
EXPECTED_ASSETS=3

die() { echo "error: $*" >&2; exit 1; }
step() { echo; echo "==> $*"; }

VERSION="${1:-}"
DRY_RUN=false
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=true
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "usage: scripts/release.sh <X.Y.Z> [--dry-run]"
TAG="v$VERSION"

cd "$(git rev-parse --show-toplevel)"

step "Preflight"
[[ "$(git branch --show-current)" == "main" ]] || die "not on main"
git diff --quiet && git diff --cached --quiet || die "uncommitted changes to tracked files"
git fetch -q origin main --tags
[[ "$(git rev-list --count HEAD..origin/main)" == "0" ]] || die "behind origin/main; pull first"
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && die "tag $TAG already exists"
CURRENT=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[[ "$(printf '%s\n%s\n' "$CURRENT" "$VERSION" | sort -V | tail -1)" == "$VERSION" && "$CURRENT" != "$VERSION" ]] \
    || die "version $VERSION must be greater than current $CURRENT"
echo "releasing $CURRENT → $VERSION"

step "Tests"
cargo test --quiet
cargo build --quiet
SMARTGREP=./target/debug/smartgrep bash tests/regression/run.sh >/dev/null || die "regression suite failed"
echo "tests and regression suite passed"

if $DRY_RUN; then
    echo; echo "dry run: stopping before version bump"
    exit 0
fi

step "Bump version"
# Only the first `version = ` line (the [package] one); portable across macOS/Linux.
perl -0pi -e "s/^version = \"[^\"]*\"/version = \"$VERSION\"/m" Cargo.toml
grep -q "^version = \"$VERSION\"" Cargo.toml || die "failed to bump Cargo.toml"
cargo check --quiet  # refreshes Cargo.lock
git add Cargo.toml Cargo.lock
git commit -q -m "bump version to $VERSION"
git tag -a "$TAG" -m "smartgrep $VERSION"

step "Push"
git push -q origin main
git push -q origin "$TAG"

step "Wait for Release workflow"
RUN_ID=""
for _ in $(seq 1 30); do
    RUN_ID=$(gh run list --repo "$REPO" --workflow release.yml --branch "$TAG" --limit 1 \
        --json databaseId --jq '.[0].databaseId // empty')
    [[ -n "$RUN_ID" ]] && break
    sleep 5
done
[[ -n "$RUN_ID" ]] || die "release workflow did not start; check https://github.com/$REPO/actions"
gh run watch "$RUN_ID" --repo "$REPO" --exit-status --interval 20 >/dev/null \
    || die "release workflow failed: https://github.com/$REPO/actions/runs/$RUN_ID"

ASSETS=$(gh release view "$TAG" --repo "$REPO" --json assets --jq '.assets | length')
[[ "$ASSETS" -ge "$EXPECTED_ASSETS" ]] || die "release $TAG has $ASSETS assets, expected $EXPECTED_ASSETS"
echo; echo "released $TAG with $ASSETS assets: https://github.com/$REPO/releases/tag/$TAG"
