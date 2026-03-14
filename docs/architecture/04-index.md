---
layout: page
title: "04 — Index"
---

# Index

**Files:** `src/index/types.rs`, `src/index/builder.rs`

## What the index is

The `Index` is the queryable form of a codebase's structure. It holds the same symbols and dependencies as the `Ir`, but adds four lookup tables that make queries fast without scanning all symbols on every call.

```rust
pub struct Index {
    pub symbols: Vec<Symbol>,
    pub deps: Vec<Dependency>,

    pub name_lookup:      HashMap<String, Vec<usize>>,   // name → symbol indices
    pub file_lookup:      HashMap<PathBuf, Vec<usize>>,  // file → symbol indices
    pub qualified_lookup: HashMap<String, usize>,         // qualified name → symbol index
    pub reverse_deps:     HashMap<String, Vec<usize>>,   // target name → dep indices
}
```

Symbols and deps are stored as `Vec`; lookup tables store integer indices into those vecs. This avoids cloning data at query time — a lookup returns `&Symbol` references into the same backing store.

## How the builder works

`src/index/builder.rs` exposes a single function:

```rust
pub fn build(ir: &Ir) -> Index
```

It clones the symbols and deps from the `Ir` once, then builds the four maps in a single pass over each list:

1. For each symbol at index `i`: insert `i` into `name_lookup[sym.name]`, `file_lookup[sym.loc.file]`, and `qualified_lookup[sym.qualified_name]`.
2. For each dependency at index `i`: insert `i` into `reverse_deps[dep.to_name]`.

No parsing, no I/O, no language knowledge. Just hash map construction.

## How queries resolve symbols

| Query | Method | Lookup used |
|-------|--------|-------------|
| `show Foo` | `by_name("Foo")` | `name_lookup` |
| `ls functions --in src/` | `by_file(path)` then filter | `file_lookup` |
| `deps Foo` | `deps_of("mod::Foo")` | linear scan on `deps` |
| `refs Foo` | `refs_to("Foo")` | `reverse_deps` |

`deps_of` does a linear scan over `deps` filtering on `from_qualified`. For most codebases this is fast enough; the reverse direction (`refs_to`) uses the pre-built `reverse_deps` map.

## Testing the index builder

Index builder tests construct `Ir` values by hand (no file parsing) and assert that the resulting `Index` has the expected lookup entries. This isolates builder correctness from parser correctness.

## Storage and auto-indexing

The index is serialized as JSON to a cache file (`.smartgrep/index.json` or similar). On each command invocation, smartgrep checks whether any source file has been modified since the cache was written. If so, it re-indexes transparently. The daemon mode (see [06 — Daemon](06-daemon)) avoids this per-invocation check by keeping the index in memory.

---

Previous: [03 — Parsers](03-parsers) | Next: [05 — Query DSL](05-query-dsl)
