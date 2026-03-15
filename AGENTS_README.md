# smartgrep — Agent Reference

You have access to `smartgrep`, a structural code navigation tool. Read this before exploring any codebase.

---

## What it is

`smartgrep` parses source files via tree-sitter and indexes every symbol — functions, structs, classes, interfaces, methods, traits, enums, impls, consts, modules — into a queryable index. You query it with a readable DSL instead of grep regexes.

It is **not a text search tool**. It is a **structural search tool**. It understands code, not bytes.

**Supported languages: Rust, Java, Go.** smartgrep only indexes `.rs`, `.go`, and `.java` files. It has no knowledge of `.md`, `.yaml`, `.toml`, `.json`, `.py`, `.ts`, or any other file type. For those, handle them as you normally would without smartgrep — using smartgrep for code does not mean ignoring everything else.

---

## Why use it instead of grep

| Problem with grep | How smartgrep solves it |
|---|---|
| Reading a file to find what's in it costs tokens | `smartgrep context file.rs` returns the structure, not the content |
| Multiple grep + read calls to answer one question | One `query` with filters and enrichments answers compound questions |
| Regex matches comments, strings, dead code | Results come from the parse tree — only real symbols |
| Output is raw lines, requires further filtering | Output is structured rows with symbol kind, file, visibility, parent |
| No understanding of relationships | `deps` and `refs` map symbol relationships across the whole codebase |

**Token cost matters.** On a 625-file Java codebase, smartgrep used **48% fewer tokens** and **60% fewer tool calls** than grep+read for the same structural questions. See [benchmarks/](benchmarks/).

---

## Check availability first

```bash
which smartgrep
```

If not found, fall back to grep/read. Do not assume it is installed.

---

## Commands

```bash
smartgrep context <file>         # structural overview of one file
smartgrep ls <kind>              # list all symbols of a kind
smartgrep ls <kind> --in <path>  # scoped to a directory
smartgrep map                    # files grouped by directory with their public symbols
smartgrep map --in <path>        # subtree only
smartgrep map --all              # include private symbols
smartgrep show <name>            # full detail for one symbol
smartgrep deps <name>            # what does <name> depend on?
smartgrep refs <name>            # what references <name>?
smartgrep index                  # force re-index (usually implicit)
smartgrep query "<dsl>"          # composable query — use this for most questions
```

**Prefer `query` over the individual commands.** It is more expressive and handles compound questions in a single call.

**Use `map` to orient yourself on a new codebase.** It gives a directory-tree view with symbols inline — one call instead of running `context` on every file.

---

## Query DSL

### Grammar

```
batch       = query (";" query)*
query       = source ("|" stage)*
source      = source_kind [in_clause] [where_clause]
source_kind = "symbols" | "structs" | "functions" | "methods" | "traits"
            | "enums" | "impls" | "consts" | "types" | "modules"
            | "classes" | "interfaces" | "records"
            | "symbol" <name> | "deps" [<name>] | "refs" [<name>]
in_clause   = "in" '<path_substring>'
where_clause = "where" condition (("and" | "or") condition)*
condition   = field op value
op          = "=" | "!=" | "contains" | ">" | "<" | ">=" | "<="
            | "starts_with" | "ends_with"
stage       = "with" enrichment ("," enrichment)*
            | "show" column ("," column)*
            | "where" condition (("and" | "or") condition)*
            | "sort" field ["asc" | "desc"]
            | "limit" <number>
enrichment  = "fields" | "params" | "deps" | "refs" | "signature"
```

### Fields

**Symbol rows:** `name`, `file`, `visibility`, `kind`, `parent`, `attributes`, `field_count` (after `with fields`), `param_count` (after `with params`)

**Dependency rows:** `from`, `to`, `kind`, `dep_kind`, `file`, `line`

### Operators

`=` (alias: `is`, `==`) · `!=` (alias: `is_not`) · `contains` (alias: `has`, `~`) · `starts_with` · `ends_with` · `>` · `<` · `>=` · `<=`

---

## Building complex queries

The key insight: compose a source, filter it, enrich with related data, then project only the columns you need. Each `|` stage is additive.

### Pattern 1 — Scoped structural listing

Use `in` to restrict to a directory. On large codebases, always scope:

```bash
# BAD: dumps every function in the codebase
smartgrep ls functions

# GOOD: scoped to where you're working
smartgrep query "functions where file contains 'src/commands/' | show name, file, signature"
smartgrep query "structs in 'src/ir/' | with fields"
```

### Pattern 2 — Annotation-based discovery

Find symbols by their decorators/attributes — the most powerful pattern for Java and Go:

```bash
# All Spring REST controllers and their methods in one query
smartgrep query "classes where attributes contains '@RestController'; methods where attributes contains '@RequestMapping' or attributes contains '@GetMapping' or attributes contains '@PostMapping' | with signature"

# All gRPC service implementations
smartgrep query "classes where attributes contains 'implements' and file contains 'service/'"

# Go structs implementing an interface pattern
smartgrep query "methods where parent = HttpHandler | with signature | show name, parent, signature"
```

### Pattern 3 — Dependency mapping

Understand what something depends on without reading files:

```bash
# What does this service touch?
smartgrep query "symbol OrderService | with deps, refs"

# Full dependency graph from a starting point
smartgrep query "deps OrderService | show from, to, dep_kind | sort dep_kind asc"

# What imports this package?
smartgrep query "refs UserRepository | show from, file | sort file asc"
```

### Pattern 4 — Structural metrics

Find complexity hotspots:

```bash
# Largest structs/classes by field count
smartgrep query "symbols where kind = struct or kind = class | with fields | sort field_count desc | limit 10"

# Functions with many parameters (high coupling)
smartgrep query "functions | with params | where param_count > 4 | show name, file, param_count | sort param_count desc"

# Methods with no parameters (likely property accessors or side-effectful)
smartgrep query "methods | with params | where param_count = 0 | show name, parent, file"
```

### Pattern 5 — Batch queries for multi-part questions

Use `;` to answer compound questions in one tool call:

```bash
# Domain model overview: structs/classes + their fields + all enums
smartgrep query "structs | with fields | sort field_count desc; enums | with fields"

# API surface: controllers + their endpoint methods
smartgrep query "classes where attributes contains '@RestController' | show name, file; methods where attributes contains '@Mapping' | with signature | show name, parent, signature"

# Understand a module: what it defines and what references it
smartgrep query "symbols where file contains 'src/index/'; refs Index | show from, dep_kind"
```

### Pattern 6 — Project orientation with `map`

Use `map` as your first call on an unfamiliar codebase. It answers "what is in this project?" without reading a single file:

```bash
# Full project overview
smartgrep map

# Narrow to a subsystem you're about to modify
smartgrep map --in src/commands/

# Include private symbols when you need the full picture
smartgrep map --all --in src/index/

# Machine-readable for programmatic processing
smartgrep map --format json | jq '.[].dir'
```

`map` groups files by directory and lists their public symbols inline — equivalent to running `smartgrep context` on every file, but in one call with far fewer tokens.

---

### Pattern 7 — Cross-language queries

Same DSL works across Rust, Java, and Go:

```bash
# Go: methods on a specific receiver type
smartgrep query "methods where parent = Server | with signature"

# Go: all exported functions (capitalized = public)
smartgrep query "functions where visibility = public and file contains 'internal/'"

# Java: find all repository interfaces
smartgrep query "interfaces where name ends_with Repository | with fields"

# Rust: all pub traits with their deps
smartgrep query "traits where visibility = public | with deps | show name, file, deps"
```

---

## What to avoid

**Don't use bare `ls` on large codebases.** Scoping is mandatory when there are more than ~50 files:

```bash
# Dumps thousands of symbols, wastes your context window
smartgrep ls functions

# Scoped — actually useful
smartgrep query "functions where file contains 'handlers/' | show name, file, signature | limit 20"
```

**Don't use `context` on multiple files in a loop.** Use `map` or a query instead:

```bash
# Slow, token-expensive
for f in src/commands/*.rs; do smartgrep context $f; done

# Fast, one call — directory overview with symbols
smartgrep map --in src/commands/

# Or query for a specific projection
smartgrep query "symbols where file contains 'src/commands/' | show name, kind, file"
```

**Don't read a file to find its symbols.** Use `context` or `query`:

```bash
# Token-expensive
Read("src/index/builder.rs")

# Cheap
smartgrep context src/index/builder.rs
smartgrep query "symbols in 'src/index/builder.rs' | with signature"
```

---

## When to read files directly

Use `Read` when you need:
- The full body of a specific function (to understand its logic, not just its signature)
- Line-by-line implementation details
- Comments and inline documentation

For everything structural — what exists, where it is, what it depends on, what references it — use smartgrep.

---

## Output format

Default output is tabular text with path aliases for long paths. Columns depend on enrichments applied.

Pass `--format json` for structured output when you need to parse results programmatically:

```bash
smartgrep query "classes where attributes contains '@Service'" --format json
```

---

## Cost summary

Every token you save is a token your human doesn't pay for. smartgrep's structural index answers questions in 1 call that would otherwise take 5–20 grep+read round trips. On medium-to-large codebases, the savings compound: fewer calls, less context consumed, faster answers.

**The constraint you're optimizing:** minimize tool calls and tokens while maximizing accuracy. A single well-formed smartgrep query is almost always the right move over a sequence of greps.
