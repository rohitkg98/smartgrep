"""Loading and validating the (private) task set: repos.toml, tasks/*.toml, hidden/.

The task set lives outside this repository. Its location comes from
``--tasks-dir`` or the ``SMARTGREP_BENCH_TASKS`` environment variable.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

TASKS_ENV = "SMARTGREP_BENCH_TASKS"
CACHE_ENV = "SMARTGREP_BENCH_CACHE"
DEFAULT_CACHE = "~/.cache/smartgrep-bench"

ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")
TASK_KINDS = ("answer", "change")


class ConfigError(Exception):
    """A task-set file is missing or malformed. The message says where and why."""


@dataclass
class Repo:
    id: str
    url: str
    ref: str
    sha: str
    language: str
    setup: list[str] = field(default_factory=list)


@dataclass
class Task:
    id: str
    repo: str
    kind: str
    prompt: str
    source: str  # file the task was loaded from
    answer_format: str | None = None
    gold: list | None = None  # list of str | list[str]
    pass_f1: float = 0.9
    hidden_tests: str | None = None
    test_cmd: str | None = None
    timeout_s: int = 900
    raw: dict = field(default_factory=dict)

    def definition_hash(self) -> str:
        """Stable hash of the task definition (prompt, gold, tests...), for provenance."""
        blob = json.dumps(self.raw, sort_keys=True, ensure_ascii=False).encode()
        return "sha256:" + hashlib.sha256(blob).hexdigest()[:16]


@dataclass
class TaskSet:
    root: Path
    repos: dict[str, Repo]
    tasks: list[Task]

    def task(self, task_id: str) -> Task:
        for t in self.tasks:
            if t.id == task_id:
                return t
        raise ConfigError(f"unknown task id {task_id!r}")

    def hidden_dir(self, task: Task) -> Path:
        return self.root / "hidden" / task.hidden_tests

    def git_sha(self) -> str | None:
        return git_head(self.root)


def git_head(path: Path) -> str | None:
    try:
        out = subprocess.run(["git", "-C", str(path), "rev-parse", "HEAD"],
                             capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return out.stdout.strip() if out.returncode == 0 else None


def resolve_tasks_dir(arg: str | None) -> Path:
    value = arg or os.environ.get(TASKS_ENV)
    if not value:
        raise ConfigError(
            f"no task set given: pass --tasks-dir DIR or set {TASKS_ENV} "
            "(the directory holding repos.toml, tasks/ and hidden/)")
    path = Path(value).expanduser().resolve()
    if not (path / "repos.toml").is_file():
        raise ConfigError(f"{path}: no repos.toml found (is this the task-set directory?)")
    return path


def resolve_cache_dir(arg: str | None) -> Path:
    return Path(arg or os.environ.get(CACHE_ENV) or DEFAULT_CACHE).expanduser().resolve()


def _load_toml(path: Path) -> dict:
    try:
        with open(path, "rb") as f:
            return tomllib.load(f)
    except tomllib.TOMLDecodeError as e:
        raise ConfigError(f"{path}: invalid TOML: {e}") from None


def _req(entry: dict, key: str, typ, where: str):
    if key not in entry:
        raise ConfigError(f"{where}: missing required field {key!r}")
    val = entry[key]
    if not isinstance(val, typ) or (isinstance(val, str) and not val.strip()):
        raise ConfigError(f"{where}: field {key!r} must be a non-empty {typ.__name__}")
    return val


def load_repos(path: Path) -> dict[str, Repo]:
    data = _load_toml(path)
    entries = data.get("repo")
    if not isinstance(entries, list) or not entries:
        raise ConfigError(f"{path}: expected at least one [[repo]] table")
    repos: dict[str, Repo] = {}
    for i, e in enumerate(entries):
        where = f"{path} [[repo]] #{i + 1}"
        rid = _req(e, "id", str, where)
        where = f"{path} repo {rid!r}"
        if not ID_RE.match(rid):
            raise ConfigError(f"{where}: id must match {ID_RE.pattern}")
        if rid in repos:
            raise ConfigError(f"{where}: duplicate repo id")
        sha = _req(e, "sha", str, where)
        if not SHA_RE.match(sha):
            raise ConfigError(f"{where}: sha must be a full 40-char lowercase hex commit id")
        setup = e.get("setup", [])
        if not isinstance(setup, list) or not all(isinstance(s, str) for s in setup):
            raise ConfigError(f"{where}: setup must be a list of strings")
        unknown = set(e) - {"id", "url", "ref", "sha", "language", "setup"}
        if unknown:
            raise ConfigError(f"{where}: unknown field(s) {sorted(unknown)}")
        repos[rid] = Repo(id=rid, url=_req(e, "url", str, where), ref=_req(e, "ref", str, where),
                          sha=sha, language=_req(e, "language", str, where), setup=setup)
    return repos


def _validate_gold(gold, where: str) -> list:
    if not isinstance(gold, list) or not gold:
        raise ConfigError(f"{where}: gold must be a non-empty array")
    for g in gold:
        if isinstance(g, str):
            if not g.strip():
                raise ConfigError(f"{where}: gold contains an empty string")
        elif isinstance(g, list):
            if not g or not all(isinstance(a, str) and a.strip() for a in g):
                raise ConfigError(f"{where}: each gold alias list must be a non-empty list of strings")
        else:
            raise ConfigError(f"{where}: gold entries must be strings or lists of alias strings")
    return gold


TASK_FIELDS = {"id", "repo", "kind", "prompt", "answer_format", "gold", "pass_f1",
               "hidden_tests", "test_cmd", "timeout_s"}


def load_tasks(path: Path, repos: dict[str, Repo], root: Path) -> list[Task]:
    data = _load_toml(path)
    entries = data.get("task")
    if not isinstance(entries, list):
        raise ConfigError(f"{path}: expected [[task]] tables")
    tasks = []
    for i, e in enumerate(entries):
        where = f"{path} [[task]] #{i + 1}"
        tid = _req(e, "id", str, where)
        where = f"{path} task {tid!r}"
        if not ID_RE.match(tid):
            raise ConfigError(f"{where}: id must match {ID_RE.pattern}")
        unknown = set(e) - TASK_FIELDS
        if unknown:
            raise ConfigError(f"{where}: unknown field(s) {sorted(unknown)}")
        repo = _req(e, "repo", str, where)
        if repo not in repos:
            raise ConfigError(f"{where}: repo {repo!r} is not defined in repos.toml")
        kind = _req(e, "kind", str, where)
        if kind not in TASK_KINDS:
            raise ConfigError(f"{where}: kind must be one of {TASK_KINDS}, got {kind!r}")
        prompt = _req(e, "prompt", str, where)
        t = Task(id=tid, repo=repo, kind=kind, prompt=prompt.strip(), source=str(path), raw=dict(e))
        if kind == "answer":
            t.gold = _validate_gold(e.get("gold"), where)
            af = e.get("answer_format")
            if af is not None and not isinstance(af, str):
                raise ConfigError(f"{where}: answer_format must be a string")
            t.answer_format = af
            pf = e.get("pass_f1", 0.9)
            if not isinstance(pf, (int, float)) or isinstance(pf, bool) or not 0 < pf <= 1:
                raise ConfigError(f"{where}: pass_f1 must be a number in (0, 1]")
            t.pass_f1 = float(pf)
            for bad in ("hidden_tests", "test_cmd"):
                if bad in e:
                    raise ConfigError(f"{where}: {bad!r} only applies to kind='change'")
        else:
            t.hidden_tests = _req(e, "hidden_tests", str, where)
            t.test_cmd = _req(e, "test_cmd", str, where)
            if not (root / "hidden" / t.hidden_tests).is_dir():
                raise ConfigError(f"{where}: hidden tests directory hidden/{t.hidden_tests}/ not found in {root}")
            if "gold" in e:
                raise ConfigError(f"{where}: 'gold' only applies to kind='answer'")
        to = e.get("timeout_s", 900)
        if not isinstance(to, int) or isinstance(to, bool) or to <= 0:
            raise ConfigError(f"{where}: timeout_s must be a positive integer")
        t.timeout_s = to
        tasks.append(t)
    return tasks


def load_taskset(root: Path) -> TaskSet:
    repos = load_repos(root / "repos.toml")
    tasks: list[Task] = []
    seen: dict[str, str] = {}
    task_dir = root / "tasks"
    if not task_dir.is_dir():
        raise ConfigError(f"{root}: no tasks/ directory")
    for path in sorted(task_dir.glob("*.toml")):
        for t in load_tasks(path, repos, root):
            if t.id in seen:
                raise ConfigError(f"{path}: duplicate task id {t.id!r} (also in {seen[t.id]})")
            seen[t.id] = str(path)
            tasks.append(t)
    if not tasks:
        raise ConfigError(f"{task_dir}: no tasks found")
    return TaskSet(root=root, repos=repos, tasks=tasks)


def select_tasks(ts: TaskSet, task_ids: list[str] | None, repo_ids: list[str] | None) -> list[Task]:
    tasks = ts.tasks
    if repo_ids:
        for r in repo_ids:
            if r not in ts.repos:
                raise ConfigError(f"unknown repo id {r!r}")
        tasks = [t for t in tasks if t.repo in repo_ids]
    if task_ids:
        known = {t.id for t in ts.tasks}
        for tid in task_ids:
            if tid not in known:
                raise ConfigError(f"unknown task id {tid!r}")
        tasks = [t for t in tasks if t.id in task_ids]
    if not tasks:
        raise ConfigError("no tasks selected")
    return tasks
