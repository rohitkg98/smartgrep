# smartgrep implementation plan

## CLI API
```
smartgrep context <file>     # structural summary of a file
smartgrep ls [type]          # list symbols: functions, structs, traits, impls, enums
smartgrep show <name>        # detail for one named thing
smartgrep deps <name>        # what does X depend on?
smartgrep refs <name>        # what references X?
smartgrep index              # force re-index (usually implicit)
```
All commands: `--format text|json`, `--no-color`, `--project-root <dir>`. Text output is tab-aligned, greppable. Every entry includes `file:line`.

## Architecture
```
Parser (tree-sitter) → IR → Index Builder → Index → Command
```
Parsers produce language-agnostic IR, commands consume the Index. Adding a language = writing one parser.

## 3 implementation phases

### Phase 1: Scaffold + IR + Parser
Create project structure, IR types, Rust tree-sitter parser, `context` command.

### Phase 2: Index + Auto-indexing + `ls`
Build index layer with lookups, auto-detect project, staleness checking, `ls` command.

### Phase 3: `show`, `deps`, `refs`
Complete the API with detail and graph traversal commands.

## Module structure
```
src/
  main.rs                 # CLI entry, command dispatch
  cli.rs                  # clap derive structs
  ir/
    mod.rs
    types.rs              # Symbol, Dependency, SourceLoc, Ir
  parser/
    mod.rs
    rust.rs               # tree-sitter-rust parser
  index/
    mod.rs
    types.rs              # Index with lookup HashMaps
    builder.rs            # IR → Index
    store.rs              # bincode serialization
    auto.rs               # staleness detection, auto-rebuild
  commands/
    mod.rs
    context.rs
    ls.rs
    show.rs
    deps.rs
    refs.rs
    index_cmd.rs
  format/
    mod.rs
    text.rs               # aligned table rendering
    json.rs
tests/
  fixtures/               # small .rs files for parser tests
```
