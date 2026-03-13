# smartgrep — Structural Code Navigation

An always-loaded skill. When a user asks about code structure, symbols, dependencies, or architecture, use `smartgrep` instead of multiple grep/read calls.

## When to use smartgrep

Use it when the user asks about:
- Code structure, architecture, or organization
- Finding classes, functions, interfaces, enums, records, structs, traits
- Dependencies between symbols or what references a symbol
- Exploring a new or unfamiliar codebase
- Finding implementations of an interface or trait
- Listing endpoints, services, controllers, consumers
- Any structural question that would otherwise need multiple grep/read calls

## Detecting availability

Before first use in a session, verify smartgrep is installed:
```bash
which smartgrep
```
If not on PATH, fall back to normal grep/read tools.

## Key principle

Prefer ONE compound `smartgrep query` over multiple grep+read calls. It is more token-efficient and returns structured results.

## Quick commands

```bash
smartgrep ls structs              # list all structs/classes
smartgrep ls functions            # list all functions
smartgrep show <name>             # detail for a symbol
smartgrep deps <name>             # what does X depend on?
smartgrep refs <name>             # what references X?
smartgrep context path/to/file    # structural summary of a file
```

## Query DSL

### Sources
- `structs`, `functions`, `methods`, `traits`, `enums`, `symbols` — list by kind
- `symbol <name>` — single symbol lookup
- `deps [name]` — dependencies (optionally for a symbol)
- `refs [name]` — references (optionally for a symbol)
- `symbols in 'path'` — symbols in a file or directory

### Filters
```
where <field> <op> <value> [and|or ...]
```
**Fields:** name, file, visibility, kind, parent, from, to, dep_kind, field_count, param_count, attributes
**Operators:** `=`, `!=`, `contains`, `>`, `<`, `>=`, `<=`, `starts_with`, `ends_with`

### Pipeline stages (after `|`)
- `with fields` / `with params` / `with deps` / `with refs` / `with signature`
- `show col1, col2, ...`
- `sort field asc|desc`
- `limit N`

### Batch
Semicolon-separated: `"structs | limit 5 ; functions | limit 5"`

## Example patterns

| User question | Query |
|---|---|
| What are the REST endpoints? | `smartgrep query "methods where attributes contains '@PostMapping' or attributes contains '@GetMapping' \| show name, file, signature"` |
| Show me the domain model | `smartgrep query "structs where file contains 'domain' or file contains 'model' \| with fields"` |
| What does X depend on? | `smartgrep query "symbol X \| with deps, refs"` |
| Explore this codebase | `smartgrep ls structs` then `smartgrep ls functions` |
| Find implementations of Y | `smartgrep refs Y` or `smartgrep query "structs where parent = 'Y' \| with fields"` |
| List services | `smartgrep query "structs where attributes contains '@Service' \| show name, file"` |
| Public API in a directory | `smartgrep query "functions where file contains 'src/api' and visibility = public \| show name, file, signature"` |

## Output guidance

Present results concisely. Summarize patterns ("5 controllers, all in src/api/"). Quote symbol names and file paths precisely. Use smartgrep output directly rather than re-reading files unless the user needs full implementation bodies.
