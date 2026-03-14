---
layout: page
title: "05 — Query DSL"
---

# Query DSL

**File:** `src/query/ast.rs`

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
source      = source_kind [argument] [where_clause]
source_kind = "symbols" | "structs" | "functions" | "methods" | "traits"
            | "enums" | "impls" | "consts" | "types" | "modules"
            | "symbol" <name> | "deps" [<name>] | "refs" [<name>]
where_clause = "where" and_group ("or" and_group)*
and_group   = condition ("and" condition)*
condition   = field op value
field       = "name" | "file" | "visibility" | "kind" | "parent"
            | "from" | "to" | "dep_kind"
            | "field_count" | "param_count"
op          = "=" | "!=" | "contains" | ">" | "<" | ">=" | "<=" | "starts_with" | "ends_with"
value       = quoted_string | bare_word | number
stage       = with_stage | show_stage | where_stage | sort_stage | limit_stage
with_stage  = "with" enrichment ("," enrichment)*
enrichment  = "fields" | "params" | "deps" | "refs" | "signature"
show_stage  = "show" column ("," column)*
sort_stage  = "sort" field ["asc" | "desc"]
limit_stage = "limit" number
```

A **batch** is multiple queries separated by `;` — they run against the same index and their results are concatenated.

## AST types

The parser turns a query string into this AST:

```
Batch
  └── Query
        ├── Source
        │     ├── Symbols { kind_filter, in_file, where_clause }
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

1. **Source** — pull the initial row set from the `Index` (e.g., all symbols of a given kind, or all deps for a given symbol).
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
1. Source: all `Function` symbols
2. Where: `name starts_with 'parse'` AND `file contains 'parser/'`
3. With: attach `signature` field to each result row
4. Sort: by `name` ascending
5. Limit: first 20 rows

---

Previous: [04 — Index](04-index) | Next: [06 — Daemon](06-daemon)
