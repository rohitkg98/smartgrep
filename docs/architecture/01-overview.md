---
layout: page
title: "01 — Overview"
---

# Overview

## The problem

Agents that navigate code typically do one of two things:

- **Read whole files** — accurate but expensive; a 500-line file costs 500 lines of context even if you only needed the function signatures.
- **Grep for patterns** — cheap but fragile; regex can't tell a function definition from a comment, and it can't answer "which classes implement this interface?".

smartgrep's answer: parse the source structure once, store it as a queryable index, and let agents ask structural questions instead of textual ones.

```
# Bad: read everything to find one thing
cat src/parser/rust.rs

# Bad: grep can't distinguish definitions from references
grep -r "fn parse" src/

# Good: ask the structure directly
smartgrep ls functions --in src/parser/
smartgrep show Index
smartgrep query "classes implementing Serializable"
smartgrep query "structs where file contains 'ir/' | with fields"
```

## The three-layer pipeline

```mermaid
flowchart LR
    src["Source files\n(.rs, .java, .go, .ts, .py)"]
    parser["Parser\ntree-sitter"]
    ir["IR\nSymbols + Dependencies"]
    builder["Index Builder\nlookup tables"]
    index["Index\n(queryable)"]
    cmd["Commands\ncontext / map / ls / show / deps / refs / query"]

    src --> parser --> ir --> builder --> index --> cmd
```

Each layer has a single contract with its neighbors:

| Layer | Input | Output |
|-------|-------|--------|
| Parser | Source file | `Ir` (symbols + deps) |
| Index Builder | `Ir` | `Index` (lookup tables) |
| Commands | `Index` | Text or JSON output |

The contracts are defined as Rust types. Parsers produce `Ir`. The builder consumes `Ir` and produces `Index`. Commands consume `Index`. None of the layers knows about the others' internals.

## Why this structure matters

Adding a language means writing one parser and registering it in `src/lang.rs`. The builder and every command work unchanged because they only see `Ir` and `Index`. This is the core architectural promise. Rust, Java, Go, TypeScript, and Python are supported today.

---

Next: [02 — IR](02-ir)
