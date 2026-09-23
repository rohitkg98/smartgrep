# smartgrep — Structural code navigation for agents

## What this is
Language-aware CLI for coding agents. Parses source via tree-sitter to extract structural symbols (functions, structs, traits, impls, classes, methods) and presents them as greppable text output. Like an IDE's symbol browser, but for CLI agents — low-token structural queries instead of reading entire files.

## Tech
- Rust (cargo, edition 2021)
- tree-sitter for multi-language source parsing
- Rust 1.70+ required

## Commands
```bash
cargo test                                # run all tests
cargo run -- context src/main.rs          # structural summary of a file
cargo run -- ls functions                 # list all functions
cargo run -- ls structs                   # list all structs
cargo run -- show <name>                  # detail for a symbol
cargo run -- deps <name>                  # what does X depend on?
cargo run -- refs <name>                  # what references X?
cargo run -- index                        # force re-index (usually implicit)
cargo run -- init --dry-run               # preview onboarding: index + CLAUDE.md block + skill + .gitignore
```

## Architecture: 3 layers, 2 contracts

```
Parser (tree-sitter) → IR → Index Builder → Index → Command
```

- **IR** (`src/ir/types.rs`): Language-agnostic symbol/dependency maps. Parsers produce, builder consumes.
- **Index** (`src/index/types.rs`): Queryable structure with lookup tables. Builder produces, commands consume.

## Key design decisions
- IR exists so we can add languages by writing one parser, without changing the index builder or commands
- Parsers are tested against fixture files — small .rs/.java/.go/.ts/.py files in tests/fixtures/
- Index builder is tested with hand-built IR — no parsing needed
- Commands are tested with hand-built Index — no builder needed
- Auto-indexing: queries trigger indexing implicitly. Re-indexes when source files change.

## File layout
- `src/ir/` — IR types and validation
- `src/lang.rs` — language registry: extensions, parser fn, project markers, skip dirs per language
- `src/parser/` — tree-sitter parsers (rust.rs, java.rs, go.rs, typescript.rs, python.rs); `mod.rs` has `parse_by_extension` dispatch (via the registry)
- `src/ir/kinds.rs` — kind classification helpers (`is_function_kind`, `is_type_kind`, `kind_rank`)
- `src/index/` — index types, builder, storage, auto-detection
- `src/commands/` — CLI commands (context, ls, show, deps, refs)
- `src/format/` — text table and JSON output
- `tests/fixtures/` — small source files for parser tests

## Adding a new language
1. Add the tree-sitter grammar crate to `Cargo.toml` (must be compatible with the `tree-sitter` version in use).
2. Write `src/parser/<lang>.rs` exposing `pub fn parse_file(path: &Path, source: &str) -> Result<Ir>`; declare it in `src/parser/mod.rs`. Use the language's own keywords as `kind` strings; reuse `src/parser/common.rs` helpers.
3. Add a `Language { name, display_name, extensions, parse, project_markers, skip_dirs, kinds }` entry to `LANGUAGES` in `src/lang.rs`. That single entry drives file collection, parser dispatch, project-root detection, daemon file watching, `Index::languages`, and the vocabulary line `smartgrep init` writes (`kinds` = every kind string the parser emits).
4. If the language introduces a new function-like or type-like kind, add it to `FUNCTION_KINDS` / `TYPE_KINDS` in `src/ir/kinds.rs` (the `functions` umbrella and formatters read from there), and add the user-facing term to `normalize_kind_term` in `src/query/parser.rs`.
5. Bump `INDEX_VERSION` in `src/index/types.rs` so existing indexes rebuild and pick up the new files.
6. Tests: a fixture in `tests/fixtures/`, `tests/parser_<lang>_test.rs`, and a `tests/regression/<lang>_project/` with a section in `tests/regression/run.sh`.
7. Docs: language lists / vocabulary in CLAUDE.md, SKILL.md, AGENTS_README.md, README.md.

## Code Navigation
Always use `smartgrep` for structural code exploration on this project. It is faster and more token-efficient than reading files or grepping.

### Language-native vocabulary
Symbols use language-native kind strings, not a shared enum:
- **Rust:** fn, method, struct, enum, trait, impl, const, type, mod
- **Java:** class, interface, enum, method, record
- **Go:** func, method, struct, interface, const, type
- **TypeScript:** function, class, interface, enum, type, method, const, namespace
- **Python:** def, class, method, const, type

Dependency kinds emitted: Import, Call (per function/method body; instance calls reduced to the method name), Implements. `refs`/`implementing` match by name: bare names match the last path segment (`refs Index` finds `use crate::index::types::Index`), qualified names match as a suffix (`refs User::new`, `refs fmt.Errorf`). Name-based, not type-resolved: `refs new` returns every `new` call.

### Cross-language queries
Umbrella terms find symbols across all languages:
- `functions` → finds Rust `fn`, Go `func`, TS `function`, Python `def`
- `fns` → Rust only, `funcs` → Go only, `function` → TS only, `defs` → Python only
Language-specific terms target one language. Shared terms like `structs`, `interfaces`, `enums` work as before.

### Prefer smartgrep query for compound questions
```bash
# Instead of multiple grep/read calls, compose one query:
smartgrep query "structs where file contains 'ir/' and visibility = public | with fields"
smartgrep query "functions where name = 'run' and file contains 'commands/' | show name, file, signature"
smartgrep query "symbol Index | with deps, refs"
smartgrep query "deps where from contains 'parser' | show from, to, dep_kind"

# Find types implementing a trait/interface:
smartgrep query "structs implementing Display"
```

### Use basic commands for simple lookups
```bash
smartgrep context src/main.rs       # structural overview of a file
smartgrep ls functions              # list all functions
smartgrep ls structs                # list all structs
smartgrep ls interfaces             # list all interfaces (Go) / traits (Rust)
smartgrep ls structs --in src/ir/   # list structs in specific path
smartgrep show <name>               # detail for a symbol
smartgrep deps <name>               # what does X depend on?
smartgrep refs <name>               # what references X?
```

### Large codebases (100+ files): always scope your queries
On large projects, bare `ls` commands dump thousands of symbols. Always filter:
```bash
# BAD: floods context with 500KB+ of output
smartgrep ls functions

# GOOD: scope with --in or use query with file filtering
smartgrep ls functions --in go/services/
smartgrep query "structs in 'go/services/' | with fields"
smartgrep query "interfaces where file contains 'common/' | with fields"
smartgrep query "functions where name starts_with 'New' and file contains 'services/'"
```

### Language notes
- **Go/Java/TS interfaces** → use `interfaces` (kind="interface")
- **Rust traits** → use `traits` (kind="trait", Rust only)
- **`interfaces` and `traits` are distinct** — `interfaces` matches Java/Go/TS interface, `traits` matches Rust trait
- **Go method receivers** → stored in `parent` field (e.g., `methods where parent = MultiGateway`)
- **Generated code** → filter out with `where file not contains '.pb.go'` (`not` also works before `starts_with` / `ends_with`); `map` skips generated files by default
- **`implementing` clause** → `structs implementing Display` finds types that implement a trait/interface
- **TS decorators** → stored in `attributes` (e.g., `classes where attributes contains '@Injectable'`)
- **TS namespaces** → `namespaces` or `namespace` (kind="namespace", TS only)
- **node_modules** → automatically skipped during indexing
- **Python** → `def` = module-level function, `method` = function in a class body (nested functions skipped); base classes are `Implements` deps (`classes implementing BaseModel`); decorators in `attributes`; `const` = module-level UPPER_SNAKE assignment; `type` = `type X = ...`, `X: TypeAlias = ...`, `NewType`, or PascalCase `X = Union[...]`; qualified names are dotted module paths (`src/` stripped, `__init__` dropped)
- **Python venvs/caches** (`venv`, `.venv`, `__pycache__`, `site-packages`, `.tox`, ...) → automatically skipped

### When to use smartgrep vs file reading
- **Use smartgrep**: finding symbols, understanding structure, exploring dependencies, listing functions/structs
- **Read files directly**: when you need the full implementation body, line-by-line logic, or exact syntax

## Agent workflow
- The main agent is the manager. It delegates all research and implementation to agent teams.
- Use agent teams for research (codebase exploration, understanding existing code, gathering context).
- Use agent teams for implementation (writing code, editing files, running tests).
- The main agent focuses on decision-making, coordination, and communicating with the user.

## Roadmap board
GitHub Project "smartgrep roadmap" (https://github.com/users/rohitkg98/projects/2). Every item is a real issue in this repo; **board order is priority** (top = next). Always go through the script — don't create draft items (they don't show up reliably).
```bash
scripts/roadmap.sh list                                   # priority order, issue numbers, status
scripts/roadmap.sh add "<title>" body.md --after 12       # new issue + board item (status Todo)
scripts/roadmap.sh status 6 "In Progress"                 # Todo | In Progress | Done
scripts/roadmap.sh move 13 --after 7                      # or --top
gh issue view 6 / gh issue edit 6 --body-file body.md     # read/edit item details
```
- Keep the board in sync as work ships. New work that isn't on the board gets an item first (`add`), so the release can close it.
- Starting work on an item: set it `In Progress`. Reference it in commits (`Refs #6`); use `Closes #6` in the commit that finishes it (the board's auto-close workflow moves it to Done).
- Needs `gh` with the `project` scope (`gh auth refresh -s project`).
- If `list` misses an item you just created, check https://www.githubstatus.com — the project item listing can lag during GitHub incidents even though the item exists (`gh issue view <N> --json projectItems`).

## Commits and releases
- Commit style: short lowercase subject (`added python support`, `bump version to 0.2.1`), optional bullet body. One logical change per commit — split refactors from features, and make sure each commit passes `cargo test` on its own.
- Changelog: every user-visible change adds a line under `## [Unreleased]` in `CHANGELOG.md` (Added / Changed / Fixed / Removed), in the same commit as the change. Link the board issue when there is one. Internal-only changes (tests, CI, refactors with no behavior change) don't need an entry.
- Before committing: `cargo test` and `SMARTGREP=./target/debug/smartgrep bash tests/regression/run.sh` (CI runs both).
- Release: `scripts/release.sh <X.Y.Z>` (try `--dry-run` first). It refuses to run if `Unreleased` is empty, moves those entries under `## [X.Y.Z] - date` (they become the GitHub release notes), then checks the tree is clean and in sync on `main`, runs tests + regression, bumps `Cargo.toml`/`Cargo.lock`, commits `bump version to X.Y.Z`, tags `vX.Y.Z`, pushes, then waits for `.github/workflows/release.yml` and verifies every asset is attached (one archive per target in the release.yml build matrix, listed by `scripts/release-assets.sh`, plus `SHA256SUMS`). Run the release workflow on a branch without publishing: `gh workflow run release.yml --ref <branch>`. Finally it keeps the board in sync: every issue closed by a commit in the release (`Closes #N` / `Fixes #N`) gets a "Shipped in vX.Y.Z" comment and is set to Done, and it prints what's still In Progress.
- Versioning: minor bump for new languages/commands or index format changes, patch for fixes.
