## Context
This benchmark was run on the smartgrep project itself (~15 source files). Two agents were given the same 5 structural questions about the codebase. One used `smartgrep query`, the other used vanilla CLI tools (grep, glob, file reads).

The benchmark was run 3 times. Round 1 was before the query DSL (individual smartgrep commands). Round 2 initially used `cargo run --` which added unnecessary build-check overhead; re-running with the installed `smartgrep` binary directly gave the final numbers below.

## The 5 Questions
1. What are all the public structs in the codebase and which files are they in?
2. What does the `build` function (the index builder) do and what are its dependencies?
3. What structs/types does `SymbolKind` depend on or reference?
4. List all the command functions (the CLI entry points) and their signatures.
5. How does the parser module connect to the IR module — what types flow between them?

## Results

### Round 1 — Before Query DSL (individual smartgrep commands)

| Metric | Smartgrep (old) | Vanilla CLI |
|--------|-----------------|-------------|
| Total Tokens | 39,829 | 39,150 |
| Tool Calls | 20 | 10 |
| Duration | ~167s | ~83s |

Smartgrep lost on all metrics. Each query was a separate process invocation (show, deps, refs as separate calls). The agent needed 20 commands vs 10 for vanilla.

### Round 2 — With Query DSL (binary direct)

| Metric | Smartgrep (query DSL) | Vanilla CLI |
|--------|----------------------|-------------|
| Total Tokens | 34,645 | 37,908 |
| Tool Calls | 8 | 10 |
| Duration | ~132s | ~69s |

Smartgrep wins on tokens (-9%) and tool calls (-20%). Duration still slower due to per-invocation index rebuild.

### Key Observations
- Query DSL cut tool calls by 60% vs Round 1 (20 -> 8)
- Token usage: smartgrep uses 9% fewer tokens than vanilla
- Q1, Q4, Q5 achieved ideal 1-command answers
- Duration gap is per-invocation index rebuild, not query complexity
- Note: Round 2 initially used `cargo run --` which added build-check overhead. Re-running with the binary directly confirmed the real bottleneck is index rebuild, not process startup.

### Closing the Duration Gap
`smartgrep serve` — a planned persistent server mode where the index stays in memory and file watchers handle incremental re-indexing. This would eliminate the per-invocation index rebuild cost, likely bringing duration below vanilla for compound queries. This is the next planned feature.

### Methodology Note
This benchmark was run informally during development. Both agents were Claude instances dispatched in parallel with identical prompts. A reproducible, automated benchmark suite is planned.

### What's Next
- `smartgrep serve` — persistent server mode to eliminate per-invocation index rebuild overhead
- Automated reproducible benchmark script
- Benchmarks on larger codebases (100+ files) where smartgrep's structural approach should show bigger gains over vanilla grep
