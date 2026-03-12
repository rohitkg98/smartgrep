# smartgrep

Structural code navigation CLI built for coding agents. Parses source files via tree-sitter, extracts structural symbols (functions, structs, traits, impls, enums, methods, consts, modules), and presents them as compact, greppable text output.

Like an IDE's symbol browser, but for CLI agents.

## Philosophy

Agents waste tokens reading entire files when they only need to know what's defined where. Smartgrep gives low-token structural queries that return just the symbols, signatures, dependencies, and references an agent needs.

The query DSL lets agents compose one-shot questions instead of multiple round trips. One query can filter by kind, constrain by file or visibility, enrich with fields/params/deps, select columns, sort, and limit -- all in a single invocation. The grammar is designed to be fluent for LLMs to generate.

## Installation

```bash
cargo install --path .
```

Requires Rust 1.70+.

## Quick start

```bash
# Structural summary of a file
smartgrep context src/main.rs

# List all functions
smartgrep ls functions

# List all structs
smartgrep ls structs

# Detail for a specific symbol
smartgrep show Config

# What does Config depend on?
smartgrep deps Config

# What references Config?
smartgrep refs Config

# Force re-index (usually implicit -- queries trigger indexing automatically)
smartgrep index

# Run a composable query
smartgrep query "structs where visibility = public | with fields | limit 10"
```

Global flags:
- `--format text|json` -- output format (default: text)
- `--no-color` -- disable colored output
- `--project-root <path>` -- set the project root directory

## Query DSL

The `query` command accepts a composable DSL that pipes a source through transformation stages. This is the main feature.

### Grammar

```
batch       = query (";" query)*
query       = source ("|" stage)*
source      = source_kind [in_clause] [where_clause]
source_kind = "symbols" | "structs" | "functions" | "methods" | "traits"
            | "enums" | "impls" | "consts" | "types" | "modules"
            | "symbol" <name> | "deps" [<name>] | "refs" [<name>]
in_clause   = "in" '<file_path>'
where_clause = "where" condition ("and" condition)*
condition   = field op value
op          = "=" | "!=" | "contains" | ">" | "<" | ">=" | "<="
            | "starts_with" | "ends_with"
stage       = with | show | where | sort | limit
with        = "with" enrichment ("," enrichment)*
enrichment  = "fields" | "params" | "deps" | "refs" | "signature"
show        = "show" column ("," column)*
sort        = "sort" field ["asc" | "desc"]
limit       = "limit" <number>
```

### Sources

| Source | Description |
|---|---|
| `symbols` | All symbols in the index |
| `structs` | All structs |
| `functions` | All functions (also: `fn`) |
| `methods` | All methods |
| `traits` | All traits |
| `enums` | All enums |
| `impls` | All impl blocks |
| `consts` | All constants |
| `types` | All type aliases |
| `modules` | All modules (also: `mod`) |
| `symbol Foo` | Look up a specific symbol by name |
| `deps Foo` | Dependencies of symbol Foo |
| `deps` | All dependencies |
| `refs Foo` | References to symbol Foo |
| `refs` | All references |
| `symbols in 'src/ir/types.rs'` | All symbols in a specific file |
| `structs in 'src/index/'` | Structs in files matching a path substring |

### Where clauses

Filter results with `where`. Combine conditions with `and`.

**Fields for symbol rows:** `name`, `file`, `visibility`, `kind`, `parent`, `field_count`, `param_count`

**Fields for dependency rows:** `from`, `to`, `kind`, `dep_kind`, `file`, `line`

**Operators:**

| Operator | Aliases | Description |
|---|---|---|
| `=` | `==`, `is` | Equals (case-insensitive) |
| `!=` | `is_not` | Not equals |
| `contains` | `has`, `includes`, `~` | Substring match |
| `>` | | Greater than (numeric) |
| `<` | | Less than (numeric) |
| `>=` | | Greater than or equal |
| `<=` | | Less than or equal |
| `starts_with` | `startswith` | Prefix match |
| `ends_with` | `endswith` | Suffix match |

### Pipeline stages

Stages are separated by `|` and applied left to right.

**`with`** -- Enrich rows with additional data:
- `fields` -- struct/enum fields (adds `fields`, `field_count` columns)
- `params` -- function/method parameters (adds `params`, `param_count` columns)
- `deps` -- dependencies (adds `deps`, `dep_count` columns)
- `refs` -- references (adds `refs`, `ref_count` columns)
- `signature` -- full signature (also: `sig`)

**`show`** -- Select specific columns for output:
```
| show name, file, kind
```

**`where`** -- Post-filter after enrichment:
```
| where field_count > 5
```

**`sort`** -- Sort results:
```
| sort name asc
| sort field_count desc
```

**`limit`** -- Cap the number of results:
```
| limit 10
```

### Batch queries

Run multiple queries in one invocation by separating with `;`:

```bash
smartgrep query "structs; functions where file contains 'commands/'"
```

Each query's results are printed under a `# Query N` header.

### Example queries

```bash
# List all public structs
smartgrep query "structs where visibility = public"

# Find functions in a specific directory
smartgrep query "functions where file contains 'commands/'"

# Get a struct's fields
smartgrep query "symbol Config | with fields"

# Structs with more than 5 fields
smartgrep query "structs | with fields | where field_count > 5"

# All function signatures in a file
smartgrep query "functions in 'src/main.rs' | with signature | show name, signature"

# What does a specific symbol depend on?
smartgrep query "deps Config | show from, to, kind"

# Who references Index?
smartgrep query "refs Index | show from, to, kind"

# All public functions sorted by name
smartgrep query "functions where visibility = public | sort name asc"

# Top 5 structs by field count
smartgrep query "structs | with fields | sort field_count desc | limit 5"

# Methods on a specific type
smartgrep query "methods where parent = Config"

# Find symbols whose name starts with "parse"
smartgrep query "symbols where name starts_with parse"

# All traits with their dependencies
smartgrep query "traits | with deps"

# Functions that take parameters, show just the signatures
smartgrep query "functions | with params | where param_count > 0 | show name, signature"

# Batch: get structs and their fields + all enums in one shot
smartgrep query "structs | with fields; enums | with fields"
```

## Configuring your CLAUDE.md

Add this to your project's `CLAUDE.md` to instruct Claude Code to use smartgrep for structural code queries:

```markdown
## Code Navigation

Use `smartgrep` for structural code queries instead of grep/find when exploring code structure.

- `smartgrep query "<dsl>"` for composable one-shot structural questions
- `smartgrep context <file>` for a structural overview of a file
- `smartgrep query "symbol <Name> | with deps, refs"` to understand a symbol's role
- Prefer smartgrep over reading entire files when you only need structure, signatures, or dependency info
- Use batch queries (semicolon-separated) to answer multi-part questions in one call
```

## Smartgrep Skill (Coming Soon)

A Claude Code skill (`/smartgrep`) is under construction that will make smartgrep a first-class tool for agents. This will allow Claude Code to automatically invoke smartgrep queries as part of its exploration workflow without manual prompting.

## Supported languages

- **Rust** -- full support via tree-sitter-rust

More languages coming via tree-sitter grammars. The IR layer is language-agnostic -- adding a language means writing one parser, with no changes to the index builder or query engine.

## Architecture

```
Parser (tree-sitter) --> IR --> Index Builder --> Index --> Commands / Query Engine
```

Three layers, two contracts:

- **Parser** (`src/parser/`) -- Language-specific tree-sitter parsers produce the IR.
- **IR** (`src/ir/types.rs`) -- Language-agnostic symbol and dependency maps. The contract between parsers and the index builder.
- **Index** (`src/index/types.rs`) -- Queryable structure with lookup tables. The contract between the index builder and commands/query engine.

The query DSL (`src/query/`) parses query strings into an AST, then the engine executes them against the index.

Auto-indexing: queries trigger indexing implicitly. The index is rebuilt when source files change.
