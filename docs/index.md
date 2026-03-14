---
layout: home
title: smartgrep
---

# smartgrep

Structural code navigation for agents.

smartgrep parses source code with tree-sitter to extract symbols (functions, structs, traits, impls, and more) and makes them queryable from the command line. Instead of reading entire files or guessing at grep patterns, an agent can ask "what functions are in this module?" or "what depends on this type?" and get a precise, low-token answer.

## Architecture slides

Each slide covers one layer of the system, building from the problem statement to the full implementation.

1. [Overview](architecture/01-overview) — The problem, the pipeline in one picture
2. [IR](architecture/02-ir) — The language-agnostic intermediate representation
3. [Parsers](architecture/03-parsers) — tree-sitter, one parser per language
4. [Index](architecture/04-index) — Lookup tables built from IR
5. [Query DSL](architecture/05-query-dsl) — Grammar, AST, execution
6. [Daemon](architecture/06-daemon) — Persistent process, socket protocol, auto-start
7. [Adding a language](architecture/07-adding-a-language) — What it actually takes

## Reference

- [Future index format](FUTURE_INDEX_FORMAT) — Plan for binary serialization when JSON becomes a bottleneck
