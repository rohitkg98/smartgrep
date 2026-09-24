"""Executing the run matrix: environment isolation, budget guard, resume, grading."""
from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from . import prepare as prep
from .config import Task, TaskSet, git_head
from .grading import grade_answer, grade_change
from .matrix import PlannedRun, build_prompt, plan
from .openrouter import OpenRouterCost
from .store import RECORD_SCHEMA_VERSION, Store, completed_keys, infra_attempts, total_spent

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_BASE_PATH = "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin"
PASSTHROUGH_ENV = ("LANG", "LC_ALL", "LC_CTYPE", "TMPDIR", "USER", "LOGNAME", "TZ")
SNAPSHOT_EXCLUDES = (".venv/", ".smartgrep/")


class RunError(Exception):
    """A configuration problem that makes further runs pointless."""


@dataclass
class RunConfig:
    taskset: TaskSet
    tasks: list[Task]
    models: list[str]
    variants: list[str]
    repeats: int
    out: Path
    cache: Path
    smartgrep: Path | None
    budget_usd: float
    max_run_usd: float = 2.0
    effort: str | None = "high"
    max_turns: int = 60
    timeout_s: float = 1200
    seed: int = 0
    max_attempts: int = 3
    max_consecutive_infra: int = 3
    base_path: str = DEFAULT_BASE_PATH
    dry_run: bool = False
    log: object = print
    extra: dict = field(default_factory=dict)


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def clean_base_path(base_path: str) -> str:
    dirs = []
    for d in base_path.split(os.pathsep):
        if not d or not os.path.isdir(d) or d in dirs:
            continue
        if shutil.which("smartgrep", path=d):
            raise RunError(f"base PATH directory {d} contains a smartgrep binary; variant B "
                           "would see it. Remove it or pass --base-path without that dir.")
        dirs.append(d)
    return os.pathsep.join(dirs)


def smartgrep_info(smartgrep: Path | None, env: dict) -> dict:
    info = {"smartgrep_path": str(smartgrep) if smartgrep else None, "smartgrep_version": None,
            "smartgrep_binary_sha256": None, "smartgrep_git_sha": git_head(REPO_ROOT)}
    if smartgrep:
        try:
            p = subprocess.run([str(smartgrep), "--version"], capture_output=True, text=True,
                               env=env, timeout=30, stdin=subprocess.DEVNULL)
            if p.returncode == 0 and p.stdout.strip():
                info["smartgrep_version"] = p.stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            pass
        h = hashlib.sha256()
        with open(smartgrep, "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
        info["smartgrep_binary_sha256"] = h.hexdigest()[:16]
    return info


def snapshot_tree(work: Path) -> str | None:
    """Write the current work tree (tracked + untracked, minus .venv/.smartgrep) as a git
    tree object without touching HEAD or the real index. Returns the tree id."""
    with tempfile.TemporaryDirectory() as td:
        env = dict(os.environ, GIT_INDEX_FILE=os.path.join(td, "index"))
        excludes = os.path.join(td, "excludes")
        with open(excludes, "w") as f:
            f.write("\n".join(SNAPSHOT_EXCLUDES) + "\n")
        add = subprocess.run(["git", "-c", f"core.excludesFile={excludes}", "add", "-A", "--", "."],
                             cwd=work, env=env, capture_output=True, text=True)
        if add.returncode != 0:
            return None
        tree = subprocess.run(["git", "write-tree"], cwd=work, env=env, capture_output=True, text=True)
        return tree.stdout.strip() if tree.returncode == 0 else None


def tree_diff(work: Path, before: str | None, after: str | None) -> str:
    if not before or not after:
        return ""
    p = subprocess.run(["git", "diff", before, after], cwd=work, capture_output=True, text=True)
    return p.stdout


class Runner:
    def __init__(self, cfg: RunConfig, agent, cost: OpenRouterCost):
        self.cfg = cfg
        self.agent = agent
        self.cost = cost
        self.store = Store(cfg.out)
        self.log = cfg.log

    # -- environment -------------------------------------------------------------------
    def _base_env(self, home: Path, path: str) -> dict:
        env = {k: os.environ[k] for k in PASSTHROUGH_ENV if k in os.environ}
        env.setdefault("LANG", "C.UTF-8")
        env.update({"HOME": str(home), "PATH": path, "TERM": "dumb",
                    "SHELL": "/bin/bash" if os.path.exists("/bin/bash") else "/bin/sh"})
        return env

    def _variant_path(self, variant: str, shim_root: Path, base: str) -> str:
        if variant == "A":
            shim = shim_root / "bin-A"
            shim.mkdir(parents=True, exist_ok=True)
            link = shim / "smartgrep"
            if not link.exists():
                link.symlink_to(self.cfg.smartgrep)
            path = f"{shim}{os.pathsep}{base}"
            found = shutil.which("smartgrep", path=path)
            if not found or Path(found) != link:
                raise RunError(f"variant A: smartgrep resolves to {found}, expected {link}")
        else:
            path = base
            found = shutil.which("smartgrep", path=path)
            if found:
                raise RunError(f"variant B: smartgrep must not be on PATH, but found {found}")
        return path

    # -- main loop ---------------------------------------------------------------------
    def run(self) -> dict:
        cfg = self.cfg
        runs = plan(cfg.models, cfg.tasks, cfg.variants, cfg.repeats, cfg.seed)
        agent_name = self.agent.name
        records = self.store.records()
        done = completed_keys(records)
        attempts = infra_attempts(records)
        pending = [p for p in runs if p.run_key(agent_name, cfg.effort) not in done]
        gave_up = [p for p in pending if attempts.get(p.run_key(agent_name, cfg.effort), 0) >= cfg.max_attempts]
        todo = [p for p in pending if p not in gave_up]
        spent = total_spent(records)
        self.log(f"[run] plan: {len(runs)} runs (seed {cfg.seed}); done {len(runs) - len(pending)}, "
                 f"to do {len(todo)}, given up after {cfg.max_attempts} infra errors {len(gave_up)}; "
                 f"spent so far ${spent:.2f} of ${cfg.budget_usd:.2f}")
        summary = {"planned": len(runs), "todo": len(todo), "executed": 0, "ok": 0,
                   "infra_error": 0, "stopped": None, "gave_up": len(gave_up)}
        if cfg.dry_run:
            for p in runs:
                key = p.run_key(agent_name, cfg.effort)
                state = "done" if key in done else ("gave-up" if p in gave_up else "todo")
                self.log(f"  {p.index:4d} {state:8s} {key}")
            summary["stopped"] = "dry-run"
            return summary
        if cfg.smartgrep is None and "A" in cfg.variants:
            raise RunError("variant A needs --smartgrep /path/to/smartgrep")
        for repo_id in sorted({cfg.taskset.task(p.task_id).repo for p in todo}):
            if not prep.is_prepared(cfg.cache, cfg.taskset.repos[repo_id]):
                raise RunError(f"repo {repo_id!r} is not prepared in {cfg.cache}: "
                               f"run `bench.py prepare --repos {repo_id}` first")
        self.store.init()
        base = clean_base_path(cfg.base_path)
        consecutive_infra = 0
        with tempfile.TemporaryDirectory(prefix="sgbench-") as td:
            shim_root = Path(td)
            probe_env = self._base_env(shim_root, base)
            invocation = {
                "invocation_started": now(), "seed": cfg.seed,
                "agent": agent_name, "agent_version": self.agent.version(probe_env),
                "tasks_repo_sha": cfg.taskset.git_sha(),
                **smartgrep_info(cfg.smartgrep, probe_env),
            }
            for p in todo:
                key = p.run_key(agent_name, cfg.effort)
                est = self._estimate(records, p.model)
                if spent + est > cfg.budget_usd:
                    msg = (f"budget stop: spent ${spent:.2f} + next-run estimate ${est:.2f} "
                           f"> budget ${cfg.budget_usd:.2f}")
                    self.log(f"[run] {msg}")
                    summary["stopped"] = msg
                    break
                self.log(f"[run] {p.index + 1}/{len(runs)} {key} (spent ${spent:.2f})")
                rec = self._one(p, key, shim_root, base, invocation)
                self.store.append(rec)
                records.append(rec)
                spent += float(rec.get("cost_usd") or 0)
                summary["executed"] += 1
                summary[rec["status"]] = summary.get(rec["status"], 0) + 1
                g = rec.get("grade") or {}
                self.log(f"[run]   -> {rec['status']}"
                         + (f" ({rec.get('error')})" if rec.get("error") else "")
                         + f" pass={g.get('pass')} tokens={(rec.get('tokens') or {}).get('total')} "
                           f"cost=${float(rec.get('cost_usd') or 0):.4f} [{rec.get('cost_source')}]")
                if rec["status"] == "infra_error":
                    consecutive_infra += 1
                    if rec.get("fatal"):
                        summary["stopped"] = f"fatal agent error: {rec.get('error')}"
                        self.log(f"[run] stopping: {summary['stopped']}")
                        break
                    if consecutive_infra >= cfg.max_consecutive_infra:
                        summary["stopped"] = f"{consecutive_infra} consecutive infra errors"
                        self.log(f"[run] stopping: {summary['stopped']}; re-run later to retry")
                        break
                else:
                    consecutive_infra = 0
        self.log(f"[run] finished: executed {summary['executed']}, spent ${spent:.2f}")
        summary["spent_usd"] = round(spent, 4)
        return summary

    def _estimate(self, records: list[dict], model: str) -> float:
        costs = [float(r["cost_usd"]) for r in records
                 if r.get("model") == model and r.get("status") == "ok" and r.get("cost_usd") is not None]
        return sum(costs) / len(costs) if costs else self.cfg.max_run_usd

    def _one(self, p: PlannedRun, key: str, shim_root: Path, base: str, invocation: dict) -> dict:
        cfg = self.cfg
        task = cfg.taskset.task(p.task_id)
        repo = cfg.taskset.repos[task.repo]
        prompt = build_prompt(task)
        rec: dict = {
            "record_schema_version": RECORD_SCHEMA_VERSION, "run_key": key, "status": None,
            "model": p.model, "effort": cfg.effort, "max_turns": cfg.max_turns,
            "task_id": task.id, "repo_id": repo.id, "repo_sha": repo.sha, "task_kind": task.kind,
            "task_hash": task.definition_hash(), "variant": p.variant, "repeat": p.repeat,
            "plan_index": p.index, "started_at": now(), **invocation,
        }
        home = Path(tempfile.mkdtemp(prefix="home-", dir=shim_root))
        work = None
        try:
            path = self._variant_path(p.variant, shim_root, base)
            env = self._base_env(home, path)
            work = prep.restore_work(cfg.cache, repo)
            if p.variant == "A":
                t0 = time.monotonic()
                init = subprocess.run([str(cfg.smartgrep), "init"], cwd=work, env=env,
                                      capture_output=True, text=True, stdin=subprocess.DEVNULL)
                rec["smartgrep_init_s"] = round(time.monotonic() - t0, 2)
                if init.returncode != 0:
                    raise RunError(f"smartgrep init failed in {work}: {init.stderr.strip()[:500]}")
            baseline = snapshot_tree(work) if task.kind == "change" else None
            agent_env = self.agent.build_env(env, p.model)
            key_before = self.cost.key_usage() if self.cost.enabled else None
            res = self.agent.run(prompt, work, agent_env, p.model, cfg.effort, cfg.max_turns,
                                 cfg.timeout_s, cfg.max_run_usd,
                                 self.store.transcript_path(key), self.store.stderr_path(key))
            m = res.metrics
            rec.update({
                "status": res.status, "error": res.error, "fatal": res.fatal,
                "exit_code": res.exit_code, "timed_out": res.timed_out, "wall_s": res.wall_s,
                "tokens": m.get("tokens"), "tokens_source": m.get("tokens_source"),
                "num_turns": m.get("num_turns"), "duration_ms": m.get("duration_ms"),
                "is_error": m.get("is_error"), "subtype": m.get("subtype"),
                "terminal_reason": m.get("terminal_reason"),
                "api_error_status": m.get("api_error_status"), "api_retries": m.get("api_retries"),
                "cc_cost_usd": m.get("cc_cost_usd"), "models_used": m.get("models_used"),
                "tools": m.get("tools"), "smartgrep_calls": m.get("smartgrep_calls"),
                "used_smartgrep": bool(m.get("smartgrep_calls")),
                "subagents_spawned": m.get("subagents_spawned"),
                "result_text": m.get("result_text"),
                "transcript": str(res.transcript_path.relative_to(cfg.out)),
            })
            rec.update(self.cost.run_cost(m.get("generation_ids") or [], key_before,
                                          m.get("cc_cost_usd"), m.get("subagents_spawned")))
            if res.status == "ok":
                if task.kind == "answer":
                    rec["grade"] = grade_answer(m.get("result_text"), task.gold, task.pass_f1)
                else:
                    diff = tree_diff(work, baseline, snapshot_tree(work))
                    dp = self.store.diff_path(key)
                    dp.write_text(diff)
                    rec["diff"] = str(dp.relative_to(cfg.out))
                    rec["diff_bytes"] = len(diff.encode())
                    test_env = self._base_env(home, path)
                    rec["grade"] = grade_change(work, cfg.taskset.hidden_dir(task), task.test_cmd,
                                                test_env, task.timeout_s)
                rec["pass"] = bool(rec["grade"]["pass"])
        finally:
            prep.remove_work(cfg.cache, repo.id)
            shutil.rmtree(home, ignore_errors=True)
            rec["finished_at"] = now()
        return rec
