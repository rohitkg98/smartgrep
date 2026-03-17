# smartgrep — Structural Code Navigation

An always-loaded skill. When a user asks about code structure, symbols, dependencies, or architecture, use `smartgrep` instead of multiple grep/read calls.

Full agent reference: `AGENTS_README.md` in this repo (if available) or see inline guide below.

## Supported languages

smartgrep parses **Rust, Java, Go, and TypeScript** only. It has no knowledge of other file types.

**Use smartgrep for:** `.rs`, `.go`, `.java`, `.ts`, `.tsx` files — structural questions about code.

**For everything else** (`.md`, `.yaml`, `.toml`, `.json`, `.py`, `.js`, Dockerfiles, config files, documentation) — handle as you normally would without smartgrep. It has no knowledge of these file types.

Using smartgrep for code does not mean ignoring non-code files. Documentation and config often contain context the code index cannot surface.

## When to use smartgrep

Use it when the user asks about:
- Code structure, architecture, or organization in Rust, Java, Go, or TypeScript files
- Finding classes, functions, interfaces, enums, records, structs, traits, namespaces
- Dependencies between symbols or what references a symbol
- Exploring an unfamiliar Rust/Java/Go/TypeScript codebase
- Finding implementations of an interface or trait
- Listing endpoints, services, controllers, consumers
- Any structural question that would otherwise need multiple grep/read calls across `.rs`, `.go`, `.java`, `.ts`, or `.tsx` files

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
smartgrep map                     # project layout: dir summary with symbol counts + dep arrows
smartgrep map --in src/foo/       # subtree only
smartgrep map --depth 1           # top-level directories only
smartgrep map --symbols           # expand to per-file symbol lists
smartgrep map --include-generated # show auto-generated files (excluded by default)
smartgrep ls structs              # list all structs/classes
smartgrep ls functions            # list all functions
smartgrep show <name>             # detail for a symbol
smartgrep deps <name>             # what does X depend on?
smartgrep refs <name>             # what references X?
smartgrep context path/to/file    # structural summary of a file
```

## How to explore an unfamiliar codebase

Think like an IDE: start wide, zoom in, then query specifics. Never dump everything at once.

### Step 1 — Gauge project size
```bash
smartgrep map --depth 1
```
Read the header line: `N files · M symbols`. This tells you whether to use broad or targeted queries next.

- **Small (<30 files):** `smartgrep map` gives the full picture in one call.
- **Medium (30–100 files):** `smartgrep map` then drill into interesting dirs with `--in`.
- **Large (100+ files):** Start with `--depth 1` or `--depth 2` to orient, then use targeted `query` commands. **Do not run bare `smartgrep map` or `smartgrep ls` — the output will be too large to be useful.**

### Step 2 — Read the dep arrows
The `→` column in `map` output shows which directories each directory imports from. Use this to understand the layer structure before reading any code:
```
src/commands/   → src/daemon, src/format, src/index, src/ir
src/index/      → src/ir
src/ir/         (no outgoing — this is a leaf/core layer)
```
This tells you the dependency order without opening a single file.

### Step 3 — Zoom into interesting areas
```bash
smartgrep map --in src/commands/          # structure of one subsystem
smartgrep map --symbols --in src/index/   # file-level symbol listing for a dir
smartgrep query "structs in 'src/index/' | with fields"  # data shapes
```

### Step 4 — Query specifics
```bash
smartgrep show <SymbolName>               # full detail for a symbol
smartgrep query "symbol Foo | with deps, refs"  # relationships
smartgrep query "functions where file contains 'src/api' | show name, signature"
```

## Language-native vocabulary

Symbols use language-native kind strings (not a shared enum):

| Language | Kinds |
|---|---|
| **Rust** | fn, method, struct, enum, trait, impl, const, type, mod |
| **Java** | class, interface, enum, method, record |
| **Go** | func, method, struct, interface, const, type |
| **TypeScript** | function, class, interface, enum, type, method, const, namespace |

**Dependency kinds:** Call, TypeRef, Implements

**`interfaces` vs `traits`:** `interfaces` = Java/Go/TS interface; `traits` = Rust trait. They are distinct.

**Cross-language queries:**
- `functions` → finds Rust fn, Go func, TS function (all function-like symbols)
- `fns` → Rust only, `funcs` → Go only, `function` → TS only

## Query DSL

### Sources
- `structs`, `functions`, `methods`, `traits`, `enums`, `symbols`, `namespaces` — list by kind
- `interfaces` — Java/Go/TS interfaces (not Rust traits)
- `traits` — Rust traits only
- `symbol <name>` — single symbol lookup
- `deps [name]` — dependencies (optionally for a symbol)
- `refs [name]` — references (optionally for a symbol)
- `symbols in 'path'` — symbols in a file or directory

### `implementing` clause
Find types that implement a trait or interface:
```bash
smartgrep query "structs implementing Display"
smartgrep query "classes implementing Serializable | with fields"
smartgrep query "structs implementing Handler"  # Go: structural typing, matched by method set
```

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
| What's in this project? | `smartgrep map --depth 1` first, then zoom |
| What's the architecture? | `smartgrep map` — read the `→` dep arrows per directory |
| What's in this subsystem? | `smartgrep map --in src/foo/` |
| What are the REST endpoints? | `smartgrep query "methods where attributes contains '@PostMapping' or attributes contains '@GetMapping' \| show name, file, signature"` |
| Show me the domain model | `smartgrep query "structs where file contains 'domain' or file contains 'model' \| with fields"` |
| What does X depend on? | `smartgrep query "symbol X \| with deps, refs"` |
| Find implementations of Y | `smartgrep query "structs implementing Y \| with fields"` |
| List services | `smartgrep query "structs where attributes contains '@Service' \| show name, file"` |
| Public API in a directory | `smartgrep query "functions where file contains 'src/api' and visibility = public \| show name, file, signature"` |

### TypeScript examples
```bash
# TypeScript: find decorated classes
smartgrep query "classes where attributes contains '@Injectable' | with fields"

# TypeScript: interfaces in a specific directory
smartgrep query "interfaces where file contains 'src/types/' | with fields"

# TypeScript: all exported functions (cross-language)
smartgrep query "functions where visibility = public and file contains 'src/'"
```

## Large-repo warnings

**Never run bare `smartgrep map` or `smartgrep ls functions` on a repo with 100+ files.** The output can be hundreds of lines and burns context budget without adding value. Always scope first:

```bash
# BAD on large repos
smartgrep map
smartgrep ls functions

# GOOD: gauge size first, then zoom
smartgrep map --depth 1
smartgrep map --in src/services/
smartgrep query "functions where file contains 'src/api/' | show name, file | limit 20"
```

**Generated files are excluded by default.** Bindgen output, protobuf stubs, and vendor code are filtered out automatically. Use `--include-generated` only when you specifically need to inspect generated code.

**node_modules** is automatically skipped during indexing.

## Output guidance

Present results concisely. Summarize patterns ("5 controllers, all in src/api/"). Quote symbol names and file paths precisely. Use smartgrep output directly rather than re-reading files unless the user needs full implementation bodies.
