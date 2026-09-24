"""Shared test helpers: import path, tiny git repo, task-set copy, fake binaries."""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

BENCH = Path(__file__).resolve().parents[1]
if str(BENCH) not in sys.path:
    sys.path.insert(0, str(BENCH))

FIXTURES = Path(__file__).resolve().parent / "fixtures"
STREAMS = FIXTURES / "streams"
BASE_PATH = "/usr/bin:/bin"  # smartgrep must not live here

GIT_ENV = {
    "GIT_AUTHOR_NAME": "bench", "GIT_AUTHOR_EMAIL": "bench@example.invalid",
    "GIT_COMMITTER_NAME": "bench", "GIT_COMMITTER_EMAIL": "bench@example.invalid",
    "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z", "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z",
}


def git(args, cwd):
    env = dict(os.environ, **GIT_ENV)
    return subprocess.run(["git", "-c", "commit.gpgsign=false", "-c", "init.defaultBranch=main", *args],
                          cwd=cwd, env=env, check=True, capture_output=True, text=True).stdout.strip()


def make_tiny_repo(tmp: Path) -> tuple[str, str]:
    """Create a git repo from fixtures/tinyrepo. Returns (file:// url, sha)."""
    src = tmp / "tiny-src"
    shutil.copytree(FIXTURES / "tinyrepo", src)
    git(["init", "-q"], src)
    git(["config", "uploadpack.allowAnySHA1InWant", "true"], src)
    git(["add", "-A"], src)
    git(["commit", "-q", "-m", "tiny"], src)
    return src.as_uri(), git(["rev-parse", "HEAD"], src)


def make_taskset(tmp: Path, url: str = "file:///nonexistent/tiny",
                 sha: str = "0" * 40) -> Path:
    dst = tmp / "taskset"
    shutil.copytree(FIXTURES / "taskset", dst)
    p = dst / "repos.toml"
    p.write_text(p.read_text().replace("file:///nonexistent/tiny", url).replace("0" * 40, sha))
    return dst


def install_fake_bins(tmp: Path) -> tuple[Path, Path]:
    """Copy fake claude/smartgrep with a shebang for the running interpreter."""
    bindir = tmp / "fakebin"
    bindir.mkdir()
    out = []
    for name, target in (("fake_claude.py", "claude"), ("fake_smartgrep.py", "smartgrep")):
        body = (FIXTURES / "bin" / name).read_text().split("\n", 1)[1]
        # The smartgrep stand-in lives in its own dir so it is never on the base PATH.
        d = bindir / target
        d.mkdir()
        dst = d / target
        dst.write_text(f"#!{sys.executable}\n{body}")
        dst.chmod(0o755)
        out.append(dst)
    return out[0], out[1]
