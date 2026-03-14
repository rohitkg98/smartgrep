---
layout: page
title: "06 — Daemon"
---

# Daemon

**Files:** `src/daemon/server.rs`, `src/daemon/client.rs`, `src/daemon/protocol.rs`

## Why a daemon?

Every smartgrep invocation without a daemon pays the same startup cost: load the JSON index from disk, deserialize it, then answer the query. On a large codebase that can add tens or hundreds of milliseconds per call — noticeable when an agent is making dozens of queries in a session.

The daemon keeps the index in memory and answers queries over a Unix socket. Per-call overhead drops to a socket round-trip.

The daemon also runs a file watcher. When source files change it rebuilds the index in the background so the next query sees fresh data without any explicit `smartgrep index` call.

## Socket path

Each project gets its own daemon, identified by a socket path derived from the project root:

```
/tmp/smartgrep-<16-char SHA-256 of canonical project root>-v<version>.sock
```

The version slug is embedded in the path. When you upgrade smartgrep, the new binary computes a different socket path and auto-starts a fresh daemon. The old daemon idles out naturally — no explicit kill needed.

## Protocol

The protocol is newline-delimited JSON over the Unix socket. One request line, one response line.

**Request:**
```json
{"command": "ls", "args": "functions", "format": "text"}
```

**Response (success):**
```json
{"status": "ok", "output": "...rendered text..."}
```

**Response (error):**
```json
{"status": "error", "message": "unknown command"}
```

The `command` field maps directly to the CLI subcommand name. `args` is the unparsed argument string. `format` is `"text"` or `"json"`.

## Auto-start

Clients never need to start the daemon explicitly. `try_daemon` in `src/daemon/client.rs` handles it:

```
CLI invocation
    │
    ├── daemon socket exists? ──yes──► send request ──► return output
    │
    └── no ──► ensure_daemon():
                  spawn `smartgrep run-server --idle-timeout 1800`
                  wait up to 5s for socket to appear + ping to succeed
                  └── send request ──► return output
                  └── if timeout ──► fall back to direct execution (no daemon)
```

The fallback to direct execution means the daemon is an optional optimization. All commands work correctly without it.

## Idle timeout

The daemon shuts itself down after 1800 seconds (30 minutes) of no incoming requests. The server loop checks `last_activity` on each iteration and sets a `shutdown` flag when the idle threshold is exceeded.

## Server startup sequence

1. Clean up any stale socket file at the target path.
2. Write a PID file (same path as socket, `.pid` extension).
3. Build the initial index synchronously. Log symbol and dep counts.
4. Wrap the index in `Arc<Mutex<Index>>`.
5. Start a file watcher thread that holds a clone of the `Arc`. On file events, it rebuilds the index and swaps the `Mutex` contents.
6. Bind the Unix socket and enter the accept loop.
7. Each accepted connection is handled: read one JSON line, dispatch to command handler, write one JSON line, close.

---

Previous: [05 — Query DSL](05-query-dsl) | Next: [07 — Adding a Language](07-adding-a-language)
