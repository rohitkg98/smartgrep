---
layout: page
title: Future Index Format
---

# Future Index Format

**Status: plan, not yet implemented.**

## Current state

The index is serialized as JSON to `.smartgrep/index.json` (or the platform cache directory). JSON was chosen for debuggability: you can open the file with any text editor and inspect what was indexed. During early development this tradeoff is correct.

## Why we'll want a binary format eventually

JSON has two costs that grow with codebase size:

1. **File size.** Field names (`"qualified_name"`, `"visibility"`, etc.) are repeated for every symbol. A 5000-symbol index can easily be 5–10 MB of JSON. Binary formats like bincode or MessagePack represent the same data in 1–2 MB.

2. **Load time.** JSON deserialization is text parsing — tokenizing, allocating strings, building maps. Bincode deserialization is a near-zero-copy read from bytes into Rust structs. At scale the difference is 5–10x.

For most current use cases neither cost is noticeable. The daemon keeps the index in memory, so load time is paid once per session. But as codebases grow, a cold start (no daemon running) that takes 200–500ms will become friction.

## Proposed approach

Add a version header to the serialized index file, prepended to a bincode or MessagePack payload:

```
Byte layout:
  [0..4]   magic:   u32 = 0x534D4752  ("SMGR")
  [4..8]   version: u32 = INDEX_FORMAT_VERSION
  [8..]    payload: bincode-encoded Index
```

The magic number makes it easy to distinguish a smartgrep index file from arbitrary data. The version field allows the loader to detect format changes.

Define the constant in a new module `src/version.rs`:

```rust
/// Increment this whenever the on-disk index format changes in a
/// backward-incompatible way. The loader checks this on startup and
/// rebuilds if the stored version does not match.
pub const INDEX_FORMAT_VERSION: u32 = 1;
```

Import it wherever the index is read or written.

## Migration strategy

The migration is transparent to the user. On load:

1. Read the first 8 bytes.
2. Check the magic number. If it doesn't match `0x534D4752`, the file is JSON (or garbage) — delete it and rebuild.
3. Check the version field. If `stored_version != INDEX_FORMAT_VERSION`, delete and rebuild.
4. Deserialize the payload.

On write: always write the current header + bincode payload. Never write JSON once binary is enabled.

No user-facing command is needed. The first invocation after a format change rebuilds automatically, like a stale index rebuild today.

## When to do it

Only when index load time becomes measurable on target codebases. Suggested threshold:

- **500+ files** in the project, AND
- **>50ms** cold-start load time on the target machine

Below that threshold, JSON's debuggability advantage outweighs the binary format's performance advantage. Measure before migrating.

A simple way to check: add a `--debug-timing` flag (or look at existing timing logs) and record load time on a representative large project. If it's under 50ms, defer.

## Crate choice

- **bincode** (`serde` integration, fast, Rust-only): preferred if we never need to read the index from another language.
- **MessagePack** (`rmp-serde`): if we later want a Python or JavaScript client to read the index directly, MessagePack is more portable.

Start with bincode. Switch to MessagePack only if cross-language reads become a requirement.
