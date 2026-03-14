---
layout: page
title: "03 — Parsers"
---

# Parsers

**Files:** `src/parser/rust.rs`, `src/parser/go.rs`, `src/parser/java.rs`

## tree-sitter

Every parser uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) to turn source text into a concrete syntax tree. tree-sitter grammars are language-specific; the parsing logic in smartgrep is the thin layer that walks those trees and emits `Ir`.

Tree-sitter gives us:

- **Correctness** — a real parse, not regex. Comments, strings, and macro invocations don't confuse it.
- **Speed** — incremental parsing is fast enough to re-parse on file change.
- **Grammar reuse** — the tree-sitter ecosystem has grammars for dozens of languages.

## What a parser does

A parser receives the text of a source file and returns an `Ir`. Internally it:

1. Passes the text to the tree-sitter parser for its language.
2. Walks the resulting syntax tree.
3. At each node of interest (function definition, struct definition, impl block, etc.) it constructs a `Symbol` and pushes it onto a `Vec<Symbol>`.
4. When it encounters a reference (a function call, a type used in a field, a trait impl) it constructs a `Dependency` and pushes it onto a `Vec<Dependency>`.
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
| `rust.rs` | Rust |
| `go.rs` | Go |
| `java.rs` | Java |

Each file exports a single entry point: a function that takes a file path (or source text) and returns `Result<Ir>`. The dispatch logic in `src/parser/mod.rs` maps file extensions to parser functions.

## Testing parsers

Parsers are tested against fixture files in `tests/fixtures/`. A fixture is a small, representative source file. The test asserts that the parser produces the expected symbols and dependencies from that fixture.

This approach keeps parser tests independent of the index builder and commands. A parser can be tested and debugged in isolation.

---

Previous: [02 — IR](02-ir) | Next: [04 — Index](04-index)
