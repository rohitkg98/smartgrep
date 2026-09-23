---
layout: page
title: "04 — Index"
---

# Index

**Files:** `src/index/types.rs`, `src/index/builder.rs`, `src/index/store.rs`, `src/index/auto.rs`

## What the index is

The `Index` is the queryable form of a codebase's structure. It holds the same symbols and dependencies as the `Ir`, but adds four lookup tables that make queries fast without scanning all symbols on every call.

```rust
pub const INDEX_VERSION: u32 = 6;

pub struct Index {
    pub version: u32,                                     // must equal INDEX_VERSION on load
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

It clones the symbols and deps from the `Ir` once, adds the type-reference deps it derives from the symbols, then builds the four maps in a single pass over each list:

1. **Type references.** For each callable (`fn`/`func`/`function`/`def`/`method`), a `TypeRef` dep to every project type named in its parameter types or return type; for each type-kind symbol with fields, a `FieldType` dep to every project type named in its field types. Type strings are split into identifier paths (anything other than `[A-Za-z0-9_]`, `::` or `.` ends a token: `&mut crate::x::Index` → `mut`, `crate::x::Index`; `map[int64]*User` → `map`, `int64`, `User`), and a token is kept only if its last segment is the name of a type-kind symbol in the project (`ir::kinds::is_type_kind`). That drops primitives, wrappers (`Option`, `Vec`, `List`, `Promise`, `Iterable`) and external types. `to_name` is the token as written, deduped per `(from, to_name)`; recursive types (`next: Option<Box<Node>>` in `Node`) keep their self-reference. `loc` is the symbol's own location (params carry none). Derived deps are placed after their file's parser deps, so output stays grouped by file. This is done once here rather than in each parser, since every parser already fills `params`, `return_type` and `fields`.
2. For each symbol at index `i`: insert `i` into `name_lookup[sym.name]`, `file_lookup[sym.loc.file]`, and `qualified_lookup[sym.qualified_name]`.
3. For each dependency at index `i`: insert `i` into `reverse_deps[dep_target_key(dep.to_name)]` — the last path segment with generics stripped (`crate::index::types::Index` → `Index`, `Processor<String>` → `Processor`, `os.path.join` → `join`). A qualified `Call` is also indexed under its owner segment (`User::new` → `User`), so `refs User` lists `User::new(..)` call sites. Normalization lives in `src/ir/names.rs`.

No parsing, no I/O, no language knowledge. Just hash map construction.

## How queries resolve symbols

| Query | Method | Lookup used |
|-------|--------|-------------|
| `show Foo` | `by_name("Foo")` | `name_lookup` |
| `ls functions --in src/` | `by_kinds(FUNCTION_KINDS)` then path-substring filter | linear scan on `symbols` |
| `deps Foo` | `by_name("Foo")`, then `deps_of(qualified_name)` for each | `name_lookup` + linear scan on `deps` |
| `refs Foo` | `refs_to("Foo")` | `reverse_deps` |

`by_kind` / `by_kinds` and `deps_of` are linear scans (kind strings are matched directly — see [08](08-language-native-vocabulary)). For most codebases this is fast enough; the reverse direction (`refs_to`) uses the pre-built `reverse_deps` map. A bare name (`refs validate`) is a key lookup; a qualified name (`refs User::new`, `refs fmt.Errorf`) looks up its last segment and keeps deps whose path ends with the query's segments — `::`, `.` and `/` are treated as the same separator. Matching is by name, not resolved through imports or types: `refs new` returns every `new` call, and `refs Index` lists every function taking or returning an `Index` and every type with an `Index` field (`type_ref` / `field_type`), whichever `Index` that is.

## Testing the index builder

Index builder tests construct `Ir` values by hand (no file parsing) and assert that the resulting `Index` has the expected lookup entries. This isolates builder correctness from parser correctness.

## Storage and auto-indexing

The index is serialized as JSON to `<project root>/.smartgrep/index.json` (`src/index/store.rs`). The project root is found by walking up to the nearest language marker file from the registry (`Cargo.toml`, `go.mod`, `package.json`, `pyproject.toml`, ...).

On each command invocation, `ensure_index` checks whether any source file is newer than the index file. If so — or if the file fails to deserialize, or its `version` differs from `INDEX_VERSION` — it re-parses every source file and rebuilds transparently. `INDEX_VERSION` is bumped whenever the schema or the set of indexed languages changes (v3 added Python, v6 added derived type references), so old indexes never need a manual `smartgrep index`. The opt-in daemon (see [06 — Daemon](06-daemon)) avoids the per-invocation check by keeping the index in memory.

---

Previous: [03 — Parsers](03-parsers) | Next: [05 — Query DSL](05-query-dsl)
