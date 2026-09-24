#!/usr/bin/env python3
"""smartgrep token-savings benchmark harness.

  bench.py prepare      fetch pinned repos into the cache and run their setup
  bench.py run          run the model x task x variant x repeat matrix (resumable)
  bench.py report       aggregate runs.jsonl into summary.json / report.md / index.html
  bench.py check-model  1-task, both-variant smoke test for a new model

See bench/README.md.
"""
from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from adapters.claude_code import ClaudeCodeAgent  # noqa: E402
from lib import config as cfgmod  # noqa: E402
from lib import prepare as prep  # noqa: E402
from lib.matrix import slug  # noqa: E402
from lib.openrouter import OpenRouterCost  # noqa: E402
from lib.report import write_report  # noqa: E402
from lib.runner import DEFAULT_BASE_PATH, REPO_ROOT, RunConfig, RunError, Runner  # noqa: E402

AGENTS = {"claude_code": ClaudeCodeAgent}


def _csv(s: str | None) -> list[str] | None:
    return [x.strip() for x in s.split(",") if x.strip()] if s else None


def _out_dir(args, taskset_root: Path | None, default_name: str) -> Path:
    if args.out:
        out = Path(args.out).expanduser().resolve()
    elif taskset_root is not None:
        out = taskset_root / "results" / default_name
    else:
        raise cfgmod.ConfigError("--out is required")
    if out.is_relative_to(REPO_ROOT) and not getattr(args, "allow_out_in_repo", False):
        raise cfgmod.ConfigError(
            f"--out {out} is inside the smartgrep repository; results contain private task data. "
            "Choose a directory outside it (or pass --allow-out-in-repo).")
    return out


def cmd_prepare(args) -> int:
    ts = cfgmod.load_taskset(cfgmod.resolve_tasks_dir(args.tasks_dir))
    cache = cfgmod.resolve_cache_dir(args.cache)
    ids = _csv(args.repos) or list(ts.repos)
    for rid in ids:
        if rid not in ts.repos:
            raise cfgmod.ConfigError(f"unknown repo id {rid!r}")
    for rid in ids:
        prep.prepare_repo(cache, ts.repos[rid], force=args.force)
    return 0


def _make_agent(args):
    if args.agent not in AGENTS:
        raise cfgmod.ConfigError(f"unknown agent {args.agent!r} (known: {', '.join(AGENTS)})")
    binary = shutil.which(args.agent_bin) or (args.agent_bin if os.path.isfile(args.agent_bin) else None)
    if binary is None:
        if args.dry_run:
            binary = args.agent_bin
        else:
            raise cfgmod.ConfigError(f"agent binary {args.agent_bin!r} not found")
    return AGENTS[args.agent](binary=str(Path(binary).resolve()) if os.path.isfile(binary) else binary)


def _cost_client(args) -> OpenRouterCost:
    if args.no_openrouter_cost:
        return OpenRouterCost.disabled()
    if not (os.environ.get("OPENROUTER_API_KEY") or os.environ.get("BENCH_OPENROUTER_API_BASE")
            or os.environ.get("BENCH_ANTHROPIC_BASE_URL")):
        print("[run] warning: no OPENROUTER_API_KEY / BENCH_*_BASE set; cost falls back to "
              "Claude Code's estimate", file=sys.stderr)
        return OpenRouterCost.disabled()
    return OpenRouterCost()


def _run(args, models, task_ids, repo_ids, variants, repeats, default_out) -> int:
    root = cfgmod.resolve_tasks_dir(args.tasks_dir)
    ts = cfgmod.load_taskset(root)
    tasks = cfgmod.select_tasks(ts, task_ids, repo_ids)
    out = _out_dir(args, root, default_out)
    smartgrep = None
    if args.smartgrep:
        smartgrep = Path(args.smartgrep).expanduser().resolve()
        if not (smartgrep.is_file() and os.access(smartgrep, os.X_OK)):
            raise cfgmod.ConfigError(f"--smartgrep {smartgrep} is not an executable file")
    elif "A" in variants and not args.dry_run:
        raise cfgmod.ConfigError("variant A needs --smartgrep /path/to/smartgrep")
    rc = RunConfig(
        taskset=ts, tasks=tasks, models=models, variants=variants, repeats=repeats, out=out,
        cache=cfgmod.resolve_cache_dir(args.cache), smartgrep=smartgrep,
        budget_usd=args.budget_usd, max_run_usd=args.max_run_usd, effort=args.effort,
        max_turns=args.max_turns, timeout_s=args.timeout_s, seed=args.seed,
        max_attempts=args.max_attempts, base_path=args.base_path, dry_run=args.dry_run)
    print(f"[run] results: {out}")
    summary = Runner(rc, _make_agent(args), _cost_client(args)).run()
    if summary.get("stopped") and summary["stopped"] != "dry-run":
        print(f"[run] stopped early: {summary['stopped']}")
    return 0


def cmd_run(args) -> int:
    variants = _csv(args.variants) or ["A", "B"]
    sha = cfgmod.git_head(REPO_ROOT)
    return _run(args, _csv(args.models), _csv(args.tasks), _csv(args.repos), variants,
                args.repeats, f"sg-{sha[:10] if sha else 'dev'}")


def cmd_check_model(args) -> int:
    root = cfgmod.resolve_tasks_dir(args.tasks_dir)
    ts = cfgmod.load_taskset(root)
    if args.task:
        task_id = args.task
    else:
        answer = [t for t in ts.tasks if t.kind == "answer"]
        if not answer:
            raise cfgmod.ConfigError("no answer task in the task set; pass --task")
        task_id = answer[0].id
    return _run(args, [args.model], [task_id], None, ["A", "B"], 1, f"check-{slug(args.model)}")


def cmd_report(args) -> int:
    taskset = None
    root = None
    try:
        root = cfgmod.resolve_tasks_dir(args.tasks_dir)
        taskset = cfgmod.load_taskset(root)
    except cfgmod.ConfigError as e:
        if args.tasks_dir:
            raise
        print(f"[report] note: {e}; private report will not include prompts/gold", file=sys.stderr)
    out = _out_dir(args, root, "")
    summary = write_report(out, taskset)
    print(f"[report] {summary['runs']['ok']} runs -> {out / 'report.md'}, {out / 'index.html'}, "
          f"{out / 'summary.json'}; publishable: {out / 'public'}/")
    return 0


def _add_common(p):
    p.add_argument("--tasks-dir", help=f"task set directory (default: ${cfgmod.TASKS_ENV})")
    p.add_argument("--cache", help=f"repo cache (default: ${cfgmod.CACHE_ENV} or {cfgmod.DEFAULT_CACHE})")


def _add_run_opts(p, budget_default: float):
    p.add_argument("--smartgrep", help="path to the smartgrep binary under test (variant A)")
    p.add_argument("--out", help="results dir (default: <tasks-dir>/results/<name>)")
    p.add_argument("--allow-out-in-repo", action="store_true", help=argparse.SUPPRESS)
    p.add_argument("--budget-usd", type=float, default=budget_default, help="hard cap for everything recorded in --out")
    p.add_argument("--max-run-usd", type=float, default=2.0,
                   help="per-run cap passed to the agent; also the cost estimate before any run of a model")
    p.add_argument("--agent", default="claude_code", help="agent adapter (default: claude_code)")
    p.add_argument("--agent-bin", default="claude", help="agent executable (default: claude)")
    p.add_argument("--effort", default="high")
    p.add_argument("--max-turns", type=int, default=60)
    p.add_argument("--timeout-s", type=float, default=1200, help="wall-clock limit per agent run")
    p.add_argument("--seed", type=int, default=0, help="seed for the task order")
    p.add_argument("--max-attempts", type=int, default=3, help="give up on a run after N infra errors")
    p.add_argument("--base-path", default=DEFAULT_BASE_PATH,
                   help="PATH the agent sees (smartgrep must not be in it)")
    p.add_argument("--no-openrouter-cost", action="store_true", help="skip OpenRouter cost lookups")
    p.add_argument("--dry-run", action="store_true", help="print the plan, run nothing")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="bench.py", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("prepare", help="fetch pinned repos and run setup")
    _add_common(p)
    p.add_argument("--repos", help="comma-separated repo ids (default: all)")
    p.add_argument("--force", action="store_true", help="re-fetch even if prepared")
    p.set_defaults(fn=cmd_prepare)

    p = sub.add_parser("run", help="run the benchmark matrix")
    _add_common(p)
    p.add_argument("--models", required=True, help="comma-separated model ids")
    p.add_argument("--variants", default="A,B")
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--tasks", help="comma-separated task ids")
    p.add_argument("--repos", help="comma-separated repo ids")
    _add_run_opts(p, 25.0)
    p.set_defaults(fn=cmd_run)

    p = sub.add_parser("check-model", help="1-task A/B compatibility smoke for a model")
    _add_common(p)
    p.add_argument("--model", required=True)
    p.add_argument("--task", help="task id (default: first answer task)")
    _add_run_opts(p, 5.0)
    p.set_defaults(fn=cmd_check_model)

    p = sub.add_parser("report", help="write summary.json, report.md, index.html (+ public/)")
    _add_common(p)
    p.add_argument("--out", required=True, help="results dir written by run")
    p.add_argument("--allow-out-in-repo", action="store_true", help=argparse.SUPPRESS)
    p.set_defaults(fn=cmd_report)

    args = ap.parse_args(argv)
    try:
        return args.fn(args)
    except (cfgmod.ConfigError, prep.PrepareError, RunError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
