---
layout: page
title: "05 — Query DSL"
---

# Query DSL

**Files:** `src/query/ast.rs`, `src/query/parser.rs`, `src/query/engine.rs`

## Why a DSL?

The basic commands (`ls`, `show`, `deps`, `refs`) answer point queries. The query DSL handles compound questions that would otherwise require multiple round-trips:

```bash
# Without DSL: three separate calls
smartgrep ls structs --in src/ir/
smartgrep show Symbol
smartgrep deps Symbol

# With DSL: one call
smartgrep query "structs where file contains 'ir/' | with fields, deps"
```

## Grammar

```
batch       = query (";" query)*
query       = source ("|" stage)*
source      = source_kind [in_clause] [implementing_clause] [where_clause]
            | "symbol" <name> [where_clause]
            | "deps" [<name>] [where_clause] | "refs" [<name>] [where_clause]
source_kind = "symbols" | "functions"                      (all languages)
            | "fns" | "structs" | "traits" | "impls" | "mods"   (Rust)
            | "classes"                                     (Java / TS / Python)
            | "records"                                     (Java)
            | "funcs"                                       (Go)
            | "function" | "namespaces"                     (TypeScript)
            | "defs"                                        (Python)
            | "methods" | "interfaces" | "enums" | "consts" | "types"   (shared)
in_clause   = "in" path_substring
implementing_clause = "implementing" <name>
where_clause = "where" and_group ("or" and_group)*
and_group   = condition ("and" condition)*
condition   = field op value
field       = "name" | "qualified_name" | "file" | "line" | "visibility"
            | "kind" | "parent" | "signature" | "return_type" | "attributes"
            | "from" | "to" | "dep_kind"
            | "field_count" | "param_count"
op          = "=" | "!=" | "contains" | ">" | "<" | ">=" | "<=" | "starts_with" | "ends_with"
            | "not" ("contains" | "starts_with" | "ends_with" | "=")
value       = quoted_string | bare_word | number
stage       = with_stage | show_stage | where_stage | sort_stage | limit_stage
with_stage  = "with" enrichment ("," enrichment)*
enrichment  = "fields" | "params" | "deps" | "refs" | "signature"
show_stage  = "show" column ("," column)*
sort_stage  = "sort" field ["asc" | "desc"]
limit_stage = "limit" number
```

A **batch** is multiple queries separated by `;` — they run against the same index and their results are concatenated.

Source terms are language-native: each maps to the kind string a parser emits (`funcs` → `func`, `defs` → `def`, `classes` → `class`), singular or plural. `functions` is the one umbrella term, expanding to every free-function kind (`fn`, `func`, `function`, `def`). `interfaces` (Java/Go/TS) and `traits` (Rust) are deliberately distinct. `implementing <name>` keeps symbols that have an `Implements` dep to `<name>`. Full table and rationale: [08 — Language-native vocabulary](08-language-native-vocabulary).

## AST types

The parser turns a query string into this AST:

```
Batch
  └── Query
        ├── Source
        │     ├── Symbols { kind_filter: Option<Vec<String>>, in_file, implementing, where_clause }
        │     ├── Symbol  { name, where_clause }
        │     ├── Deps    { name, where_clause }
        │     └── Refs    { name, where_clause }
        └── Vec<Stage>
              ├── With   { enrichments: [Fields, Params, Deps, Refs, Signature] }
              ├── Show   { columns: ["name", "file", "kind", ...] }
              ├── Where  { conditions: [[Condition, ...], ...] }  (CNF)
              ├── Sort   { field, descending }
              └── Limit  { count }
```

`where_clause` conditions are in conjunctive normal form: the outer `Vec` is OR-groups, each inner `Vec` is AND-conditions within a group. So `where a and b or c` becomes `[[a, b], [c]]`.

## Execution

The query engine evaluates a `Query` in two steps:

1. **Source** — pull the initial row set from the `Index` (e.g., all symbols whose kind is in `kind_filter`, narrowed by `in` path and `implementing`, or all deps for a given symbol), then apply the source's `where` clause.
2. **Pipeline** — pass the rows through each `Stage` left to right:
   - `With`: fetch and attach additional data (fields, deps, refs) to each row.
   - `Where`: filter rows that don't match the condition expression.
   - `Show`: project rows to a subset of columns.
   - `Sort`: stable sort on a field.
   - `Limit`: truncate to N rows.

Each stage is a pure transformation on a row set. No stage touches the index after the source step (except `With`, which may do additional lookups).

## Example

```bash
smartgrep query "functions where name starts_with 'parse' and file contains 'parser/' | with signature | sort name asc | limit 20"
```

Pipeline:
1. Source: all symbols with a kind in `FUNCTION_KINDS` (`fn`, `func`, `function`, `def`)
2. Where: `name starts_with 'parse'` AND `file contains 'parser/'`
3. With: attach `signature` field to each result row
4. Sort: by `name` ascending
5. Limit: first 20 rows

---

Previous: [04 — Index](04-index) | Next: [06 — Daemon](06-daemon)
