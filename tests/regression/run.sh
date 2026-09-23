#!/usr/bin/env bash
#
# Regression test: exercises smartgrep against multi-file test projects.
# Output is for human review in CI — no automated assertions.
# Exit code: 0 if all commands succeed, 1 if any command fails.
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMARTGREP="${SMARTGREP:-cargo run --quiet --}"
FAILED=0

header() {
    echo ""
    echo "================================================================"
    echo "  $1"
    echo "================================================================"
}

section() {
    echo ""
    echo "--- $1 ---"
}

run_cmd() {
    local desc="$1"
    shift
    echo ""
    echo "\$ $*"
    if ! eval "$@" 2>&1; then
        echo "^^^ COMMAND FAILED ^^^"
        FAILED=1
    fi
}

# ──────────────────────────────────────────────────────────────────────
# RUST PROJECT
# ──────────────────────────────────────────────────────────────────────

header "Rust project (4 files: lib, models, service, errors)"

section "context — per-file structural summary"
run_cmd "context lib.rs"      $SMARTGREP context "$SCRIPT_DIR/rust_project/src/lib.rs"
run_cmd "context models.rs"   $SMARTGREP context "$SCRIPT_DIR/rust_project/src/models.rs"
run_cmd "context service.rs"  $SMARTGREP context "$SCRIPT_DIR/rust_project/src/service.rs"
run_cmd "context errors.rs"   $SMARTGREP context "$SCRIPT_DIR/rust_project/src/errors.rs"

section "init --dry-run — onboarding preview (writes nothing)"
run_cmd "init --dry-run" $SMARTGREP init --dry-run --project-root "$SCRIPT_DIR/rust_project"

section "map — project overview"
run_cmd "map" $SMARTGREP map --project-root "$SCRIPT_DIR/rust_project"
run_cmd "map --symbols" $SMARTGREP map --symbols --project-root "$SCRIPT_DIR/rust_project"

section "ls — kind listings"
run_cmd "ls fns"      $SMARTGREP ls fns      --project-root "$SCRIPT_DIR/rust_project"
run_cmd "ls structs"  $SMARTGREP ls structs  --project-root "$SCRIPT_DIR/rust_project"
run_cmd "ls traits"   $SMARTGREP ls traits   --project-root "$SCRIPT_DIR/rust_project"
run_cmd "ls enums"    $SMARTGREP ls enums    --project-root "$SCRIPT_DIR/rust_project"
run_cmd "ls impls"    $SMARTGREP ls impls    --project-root "$SCRIPT_DIR/rust_project"
run_cmd "ls consts"   $SMARTGREP ls consts   --project-root "$SCRIPT_DIR/rust_project"

section "show — symbol detail"
run_cmd "show User"        $SMARTGREP show User        --project-root "$SCRIPT_DIR/rust_project"
run_cmd "show UserService" $SMARTGREP show UserService  --project-root "$SCRIPT_DIR/rust_project"
run_cmd "show AppError"    $SMARTGREP show AppError     --project-root "$SCRIPT_DIR/rust_project"

section "deps/refs — cross-file relationships"
run_cmd "deps User"        $SMARTGREP deps User        --project-root "$SCRIPT_DIR/rust_project"
run_cmd "refs User"        $SMARTGREP refs User        --project-root "$SCRIPT_DIR/rust_project"
run_cmd "refs AppError"    $SMARTGREP refs AppError     --project-root "$SCRIPT_DIR/rust_project"

section "query — cross-file queries"
run_cmd "query structs with fields" \
    $SMARTGREP query '"structs | with fields"' --project-root "$SCRIPT_DIR/rust_project"
run_cmd "query implementing Validatable" \
    $SMARTGREP query '"structs implementing Validatable"' --project-root "$SCRIPT_DIR/rust_project"
run_cmd "query fns with signature" \
    $SMARTGREP query '"fns | with signature"' --project-root "$SCRIPT_DIR/rust_project"

# ──────────────────────────────────────────────────────────────────────
# GO PROJECT
# ──────────────────────────────────────────────────────────────────────

header "Go project (3 files: model, service, errors)"

section "context — per-file structural summary"
run_cmd "context model.go"   $SMARTGREP context "$SCRIPT_DIR/go_project/model.go"
run_cmd "context service.go" $SMARTGREP context "$SCRIPT_DIR/go_project/service.go"
run_cmd "context errors.go"  $SMARTGREP context "$SCRIPT_DIR/go_project/errors.go"

section "map — project overview"
run_cmd "map" $SMARTGREP map --project-root "$SCRIPT_DIR/go_project"

section "ls — kind listings"
run_cmd "ls funcs"      $SMARTGREP ls funcs      --project-root "$SCRIPT_DIR/go_project"
run_cmd "ls structs"    $SMARTGREP ls structs    --project-root "$SCRIPT_DIR/go_project"
run_cmd "ls interfaces" $SMARTGREP ls interfaces --project-root "$SCRIPT_DIR/go_project"
run_cmd "ls methods"    $SMARTGREP ls methods    --project-root "$SCRIPT_DIR/go_project"
run_cmd "ls consts"     $SMARTGREP ls consts     --project-root "$SCRIPT_DIR/go_project"
run_cmd "ls types"      $SMARTGREP ls types      --project-root "$SCRIPT_DIR/go_project"

section "show — symbol detail"
run_cmd "show User"        $SMARTGREP show User        --project-root "$SCRIPT_DIR/go_project"
run_cmd "show UserService" $SMARTGREP show UserService  --project-root "$SCRIPT_DIR/go_project"
run_cmd "show Repository"  $SMARTGREP show Repository   --project-root "$SCRIPT_DIR/go_project"

section "deps/refs"
run_cmd "deps User"    $SMARTGREP deps User    --project-root "$SCRIPT_DIR/go_project"
run_cmd "refs User"    $SMARTGREP refs User    --project-root "$SCRIPT_DIR/go_project"

section "query"
run_cmd "query structs with fields" \
    $SMARTGREP query '"structs | with fields"' --project-root "$SCRIPT_DIR/go_project"
run_cmd "query methods where parent = User" \
    $SMARTGREP query '"methods where parent = User"' --project-root "$SCRIPT_DIR/go_project"

# ──────────────────────────────────────────────────────────────────────
# JAVA PROJECT
# ──────────────────────────────────────────────────────────────────────

header "Java project (5 files: User, UserService, Repository, Validatable, Identifiable, ValidationException)"

section "context — per-file structural summary"
for f in "$SCRIPT_DIR"/java_project/src/com/example/*.java; do
    run_cmd "context $(basename "$f")" $SMARTGREP context "$f"
done

section "map — project overview"
run_cmd "map" $SMARTGREP map --project-root "$SCRIPT_DIR/java_project"

section "ls — kind listings"
run_cmd "ls classes"    $SMARTGREP ls classes    --project-root "$SCRIPT_DIR/java_project"
run_cmd "ls interfaces" $SMARTGREP ls interfaces --project-root "$SCRIPT_DIR/java_project"
run_cmd "ls methods"    $SMARTGREP ls methods    --project-root "$SCRIPT_DIR/java_project"

section "show"
run_cmd "show User"        $SMARTGREP show User        --project-root "$SCRIPT_DIR/java_project"
run_cmd "show UserService" $SMARTGREP show UserService  --project-root "$SCRIPT_DIR/java_project"

section "deps/refs"
run_cmd "deps User"    $SMARTGREP deps User    --project-root "$SCRIPT_DIR/java_project"
run_cmd "refs User"    $SMARTGREP refs User    --project-root "$SCRIPT_DIR/java_project"

section "query"
run_cmd "query classes implementing Validatable" \
    $SMARTGREP query '"classes implementing Validatable"' --project-root "$SCRIPT_DIR/java_project"
run_cmd "query interfaces with fields" \
    $SMARTGREP query '"interfaces | with fields"' --project-root "$SCRIPT_DIR/java_project"

# ──────────────────────────────────────────────────────────────────────
# TYPESCRIPT PROJECT
# ──────────────────────────────────────────────────────────────────────

header "TypeScript project (3 files: models, user-service, utils)"

section "context — per-file structural summary"
run_cmd "context models.ts"       $SMARTGREP context "$SCRIPT_DIR/ts_project/src/models.ts"
run_cmd "context user-service.ts" $SMARTGREP context "$SCRIPT_DIR/ts_project/src/services/user-service.ts"
run_cmd "context utils.ts"        $SMARTGREP context "$SCRIPT_DIR/ts_project/src/utils.ts"

section "map — project overview"
run_cmd "map" $SMARTGREP map --project-root "$SCRIPT_DIR/ts_project"
run_cmd "map --symbols" $SMARTGREP map --symbols --project-root "$SCRIPT_DIR/ts_project"

section "ls — kind listings"
run_cmd "ls functions"   $SMARTGREP ls functions   --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls classes"     $SMARTGREP ls classes     --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls interfaces"  $SMARTGREP ls interfaces  --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls enums"       $SMARTGREP ls enums       --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls types"       $SMARTGREP ls types       --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls consts"      $SMARTGREP ls consts      --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls namespaces"  $SMARTGREP ls namespaces  --project-root "$SCRIPT_DIR/ts_project"
run_cmd "ls methods"     $SMARTGREP ls methods     --project-root "$SCRIPT_DIR/ts_project"

section "show — symbol detail"
run_cmd "show User"          $SMARTGREP show User          --project-root "$SCRIPT_DIR/ts_project"
run_cmd "show UserService"   $SMARTGREP show UserService   --project-root "$SCRIPT_DIR/ts_project"
run_cmd "show Pagination"    $SMARTGREP show Pagination    --project-root "$SCRIPT_DIR/ts_project"

section "deps/refs"
run_cmd "deps User"          $SMARTGREP deps User          --project-root "$SCRIPT_DIR/ts_project"
run_cmd "refs User"          $SMARTGREP refs User          --project-root "$SCRIPT_DIR/ts_project"
run_cmd "refs Validatable"   $SMARTGREP refs Validatable   --project-root "$SCRIPT_DIR/ts_project"

section "query — cross-file queries"
run_cmd "query classes implementing Validatable" \
    $SMARTGREP query '"classes implementing Validatable"' --project-root "$SCRIPT_DIR/ts_project"
run_cmd "query interfaces with fields" \
    $SMARTGREP query '"interfaces | with fields"' --project-root "$SCRIPT_DIR/ts_project"
run_cmd "query namespaces" \
    $SMARTGREP query '"namespaces"' --project-root "$SCRIPT_DIR/ts_project"
run_cmd "query functions with signature" \
    $SMARTGREP query '"functions | with signature"' --project-root "$SCRIPT_DIR/ts_project"

# ──────────────────────────────────────────────────────────────────────
# PYTHON PROJECT
# ──────────────────────────────────────────────────────────────────────

header "Python project (4 files: shop/__init__, models/base, models/user, services/user_service)"

section "context — per-file structural summary"
run_cmd "context __init__.py"        $SMARTGREP context "$SCRIPT_DIR/python_project/src/shop/__init__.py"
run_cmd "context base.py"            $SMARTGREP context "$SCRIPT_DIR/python_project/src/shop/models/base.py"
run_cmd "context user.py"            $SMARTGREP context "$SCRIPT_DIR/python_project/src/shop/models/user.py"
run_cmd "context user_service.py"    $SMARTGREP context "$SCRIPT_DIR/python_project/src/shop/services/user_service.py"

section "map — project overview"
run_cmd "map" $SMARTGREP map --project-root "$SCRIPT_DIR/python_project"
run_cmd "map --symbols" $SMARTGREP map --symbols --project-root "$SCRIPT_DIR/python_project"

section "ls — kind listings"
run_cmd "ls defs"       $SMARTGREP ls defs       --project-root "$SCRIPT_DIR/python_project"
run_cmd "ls functions"  $SMARTGREP ls functions  --project-root "$SCRIPT_DIR/python_project"
run_cmd "ls classes"    $SMARTGREP ls classes    --project-root "$SCRIPT_DIR/python_project"
run_cmd "ls methods"    $SMARTGREP ls methods    --project-root "$SCRIPT_DIR/python_project"
run_cmd "ls consts"     $SMARTGREP ls consts     --project-root "$SCRIPT_DIR/python_project"
run_cmd "ls types"      $SMARTGREP ls types      --project-root "$SCRIPT_DIR/python_project"

section "show — symbol detail"
run_cmd "show User"          $SMARTGREP show User          --project-root "$SCRIPT_DIR/python_project"
run_cmd "show UserService"   $SMARTGREP show UserService   --project-root "$SCRIPT_DIR/python_project"
run_cmd "show register"      $SMARTGREP show register      --project-root "$SCRIPT_DIR/python_project"

section "deps/refs"
run_cmd "deps User"          $SMARTGREP deps User          --project-root "$SCRIPT_DIR/python_project"
run_cmd "refs Entity"        $SMARTGREP refs Entity        --project-root "$SCRIPT_DIR/python_project"
run_cmd "refs Validatable"   $SMARTGREP refs Validatable   --project-root "$SCRIPT_DIR/python_project"

section "query — cross-file queries"
run_cmd "query classes implementing Entity" \
    $SMARTGREP query '"classes implementing Entity"' --project-root "$SCRIPT_DIR/python_project"
run_cmd "query classes implementing Exception" \
    $SMARTGREP query '"classes implementing Exception"' --project-root "$SCRIPT_DIR/python_project"
run_cmd "query classes with fields" \
    $SMARTGREP query '"classes | with fields"' --project-root "$SCRIPT_DIR/python_project"
run_cmd "query methods where parent = User" \
    $SMARTGREP query '"methods where parent = User | show name, signature"' --project-root "$SCRIPT_DIR/python_project"
run_cmd "query defs with signature" \
    $SMARTGREP query '"defs | with signature"' --project-root "$SCRIPT_DIR/python_project"
run_cmd "query import deps" \
    $SMARTGREP query '"deps where dep_kind = import | show from, to"' --project-root "$SCRIPT_DIR/python_project"

# ──────────────────────────────────────────────────────────────────────
# CROSS-LANGUAGE
# ──────────────────────────────────────────────────────────────────────

header "Cross-language queries (using main repo fixtures)"

section "ls functions — should find fn + func + function + def"
run_cmd "ls functions" $SMARTGREP ls functions --in tests/fixtures/

section "ls fns — Rust only"
run_cmd "ls fns" $SMARTGREP ls fns --in tests/fixtures/

section "ls defs — Python only"
run_cmd "ls defs" $SMARTGREP ls defs --in tests/fixtures/

section "ls funcs — Go only"
run_cmd "ls funcs" $SMARTGREP ls funcs --in tests/fixtures/

section "query functions — cross-language"
run_cmd "query functions" \
    $SMARTGREP query '"functions where file contains fixtures/"'

# ──────────────────────────────────────────────────────────────────────
# SUMMARY
# ──────────────────────────────────────────────────────────────────────

echo ""
echo "================================================================"
if [ $FAILED -eq 0 ]; then
    echo "  ALL COMMANDS SUCCEEDED"
else
    echo "  SOME COMMANDS FAILED — review output above"
fi
echo "================================================================"

exit $FAILED
