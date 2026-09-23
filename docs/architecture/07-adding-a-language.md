---
layout: page
title: "07 — Adding a Language"
---

# Adding a Language

The three-layer architecture means adding a language is one well-scoped task: write a parser and register it. The index builder, query engine, commands, daemon, and output formatting all work unchanged.

## What you need

1. A tree-sitter grammar crate for the target language.
2. A new file `src/parser/<lang>.rs`.
3. One `Language` entry in `src/lang.rs`.
4. Possibly a kind string or two in `src/ir/kinds.rs` and `src/query/parser.rs`.
5. An `INDEX_VERSION` bump, fixtures, and tests.

The examples below use a hypothetical Kotlin parser.

## Step 1 — Add the tree-sitter grammar

In `Cargo.toml`, add the grammar crate as a dependency. It must be compatible with the `tree-sitter` version in use (currently `0.24`, with `0.23` grammars):

```toml
tree-sitter-kotlin = "0.x"
```

Most grammars follow the naming convention `tree-sitter-<language>`.

## Step 2 — Write the parser

Create `src/parser/kotlin.rs` and declare it with `pub mod kotlin;` in `src/parser/mod.rs`. The parser has one public entry point:

```rust
pub fn parse_file(path: &Path, source: &str) -> Result<Ir>
```

`path` is relative to the project root; the caller has already read the file. Inside, you:

1. Initialize the tree-sitter parser with the language grammar.
2. Parse `source` into a `tree_sitter::Tree`.
3. Walk the tree. For each node that defines a symbol, construct a `Symbol` and push it to `ir.symbols`. Use the language's **own keywords** as `kind` (`fun`, `class`, `object`, ...), not another language's — see [08 — Language-native vocabulary](08-language-native-vocabulary).
4. Emit `Dependency` values for relationships the syntax states: `Import` for imports, `Implements` for explicit supertypes (this is what the `implementing` clause queries).
5. Return `Ok(Ir { symbols, dependencies })`.

Reuse the helpers in `src/parser/common.rs` (`loc`, `node_text`, `find_child_by_kind`). `src/parser/go.rs` and `src/parser/python.rs` are good concrete references; the pattern is the same everywhere, only the tree-sitter node names differ (e.g., `function_item` in Rust vs `function_declaration` in Go).

## Step 3 — Register the language

Add an entry to `LANGUAGES` in `src/lang.rs`:

```rust
Language {
    name: "kotlin",
    display_name: "Kotlin",
    extensions: &["kt", "kts"],
    parse: parser::kotlin::parse_file,
    project_markers: &["build.gradle.kts", "settings.gradle.kts"],
    skip_dirs: &["build", ".gradle"],
    kinds: &["fun", "class", "interface", "object", "method"],
},
```

That single entry drives file collection, parser dispatch (`parse_by_extension`), project-root detection, daemon file watching, `Index::languages`, and the kind vocabulary `smartgrep init` writes for the language (`kinds` lists every kind string the parser emits; a unit test checks each one is a query term). There is no separate match arm to edit.

## Step 4 — Wire up new kinds

If the language introduces a new function-like or type-like kind:

- Add it to `FUNCTION_KINDS` or `TYPE_KINDS` in `src/ir/kinds.rs`. The `functions` umbrella term and the formatters read from there.
- Add the user-facing term (singular and plural) to `normalize_kind_term` in `src/query/parser.rs`, and to the language list in its "unknown source" error message.

Kinds that already exist (`class`, `interface`, `enum`, `method`, `const`, `type`) need nothing.

## Step 5 — Bump the index version

Increment `INDEX_VERSION` in `src/index/types.rs`. Existing on-disk indexes then fail the version check and rebuild, picking up the newly supported files.

## Step 6 — Fixtures and tests

- A small representative fixture at `tests/fixtures/Sample.kt`, exercising a function, a class, a method, an import, and a supertype.
- `tests/parser_kotlin_test.rs`, which parses the fixture and asserts symbols, kinds, and deps:

```rust
use std::path::Path;
use smartgrep::parser::kotlin::parse_file;

#[test]
fn parses_top_level_function() {
    let source = include_str!("fixtures/Sample.kt");
    let ir = parse_file(Path::new("src/Sample.kt"), source).unwrap();
    assert!(ir.symbols.iter().any(|s| s.name == "greet" && s.kind == "fun"));
}
```

- A sample project at `tests/regression/kotlin_project/` with a section in `tests/regression/run.sh`.

## Step 7 — Docs

Update the language lists and vocabulary in `CLAUDE.md`, `SKILL.md`, `AGENTS_README.md`, and `README.md`.

## That's it

Nothing else changes. The index builder sees `Ir` regardless of which parser produced it. Every command, query, and output format works on `Index` — which the builder produces from that `Ir`. The daemon picks up the new language automatically because it uses the same registry.

## Checklist

- [ ] Add tree-sitter grammar to `Cargo.toml`
- [ ] Create `src/parser/<lang>.rs` with `pub fn parse_file(path: &Path, source: &str) -> Result<Ir>`; declare it in `src/parser/mod.rs`
- [ ] Add a `Language` entry to `LANGUAGES` in `src/lang.rs`
- [ ] New kinds → `src/ir/kinds.rs` and `normalize_kind_term` in `src/query/parser.rs`
- [ ] Bump `INDEX_VERSION` in `src/index/types.rs`
- [ ] Fixture, `tests/parser_<lang>_test.rs`, and `tests/regression/<lang>_project/` + `run.sh` section
- [ ] Update CLAUDE.md, SKILL.md, AGENTS_README.md, README.md
- [ ] Run `cargo test` — all existing tests should still pass

---

Previous: [06 — Daemon](06-daemon) | Next: [08 — Language-native vocabulary](08-language-native-vocabulary)
