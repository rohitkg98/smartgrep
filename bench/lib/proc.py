"""Subprocess helper: run with a wall-clock timeout that kills the whole process group."""
from __future__ import annotations

import os
import signal
import subprocess


def run_killable(cmd, *, cwd=None, env=None, timeout=None, stdout=None, stderr=None,
                 stdin=subprocess.DEVNULL) -> tuple[int | None, bool]:
    """Run ``cmd``; return (returncode, timed_out). On timeout the process group is killed."""
    proc = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=stdin, stdout=stdout, stderr=stderr,
                            start_new_session=True)
    try:
        proc.wait(timeout=timeout)
        return proc.returncode, False
    except subprocess.TimeoutExpired:
        _kill_group(proc)
        return None, True
    except BaseException:
        _kill_group(proc)
        raise


def _kill_group(proc: subprocess.Popen) -> None:
    for sig in (signal.SIGTERM, signal.SIGKILL):
        try:
            os.killpg(proc.pid, sig)
        except (ProcessLookupError, PermissionError):
            return
        try:
            proc.wait(timeout=5)
            return
        except subprocess.TimeoutExpired:
            continue
