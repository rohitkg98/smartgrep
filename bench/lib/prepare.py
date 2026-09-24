"""Fetching pinned repositories into the cache.

Layout under the cache dir:

  work/<id>/              canonical checkout path; setup runs here and every
                          agent run happens here (restored before each run)
  pristine/<id>/          snapshot of work/<id> right after checkout + setup
  pristine/<id>.json      marker: sha + setup commands of the snapshot

Why a fixed work path instead of a fresh temp dir per run: setup steps such as
``pip install -e .`` bake absolute paths into ``.venv``. If the checkout were
copied elsewhere, tests would import the pristine code, not the agent's edits.
Runs are sequential, so one canonical path per repo is enough.
"""
from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

from .config import Repo


class PrepareError(Exception):
    pass


def work_path(cache: Path, repo_id: str) -> Path:
    return cache / "work" / repo_id


def pristine_path(cache: Path, repo_id: str) -> Path:
    return cache / "pristine" / repo_id


def _marker(cache: Path, repo_id: str) -> Path:
    return cache / "pristine" / f"{repo_id}.json"


def _marker_data(repo: Repo) -> dict:
    return {"id": repo.id, "sha": repo.sha, "url": repo.url, "setup": repo.setup}


def is_prepared(cache: Path, repo: Repo) -> bool:
    m = _marker(cache, repo.id)
    if not m.is_file() or not pristine_path(cache, repo.id).is_dir():
        return False
    try:
        return json.loads(m.read_text()) == _marker_data(repo)
    except (OSError, ValueError):
        return False


def _git(args: list[str], cwd: Path, log) -> str:
    proc = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise PrepareError(f"git {' '.join(args)} failed in {cwd}:\n{proc.stderr.strip()}")
    return proc.stdout


def copy_tree(src: Path, dst: Path) -> None:
    """Copy a directory preserving symlinks/modes (like ``cp -a``)."""
    if dst.exists():
        shutil.rmtree(dst)
    dst.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(["cp", "-a", str(src), str(dst)], capture_output=True, text=True)
    if proc.returncode != 0:
        if dst.exists():
            shutil.rmtree(dst)
        shutil.copytree(src, dst, symlinks=True)


def prepare_repo(cache: Path, repo: Repo, log=print, force: bool = False) -> Path:
    """Fetch exactly ``repo.sha``, run setup once, snapshot. Idempotent."""
    if not force and is_prepared(cache, repo):
        log(f"[prepare] {repo.id}: already prepared at {repo.sha[:12]}")
        return pristine_path(cache, repo.id)
    work = work_path(cache, repo.id)
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    log(f"[prepare] {repo.id}: fetching {repo.url} @ {repo.sha[:12]} ({repo.ref})")
    _git(["init", "-q"], work, log)
    _git(["remote", "add", "origin", repo.url], work, log)
    _git(["fetch", "-q", "--depth", "1", "origin", repo.sha], work, log)
    _git(["-c", "advice.detachedHead=false", "checkout", "-q", "FETCH_HEAD"], work, log)
    head = _git(["rev-parse", "HEAD"], work, log).strip()
    if head != repo.sha:
        raise PrepareError(f"{repo.id}: HEAD is {head}, expected {repo.sha}")
    for cmd in repo.setup:
        log(f"[prepare] {repo.id}: $ {cmd}")
        proc = subprocess.run(cmd, shell=True, cwd=work, capture_output=True, text=True)
        if proc.returncode != 0:
            tail = (proc.stdout + proc.stderr)[-2000:]
            raise PrepareError(f"{repo.id}: setup command failed (exit {proc.returncode}): {cmd}\n{tail}")
    pristine = pristine_path(cache, repo.id)
    copy_tree(work, pristine)
    _marker(cache, repo.id).write_text(json.dumps(_marker_data(repo), indent=2))
    log(f"[prepare] {repo.id}: ready")
    return pristine


def restore_work(cache: Path, repo: Repo) -> Path:
    """Reset work/<id> to the pristine snapshot and return its path."""
    if not is_prepared(cache, repo):
        raise PrepareError(f"repo {repo.id!r} is not prepared (or its definition changed): "
                           f"run `bench.py prepare --repos {repo.id}` first")
    work = work_path(cache, repo.id)
    copy_tree(pristine_path(cache, repo.id), work)
    head = subprocess.run(["git", "-C", str(work), "rev-parse", "HEAD"],
                          capture_output=True, text=True).stdout.strip()
    if head != repo.sha:
        raise PrepareError(f"{repo.id}: restored checkout is at {head}, expected {repo.sha}")
    return work


def remove_work(cache: Path, repo_id: str) -> None:
    work = work_path(cache, repo_id)
    if work.exists():
        shutil.rmtree(work, ignore_errors=True)
