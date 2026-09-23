---
layout: page
title: "03 — Parsers"
---

# Parsers

**Files:** `src/parser/{rust,java,go,typescript,python}.rs`, `src/parser/common.rs`, `src/parser/mod.rs`, `src/lang.rs`

## tree-sitter

Every parser uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) to turn source text into a concrete syntax tree. tree-sitter grammars are language-specific; the parsing logic in smartgrep is the thin layer that walks those trees and emits `Ir`.

Tree-sitter gives us:

- **Correctness** — a real parse, not regex. Comments, strings, and macro invocations don't confuse it.
- **Speed** — parsing is fast enough to re-parse the whole project when a file changes.
- **Grammar reuse** — the tree-sitter ecosystem has grammars for dozens of languages.

## What a parser does

A parser receives a file path (relative to the project root) and its source text, and returns an `Ir`. Internally it:

1. Passes the text to the tree-sitter parser for its language.
2. Walks the resulting syntax tree.
3. At each node of interest (function definition, struct/class definition, impl block, etc.) it constructs a `Symbol` — with the language's own keyword as `kind` — and pushes it onto a `Vec<Symbol>`.
4. When it encounters a relationship it records (an import, or an `impl Trait for T` / `extends` / `implements` / base class) it constructs a `Dependency` and pushes it onto a `Vec<Dependency>`. Function calls and type references are not extracted yet.
5. Returns `Ir { symbols, dependencies }`.

```
source text
    │
    ▼
tree-sitter parse
    │
    ▼
syntax tree (language-specific nodes)
    │
    ▼
smartgrep walk  ──→  Symbol, Symbol, Symbol, ...
                ──→  Dependency, Dependency, ...
    │
    ▼
Ir { symbols: [...], dependencies: [...] }
```

## One parser per language

The `src/parser/` directory has one file per language:

| File | Language |
|------|----------|
| `rust.rs` | Rust (`.rs`) |
| `java.rs` | Java (`.java`) |
| `go.rs` | Go (`.go`) |
| `typescript.rs` | TypeScript (`.ts`, `.tsx`) |
| `python.rs` | Python (`.py`, `.pyi`) |

Shared tree-sitter helpers (`loc`, `node_text`, `find_child_by_kind`) live in `common.rs`.

Each file exports a single entry point:

```rust
pub fn parse_file(path: &Path, source: &str) -> Result<Ir>
```

Dispatch is table-driven. `src/lang.rs` holds a `LANGUAGES` registry; each `Language` entry lists its name, file extensions, `parse` function, project-root marker files, and directories to skip. `parse_by_extension` in `src/parser/mod.rs` just looks up the entry for a path's extension and calls its `parse`. The same registry drives source collection, project-root detection, and the daemon's file watcher.

## Testing parsers

Parsers are tested against fixture files in `tests/fixtures/` (one per language) from `tests/parser_<lang>_test.rs`. A fixture is a small, representative source file. The test asserts that the parser produces the expected symbols and dependencies from that fixture.

This approach keeps parser tests independent of the index builder and commands. A parser can be tested and debugged in isolation. End-to-end behaviour is covered by small sample projects under `tests/regression/` driven by `tests/regression/run.sh`.

---

Previous: [02 — IR](02-ir) | Next: [04 — Index](04-index)
