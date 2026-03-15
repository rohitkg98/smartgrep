# smartgrep — Structural code navigation for agents

## What this is
Language-aware CLI for coding agents. Parses source via tree-sitter to extract structural symbols (functions, structs, traits, impls, classes, methods) and presents them as greppable text output. Like an IDE's symbol browser, but for CLI agents — low-token structural queries instead of reading entire files.

## Tech
- Rust (cargo, edition 2021)
- tree-sitter for multi-language source parsing
- Rust 1.70+ required

## Commands
```bash
cargo test                                # run all tests
cargo run -- context src/main.rs          # structural summary of a file
cargo run -- ls functions                 # list all functions
cargo run -- ls structs                   # list all structs
cargo run -- show <name>                  # detail for a symbol
cargo run -- deps <name>                  # what does X depend on?
cargo run -- refs <name>                  # what references X?
cargo run -- index                        # force re-index (usually implicit)
```

## Architecture: 3 layers, 2 contracts

```
Parser (tree-sitter) → IR → Index Builder → Index → Command
```

- **IR** (`src/ir/types.rs`): Language-agnostic symbol/dependency maps. Parsers produce, builder consumes.
- **Index** (`src/index/types.rs`): Queryable structure with lookup tables. Builder produces, commands consume.

## Key design decisions
- IR exists so we can add languages by writing one parser, without changing the index builder or commands
- Parsers are tested against fixture files — small .rs/.java files in tests/fixtures/
- Index builder is tested with hand-built IR — no parsing needed
- Commands are tested with hand-built Index — no builder needed
- Auto-indexing: queries trigger indexing implicitly. Re-indexes when source files change.

## File layout
- `src/ir/` — IR types and validation
- `src/parser/` — tree-sitter parsers (rust.rs, later java.rs)
- `src/index/` — index types, builder, storage, auto-detection
- `src/commands/` — CLI commands (context, ls, show, deps, refs)
- `src/format/` — text table and JSON output
- `tests/fixtures/` — small source files for parser tests

## Code Navigation
Always use `smartgrep` for structural code exploration on this project. It is faster and more token-efficient than reading files or grepping.

### Language-native vocabulary
Symbols use language-native kind strings, not a shared enum:
- **Rust:** fn, method, struct, enum, trait, impl, const, type, mod
- **Java:** class, interface, enum, method, record
- **Go:** func, method, struct, interface, const, type

Dependency kinds: Call (was FunctionCall), TypeRef (was TypeReference), Implements (was TraitImpl)

### Prefer smartgrep query for compound questions
```bash
# Instead of multiple grep/read calls, compose one query:
smartgrep query "structs where file contains 'ir/' and visibility = public | with fields"
smartgrep query "functions where name = 'run' and file contains 'commands/' | show name, file, signature"
smartgrep query "symbol Index | with deps, refs"
smartgrep query "deps where from contains 'parser' | show from, to, dep_kind"

# Find types implementing a trait/interface:
smartgrep query "structs implementing Display"
```

### Use basic commands for simple lookups
```bash
smartgrep context src/main.rs       # structural overview of a file
smartgrep ls functions              # list all functions
smartgrep ls structs                # list all structs
smartgrep ls interfaces             # list all interfaces (Go) / traits (Rust)
smartgrep ls structs --in src/ir/   # list structs in specific path
smartgrep show <name>               # detail for a symbol
smartgrep deps <name>               # what does X depend on?
smartgrep refs <name>               # what references X?
```

### Large codebases (100+ files): always scope your queries
On large projects, bare `ls` commands dump thousands of symbols. Always filter:
```bash
# BAD: floods context with 500KB+ of output
smartgrep ls functions

# GOOD: scope with --in or use query with file filtering
smartgrep ls functions --in go/services/
smartgrep query "structs in 'go/services/' | with fields"
smartgrep query "interfaces where file contains 'common/' | with fields"
smartgrep query "functions where name starts_with 'New' and file contains 'services/'"
```

### Language notes
- **Go/Java interfaces** → use `interfaces` (kind="interface")
- **Rust traits** → use `traits` (kind="trait", Rust only)
- **`interfaces` and `traits` are distinct** — `interfaces` matches Java/Go interface, `traits` matches Rust trait
- **Go method receivers** → stored in `parent` field (e.g., `methods where parent = MultiGateway`)
- **Generated code** → filter out with `where file not contains '.pb.go'`
- **`implementing` clause** → `structs implementing Display` finds types that implement a trait/interface

### When to use smartgrep vs file reading
- **Use smartgrep**: finding symbols, understanding structure, exploring dependencies, listing functions/structs
- **Read files directly**: when you need the full implementation body, line-by-line logic, or exact syntax

## Agent workflow
- The main agent is the manager. It delegates all research and implementation to agent teams.
- Use agent teams for research (codebase exploration, understanding existing code, gathering context).
- Use agent teams for implementation (writing code, editing files, running tests).
- The main agent focuses on decision-making, coordination, and communicating with the user.
