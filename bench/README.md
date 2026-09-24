# smartgrep token benchmark

Does a coding agent use fewer tokens, without losing correctness, when smartgrep is set up in a repo? This harness runs a real agent (Claude Code, headless) on the same task in the same pinned open-source repo twice, and compares tokens, cost, tool usage and correctness.

| variant | environment |
|---|---|
| **A** | `smartgrep` on `PATH` and `smartgrep init` run in the checkout (CLAUDE.md block, project skill, index) |
| **B** | same checkout, no `smartgrep` anywhere on `PATH` |

Everything else is the same in both variants: model, prompt text, effort, `--max-turns`, tools (WebFetch/WebSearch disabled), a restored checkout, a fresh empty `HOME`, and `--setting-sources project`. Models are reached through OpenRouter.

This never runs in CI. You run it by hand on a VM or locally. It needs Python 3.11+ (standard library only), `git`, and `claude` on the invoking `PATH`.

## Task set (private)

Tasks and gold answers live in a private task set, not in this repo, so agents trained on public code can't have seen them. Point the harness at it with `--tasks-dir DIR` or `SMARTGREP_BENCH_TASKS=DIR`. The directory holds:

```
repos.toml          [[repo]] id, url, ref, sha (40 hex), language, setup = [cmds]   (setup optional)
tasks/*.toml        [[task]] id, repo, kind = "answer" | "change", prompt, ...
hidden/<name>/      hidden tests for change tasks, copied over the checkout after the agent finishes
```

An `answer` task has `gold`: each entry is a string or a list of accepted aliases. It can also set `answer_format` and `pass_f1` (default 0.9). A `change` task has `hidden_tests`, `test_cmd` and optionally `timeout_s` (the test command's timeout, default 900). `bench/tests/fixtures/taskset/` is a small public example of the format. The run records the task set's git sha and a hash of each task definition.

## Usage

```bash
export SMARTGREP_BENCH_TASKS=/path/to/private/taskset
export OPENROUTER_API_KEY=...            # or use a proxy that adds the key (see VM notes)
cargo build --release                    # the smartgrep under test

python3 bench/bench.py prepare                                   # fetch + setup every repo (idempotent)
python3 bench/bench.py check-model --model deepseek/deepseek-v4.1-flash \
    --smartgrep target/release/smartgrep                         # 1 task, A and B, before adding a model
python3 bench/bench.py run --models anthropic/claude-opus-5.5,deepseek/deepseek-v4.1-flash \
    --repeats 3 --budget-usd 25 --smartgrep target/release/smartgrep --out ~/bench-results/v0.5.1
python3 bench/bench.py report --out ~/bench-results/v0.5.1
```

Other `run` options: `--tasks ids`, `--repos ids`, `--variants A,B`, `--effort high`, `--max-turns 60`, `--timeout-s 1200` (wall clock per agent run), `--max-run-usd 2` (per-run cap passed to Claude Code as `--max-budget-usd`), `--seed 0`, `--max-attempts 3`, `--base-path`, `--no-openrouter-cost`, and `--dry-run`, which prints the plan and runs nothing.

- **Order:** repeat, then model, then task (shuffled with the recorded seed), then variant. A and B of the same (model, task, repeat) run back to back, and which one goes first alternates between repeats. Because repeats are the outer loop, a budget stop still leaves a balanced partial result.
- **Resume:** each finished run appends one line to `runs.jsonl`. Re-running the same command skips completed runs. Infrastructure failures are recorded as `status: "infra_error"` and retried on the next invocation, up to `--max-attempts`. These include API errors reported by Claude Code (429, 5xx, ...), a missing result event, and timeouts. Three infra errors in a row, or an auth error (401/403), stop the invocation. If the model gives a wrong or missing answer, the run is still `ok` and is graded as a fail.
- **Budget guard:** before each run the harness adds up `cost_usd` over every record in `--out`, including infra errors and superseded runs. If that total plus the estimated cost of the next run is over `--budget-usd`, it stops cleanly. The estimate is the mean cost of earlier runs of the same model, or `--max-run-usd` when there are none yet.
- **`--out`:** defaults to `<tasks-dir>/results/sg-<smartgrep sha>` (`check-model` uses `check-<model>`). Results contain prompts and answers, so the harness refuses an `--out` inside this repository. Keep `results/` out of git in the task-set repo too (`.gitignore`), except for the `public/` summaries you choose to publish.

## What a run does

1. Restore `<cache>/work/<repo>` from the pristine snapshot that `prepare` made. The cache defaults to `~/.cache/smartgrep-bench` (`SMARTGREP_BENCH_CACHE`). The checkout path is fixed rather than a new temp dir because setup steps such as `pip install -e .` write absolute paths into `.venv`. Runs are sequential, so one path per repo is enough.
2. Build a clean environment:
   - `HOME` is a new empty temp dir.
   - `PATH` is `--base-path` (default `/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin`, filtered to dirs that exist). The harness refuses to start if smartgrep is found in any of those dirs.
   - For A, a dir containing only a `smartgrep` symlink goes first on `PATH`, and `smartgrep init` runs in the checkout. For B, the harness checks that `shutil.which("smartgrep")` finds nothing.
3. Run `claude -p <prompt> --model M --effort E --output-format stream-json --verbose --setting-sources project --permission-mode bypassPermissions --disallowedTools WebFetch,WebSearch --max-turns N --max-budget-usd X --no-session-persistence` in the checkout. The whole process group is killed at `--timeout-s`. The agent environment adds:
   - `ANTHROPIC_BASE_URL` (`$BENCH_ANTHROPIC_BASE_URL`, default `https://openrouter.ai/api`)
   - `ANTHROPIC_AUTH_TOKEN=$OPENROUTER_API_KEY`
   - `ANTHROPIC_API_KEY=""`
   - `DISABLE_AUTOUPDATER=1`
   - `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`
   - `ANTHROPIC_DEFAULT_{HAIKU,SONNET,OPUS}_MODEL` and `CLAUDE_CODE_SUBAGENT_MODEL`, all set to the model under test, so background tasks and subagents use the same model.
4. Prompt = task prompt + standard instructions, with the same wording in A and B. An answer task ends with *"Do not modify files. When you are done, end your reply with exactly one line of the form `ANSWER: <JSON array of strings>` (items: …answer_format…)."* A change task ends with *"Make the change in the repository. Do not ask questions."*
5. Grade:
   - **answer:** take the last `ANSWER:` line, parse it as a JSON array, and normalize the items. An item matches a gold entry if it equals one of the entry's aliases exactly. Failing that, it matches if its last `.`/`::`/`/`/`#` segment equals an alias's last segment and that segment belongs to only one gold entry. Each entry can be matched once. Duplicate items collapse, and an extra alias of an entry that is already matched is ignored rather than counted as a false positive. The run passes when F1 ≥ `pass_f1`.
   - **change:** save the agent's diff first. It is taken against a snapshot made after `smartgrep init`, so the init files don't appear in it. Then copy `hidden/<name>/` over the checkout, run `test_cmd` (pass = exit 0), and keep the last 4 KB of output.
6. Cost, recorded as `cost_usd` with a `cost_source`:
   - `openrouter_generation`: the sum of `GET /v1/generation?id=` over every assistant message id. OpenRouter's `gen-…` id comes through as the message id. Lookups retry with backoff because the stats can lag.
   - `openrouter_key_delta`: when some ids are missing or subagents ran, the `GET /v1/key` `data.usage` delta around the run. This includes anything else that used the key during the run.
   - `openrouter_generation_partial`: some ids resolved and there is no key delta.
   - `claude_code_estimate`: Claude Code's own `total_cost_usd`, used when OpenRouter lookups are off or fail. It is always kept separately as `cc_cost_usd`.

   Lookups go to `$BENCH_OPENROUTER_API_BASE`, falling back to `$BENCH_ANTHROPIC_BASE_URL` and then `https://openrouter.ai/api`.
7. Delete the checkout and `HOME`.

Metrics per run:
- **Tokens:** input, output, cache write and cache read, summed over `modelUsage`. The headline figure is the cache-invariant total of all four.
- **Claude Code's run stats:** turns, duration, `is_error`/`terminal_reason`.
- **Tool calls:** counted by tool name. Bash commands are split on `|`, `&&`, `;` and newlines, and each command counts once per category it contains: smartgrep, grep (grep/rg/ag/ack/git grep), find (find/fd/ls/tree), read (cat/head/tail/sed -n/less/bat) or other. Filters after a pipe, like `| head`, don't count as reads.
- **Subagents:** calls to the Task/Agent tool.
- **Provenance:** smartgrep path, `--version` (the current CLI has no `--version` flag, so this is null), binary sha256, this repo's git sha, agent version, model, effort, repo sha, task-set sha, and task hash.

## Output layout

```
<out>/runs.jsonl                one JSON record per attempt (private: result text, answers)
<out>/transcripts/<key>.jsonl   raw stream-json
<out>/stderr/<key>.txt          agent stderr
<out>/diffs/<key>.diff          change tasks: the agent's diff
<out>/summary.json              private summary, including task_details (prompt, gold, answers)
<out>/report.md, index.html     private report
<out>/public/summary.json       publishable: ids, kinds, metrics only
<out>/public/index.html         publishable static page (self-contained, light/dark)
```

`run_key` = `<agent>__<effort>__<model>__<task>__<A|B>__r<repeat>`.

The public output is built from an explicit whitelist. It never contains prompts, gold answers, agent answers, test commands or hidden-test output. A unit test checks this.

## summary.json (schema_version 1)

```jsonc
{
  "schema_version": 1,
  "generated_at": "2026-09-24T12:00:00+00:00",
  "runs": {"ok": 132, "infra_error_records": 3},           // private copy also has "records"
  "meta": {"agents": [...], "agent_versions": [...], "efforts": [...], "smartgrep_versions": [...],
           "smartgrep_git_shas": [...], "cost_sources": [...], "repo_shas": {"<repo-id>": "..."}},
           // private copy also has smartgrep_binary_sha256, tasks_repo_shas
  "models": [{
    "model": "anthropic/claude-opus-5.5",
    "variants": {"A": {"n", "pass_rate", "mean_total_tokens", "mean_cost_usd", "total_cost_usd",
                       "tokens_per_pass", "smartgrep_use_share"}, "B": {...}},
    "token_ratio_a_over_b": 0.62,                 // ratio of mean total tokens (A / B)
    "cost_ratio_a_over_b": 0.70,
    "task_geomean_token_ratio_a_over_b": 0.58,    // geometric mean of per-task ratios (tasks weigh equally)
    "tasks_compared": 11
  }],
  "tasks": [{"task_id", "repo_id", "kind"}],       // private copy also has task_hash
  "cells": [{"model", "task_id", "repo_id", "kind", "variant", "n", "pass_rate",
             "total_tokens": {"mean", "min", "max"}, "input_tokens": {...}, "output_tokens": {...},
             "cache_read_tokens": {...}, "cache_creation_tokens": {...}, "cost_usd": {...},
             "turns": {...}, "duration_s": {...}, "tool_calls": {...}, "bash_smartgrep": {...},
             "bash_grep": {...}, "bash_find": {...}, "bash_read": {...}, "read_tool": {...},
             "subagent_calls": {...}, "used_smartgrep_share", "f1": {...}}]
             // private cells also have cost_sources
  // private only: "task_details": {task_id: {"prompt", "gold", "test_cmd", "runs": [...]}}
}
```

Aggregates exclude infra errors and use the latest `ok` record for each `run_key`.

## VM setup (exe.dev), untested

These steps haven't been tried end to end yet.

```bash
# on the VM: python3 (3.11+), git, claude, and the build toolchain for smartgrep
ssh exe.dev integrations add http-proxy --name openrouter --target https://openrouter.ai \
    --bearer <openrouter key> --attach vm:smartgrep-bench
export BENCH_ANTHROPIC_BASE_URL=http://openrouter.int.exe.xyz/api   # the proxy adds the key
# OPENROUTER_API_KEY can stay unset; the harness sends a placeholder token that the proxy replaces
```

The generation and key lookups go through the same proxy (`BENCH_OPENROUTER_API_BASE` defaults to `BENCH_ANTHROPIC_BASE_URL`). Whether the proxy forwards `/api/v1/generation` and `/api/v1/key` is untested.

## Tests

```bash
python3 -m unittest discover bench/tests      # offline, a few seconds
```

The end-to-end test runs `bench.py prepare/run/report` against a local git repo, using `tests/fixtures/bin/fake_claude.py` in place of `claude`. The fake checks the flags, the isolated HOME and PATH (smartgrep present in A, absent in B), then emits canned stream-json. `tests/fixtures/streams/real_*.jsonl` were captured from Claude Code 2.1.281 talking to a local fake API server, so no model was called.
