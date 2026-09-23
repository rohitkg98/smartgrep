# smartgrep development guide

This repository is a Rust (edition 2021) CLI for structural code navigation. Keep this file and `CLAUDE.md` consistent when changing project conventions; `CLAUDE.md` has the detailed query vocabulary, architecture notes, roadmap workflow, and release procedure.

## Devbox and accounts

- Persistent checkout: `/home/exedev/workspace/smartgrep`. Browser editor: code-server on port 8080, opened at this checkout. Desktop VS Code (`code`) is also installed, but this VM is headless.
- Rust comes from rustup; Node.js/npm, Go, Python 3, git, `gh`, and common build tools are available. The Rust binary and cargo live in `~/.cargo/bin`.
- Git `origin` uses the VM's GitHub integration: `https://github.int.exe.xyz/rohitkg98/smartgrep.git`. Use this remote for clone/fetch/push. `gh` is a separate authentication flow and may require `gh auth login` and the `project` scope for roadmap operations.
- Claude Code is installed. To use the **Claude Pro subscription**, run `claude auth login --claudeai` in a terminal and sign in with your Claude account, then run `claude` from the repository. Do not set `ANTHROPIC_API_KEY` or `ANTHROPIC_BASE_URL` for this workflow: those select API billing/gateway access instead of the subscription. Never commit credentials.

## Build and run

```bash
cargo build --locked                       # binary: target/debug/smartgrep
cargo run -- context src/main.rs           # structural overview of a file
cargo run -- ls functions                  # list functions in the project
cargo run -- show Index                    # inspect a symbol
cargo run -- query "structs | with fields" # composable structural query
cargo run -- index                         # force a fresh index (normally implicit)
cargo install --path . --locked            # optional: put smartgrep on PATH
```

Run commands from the repository root unless providing `--project-root`. The generated `.smartgrep/`, `target/`, `.claude/`, and `.vscode/` directories are ignored.

## Tests and checks

```bash
cargo fmt --all -- --check
cargo test --locked
cargo build --locked
SMARTGREP=./target/debug/smartgrep bash tests/regression/run.sh
```

Run tests and the regression script before committing (both run in CI). `tests/parser_*_test.rs` exercise parser fixtures; index builder tests use hand-built IR; command tests use hand-built indexes. `tests/regression/run.sh` runs the built CLI against multi-file sample projects. The current checkout has pre-existing `cargo fmt --all -- --check` differences; do not reformat unrelated files just to make that check pass.

## Architecture and conventions

- Data flow: tree-sitter parser → language-neutral IR (`src/ir/`) → index builder (`src/index/`) → commands (`src/commands/`) → text/JSON formatters (`src/format/`).
- `src/lang.rs` registers extensions, parser, project markers, and skipped directories for each language. `src/parser/` contains Rust, Java, Go, TypeScript, and Python parsers. `src/query/` handles the query DSL; `src/cli.rs` defines the CLI.
- Preserve the IR/index contracts: parsers produce IR; the builder consumes IR and produces the queryable index; commands consume the index. Add fixtures and tests at the layer being changed.
- For structural exploration, prefer the local `smartgrep` CLI (`context`, `ls`, `show`, `deps`, `refs`, or `query`) over broad grep. Read source files when exact implementation details matter. Scope queries to directories on large trees.
- To add a language: add a compatible tree-sitter crate; implement its parser; register it in `src/lang.rs`; extend kind classification and query normalization as needed; bump `INDEX_VERSION`; add parser fixtures, regression project coverage, and documentation. See `CLAUDE.md` for the detailed checklist.

## Changes and releases

- Keep commits focused with a short lowercase subject. Each commit should pass tests. Add user-visible changes to the `## [Unreleased]` section of `CHANGELOG.md` under Added/Changed/Fixed/Removed; internal-only changes need no changelog entry.
- The GitHub Project **smartgrep roadmap** is the work queue; board order is priority. See `CLAUDE.md` and `scripts/roadmap.sh` for issue/status workflow. If `gh` is not authenticated, do not pretend the board was updated.
- Use `scripts/release.sh <version> --dry-run` before a release; consult `CLAUDE.md` for the full release process.
