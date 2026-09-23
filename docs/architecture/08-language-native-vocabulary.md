---
layout: page
title: "08 — Language-native vocabulary"
---

# Language-native vocabulary

**Files:** `src/ir/types.rs`, `src/ir/kinds.rs`, `src/query/parser.rs`, `src/query/engine.rs`

## Why not a shared enum?

smartgrep started with a single `SymbolKind` enum (`Function`, `Struct`, `Trait`, `Impl`, ...) used for every language. That enum was Rust's vocabulary: a Java class was stored as a "struct", a Java or Go interface as a "trait". Anyone querying a Java or Go codebase had to learn a Rust-to-target mapping that has nothing to do with the code in front of them.

The enum was replaced. `Symbol.kind` is now a plain `String` holding the word the language itself uses, and the query DSL uses those same words as source terms.

## Design principles

- **The grammar is universal.** Predicates, pipelines, `in`, `where`, `implementing`, `|` stages — the same for every language.
- **The vocabulary is language-native.** `classes` for Java/TS/Python, `funcs` for Go, `fns` for Rust, `defs` for Python.
- **The IR stores what the parser sees**, not an abstraction over it. A Go `func` stays a `func`.
- **No aliases that paper over real differences.** `interfaces` means kind `interface` (Java/Go/TS); `traits` means kind `trait` (Rust only). They are not the same thing and are not merged.
- **Umbrella terms only where the concept is genuinely shared.** Currently just `functions`.
- **Honest gaps.** Where syntax doesn't state a relationship (Go interface satisfaction), smartgrep says so instead of guessing.

## Native kind strings per language

These are the `kind` values each parser actually emits:

| Language | Kind strings |
|---|---|
| Rust | `fn`, `method`, `struct`, `enum`, `trait`, `impl`, `const`, `type`, `mod` |
| Java | `class`, `interface`, `enum`, `record`, `method` |
| Go | `func`, `method`, `struct`, `interface`, `const`, `type` |
| TypeScript | `function`, `method`, `class`, `interface`, `enum`, `type`, `const`, `namespace` |
| Python | `def`, `method`, `class`, `const`, `type` |

Notes:

- `method` is shared: any function with an enclosing type. The type goes in `parent` (Rust impl target, Go receiver, Java/TS/Python class).
- Rust `impl` blocks are kept as symbols (named `impl Display for Foo`) *and* produce an `Implements` dep.
- TS arrow functions assigned to a `const` are emitted as `function`.
- Python `const` is a module-level `UPPER_CASE` assignment; `type` covers `TypeAlias`, PEP 695 `type X = ...`, `NewType`, and PascalCase subscript aliases.

Code that needs to treat kinds by category uses `src/ir/kinds.rs` rather than matching strings locally:

```rust
pub const FUNCTION_KINDS: &[&str] = &["fn", "func", "function", "def"];
pub const TYPE_KINDS: &[&str] = &["struct", "class", "record", "enum", "trait",
                                  "interface", "annotation", "type"];
// is_function_kind, is_callable_kind (adds "method"), is_type_kind,
// has_data_fields, kind_rank (display ordering)
```

## DSL source terms

`normalize_kind_term` in `src/query/parser.rs` maps a user-facing term (singular or plural, case-insensitive) to one kind string. `normalize_kind_filter` wraps it and expands umbrella terms into a list. `ls <type>` and `query "<type> ..."` share the same table.

| Term(s) | Kind | Matches |
|---|---|---|
| `fn`, `fns` | `fn` | Rust |
| `struct`, `structs` | `struct` | Rust, Go |
| `trait`, `traits` | `trait` | Rust |
| `impl`, `impls` | `impl` | Rust |
| `mod`, `mods`, `module`, `modules` | `mod` | Rust |
| `class`, `classes` | `class` | Java, TS, Python |
| `record`, `records` | `record` | Java |
| `annotation`, `annotations` | `annotation` | (reserved; no parser emits it yet) |
| `func`, `funcs` | `func` | Go |
| `function` | `function` | TypeScript |
| `namespace`, `namespaces` | `namespace` | TypeScript |
| `def`, `defs` | `def` | Python |
| `method`, `methods` | `method` | all |
| `interface`, `interfaces` | `interface` | Java, Go, TS |
| `enum`, `enums` | `enum` | Rust, Java, TS |
| `const`, `consts`, `constants` | `const` | Rust, Go, TS, Python |
| `type`, `types` | `type` | Rust, Go, TS, Python |
| **`functions`** | **`FUNCTION_KINDS`** | umbrella: `fn` + `func` + `function` + `def` |

Plus the non-kind sources: `symbols`, `symbol <name>`, `deps [<name>]`, `refs [<name>]`.

Note the asymmetry between `function` and `functions`: the singular is TypeScript's own keyword; the plural is the cross-language umbrella. An unknown term produces an error that lists the valid terms per language.

## The `implementing` clause

```
<source> [in <path>] [implementing <name>] [where <conditions>] [| <stages>]
```

`implementing` keeps only symbols that are the `from` side of an `Implements` dependency matching `<name>`. It uses the same name matching as `refs` (see [04](04-index)): a bare name matches the last path segment with generics stripped, a qualified name matches as a segment suffix. So `implementing Display` finds `impl fmt::Display for X`, and `implementing Processor` finds `implements Processor<String>`:

```rust
let implementors: HashSet<&str> = index.refs_to(name).into_iter()
    .filter(|d| d.kind == DepKind::Implements)
    .map(|d| d.from_qualified.as_str())
    .collect();
symbols.retain(|s| implementors.contains(s.qualified_name.as_str()));
```

So it works wherever a parser emits `Implements` deps — i.e. wherever the relationship is written in the source:

| Language | Source of `Implements` deps |
|---|---|
| Rust | `impl Trait for Type` (from = the type's qualified name) |
| Java | `class ... extends X implements Y`; `implements` on enums and records |
| TypeScript | `extends` / `implements` clauses on classes and interfaces |
| Python | every base class in `class Foo(Base, Protocol)` — Python has no separate "implements", so inheritance is recorded as `Implements` |
| Go | none |

```bash
smartgrep query "structs implementing Display"      # Rust
smartgrep query "classes implementing Processor"    # Java / TS
smartgrep query "classes implementing BaseModel"    # Python
```

### Go: structural typing

Go types satisfy interfaces implicitly, by having the right methods. Nothing in the syntax says "X implements Y", so the Go parser emits no `Implements` deps. Rather than inventing them, `funcs implementing <name>` returns an error that points at a workable alternative:

```
Go uses structural typing — `implementing` is not valid for Go.
To find types that satisfy an interface, check which types have the required methods:
  methods where name = <MethodName> | show parent, file
```

## Trade-offs and known gaps

- **More terms to learn.** An agent must know that Go says `funcs` and Python says `defs`. The per-language error message and the `functions` umbrella soften this; the payoff is that terms match the code being read.
- **`implementing` is name-based, not resolved.** Matching is by path segments, not by following imports, so `implementing std::fmt::Display` does not match an impl written as `fmt::Display` (use the bare `Display`), and two unrelated traits with the same last segment are indistinguishable. Rust impls on generic types (`impl<T> Foo<T>`) record `from` as `mod::Foo<T>`, which doesn't match the struct's qualified name.
- **Go error is narrow.** It fires only when every kind in the filter is `func`. Since `structs`/`interfaces`/`types` are shared with other languages, `structs implementing Writer` on a Go project just returns no results rather than the explanatory error.
- **Rust `impl` symbols share the type's qualified name**, so `symbols implementing Foo` also returns the matching `impl` rows; use `structs`/`enums` to get just the types.
- **Java interface inheritance** (`interface A extends B`) is not recorded as `Implements`.
- **Reserved kinds.** `annotation` exists in the term table and `TYPE_KINDS`, but no parser emits it yet (Java `@interface` declarations are not indexed).
- **Only `functions` is an umbrella.** There is no cross-language umbrella for type definitions (`TYPE_KINDS` is used by formatters, not the DSL); `ls structs` will not show Java classes.
- **Index format break.** Switching `kind` to a string and renaming `DepKind` variants (`FunctionCall` → `Call`, `TypeReference` → `TypeRef`, `TraitImpl` → `Implements`) changed the on-disk JSON. The `version` field on `Index` handles this: a mismatch with `INDEX_VERSION` triggers a transparent rebuild.

---

Previous: [07 — Adding a Language](07-adding-a-language)
