---
layout: page
title: "07 — Adding a Language"
---

# Adding a Language

The three-layer architecture means adding a language is one well-scoped task: write a parser. The index builder, query DSL, commands, daemon, and output formatting all work unchanged.

## What you need

1. A tree-sitter grammar crate for the target language.
2. A new file `src/parser/<lang>.rs`.
3. A few lines in `src/parser/mod.rs` to register the file extension.
4. Fixture files and tests.

## Step 1 — Add the tree-sitter grammar

In `Cargo.toml`, add the grammar crate as a dependency:

```toml
tree-sitter-python = "0.x"
```

Most grammars follow the naming convention `tree-sitter-<language>`.

## Step 2 — Write the parser

Create `src/parser/python.rs`. The parser has one public entry point:

```rust
pub fn parse_file(path: &Path) -> Result<Ir>
```

Inside, you:

1. Read the source file.
2. Initialize the tree-sitter parser with the language grammar.
3. Parse the source text into a `tree_sitter::Tree`.
4. Walk the tree. For each node kind that corresponds to a symbol definition, construct a `Symbol` and push it to a `Vec<Symbol>`.
5. For each node that is a reference (call expression, type annotation, import, etc.), construct a `Dependency` and push it to a `Vec<Dependency>`.
6. Return `Ok(Ir { symbols, dependencies })`.

Look at `src/parser/rust.rs` or `src/parser/go.rs` as concrete references. The pattern is the same in both; only the tree-sitter node names differ (e.g., `function_item` in Rust vs `function_declaration` in Go).

## Step 3 — Register the extension

In `src/parser/mod.rs`, add an arm to the dispatch function that maps `.py` (or whatever the extension is) to your parser:

```rust
"py" | "pyw" => python::parse_file(path),
```

## Step 4 — Add fixtures and tests

Create a small representative source file at `tests/fixtures/sample.py`. It should exercise:

- A function definition
- A struct/class definition
- A method
- A function call or type reference (to test dependency extraction)

Write tests in `src/parser/python.rs` (or a dedicated test file) that call `parse_file` on the fixture and assert the expected symbols and dependencies.

```rust
#[test]
fn test_parse_python_function() {
    let ir = parse_file(Path::new("tests/fixtures/sample.py")).unwrap();
    let names: Vec<_> = ir.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"my_function"));
}
```

## That's it

Nothing else changes. The index builder sees `Ir` regardless of which parser produced it. Every command, query, and output format works on `Index` — which the builder produces from that `Ir`. The daemon picks up the new language automatically because it calls the same parser dispatch.

## Checklist

- [ ] Add tree-sitter grammar to `Cargo.toml`
- [ ] Create `src/parser/<lang>.rs` with `pub fn parse_file(path: &Path) -> Result<Ir>`
- [ ] Register file extension(s) in `src/parser/mod.rs`
- [ ] Add `tests/fixtures/sample.<ext>` with representative code
- [ ] Write parser unit tests against the fixture
- [ ] Run `cargo test` — all existing tests should still pass

---

Previous: [06 — Daemon](06-daemon)
