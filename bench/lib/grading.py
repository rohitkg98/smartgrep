"""Grading of agent runs.

answer tasks: parse the final ``ANSWER: [...]`` line and score it against the gold
list (precision / recall / F1).  change tasks: copy hidden tests over the work
tree, run the task's test command, pass = exit code 0.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

from .proc import run_killable

ANSWER_RE = re.compile(r"^\s*(?:\*\*)?ANSWER:(?:\*\*)?\s*(.*?)\s*$", re.MULTILINE)
SEGMENT_SPLIT = re.compile(r"::|[./#]")


def normalize(item: str) -> str:
    s = str(item).strip()
    for _ in range(3):
        before = s
        s = s.strip().strip("`").strip()
        if len(s) >= 2 and s[0] == s[-1] and s[0] in "'\"":
            s = s[1:-1]
        if s.endswith("()"):
            s = s[:-2]
        if s == before:
            break
    return s.strip()


def last_segment(name: str) -> str:
    parts = [p for p in SEGMENT_SPLIT.split(name) if p]
    return parts[-1] if parts else name


def _strip_fences(s: str) -> str:
    s = s.strip()
    s = re.sub(r"^```[A-Za-z]*\s*", "", s)
    s = re.sub(r"\s*```$", "", s)
    s = s.strip().strip("`").strip()
    return s.rstrip(".").strip()


def parse_answer(text: str | None) -> tuple[list[str] | None, str | None]:
    """Return (items, error). Uses the LAST ``ANSWER:`` line of the text."""
    if not text:
        return None, "no final result text"
    matches = list(ANSWER_RE.finditer(text))
    if not matches:
        return None, "no ANSWER: line found"
    m = matches[-1]
    candidates = [m.group(1), text[m.start(1):]]  # same line, or the array spilling onto later lines
    for cand in candidates:
        cand = _strip_fences(cand)
        if not cand:
            continue
        if cand.startswith("[") and "]" in cand:
            cand = cand[: cand.rindex("]") + 1]
        try:
            val = json.loads(cand)
        except ValueError:
            continue
        if isinstance(val, list):
            return [str(v) for v in val], None
        return None, "ANSWER is not a JSON array"
    return None, "ANSWER line is not valid JSON"


def score_answer(items: list[str], gold: list, pass_f1: float = 0.9) -> dict:
    """Match answer items to gold entries (each entry = str or list of aliases)."""
    entries = [[normalize(a) for a in (g if isinstance(g, list) else [g])] for g in gold]
    # dedupe answer items (after normalization), keep order
    seen, answer = set(), []
    for it in items:
        n = normalize(it)
        if n and n not in seen:
            seen.add(n)
            answer.append(n)
    seg_owners: dict[str, set[int]] = {}
    for gi, aliases in enumerate(entries):
        for a in aliases:
            seg_owners.setdefault(last_segment(a), set()).add(gi)

    matched: dict[int, int] = {}  # answer idx -> gold idx
    used: set[int] = set()
    redundant: list[str] = []
    # pass 1: exact alias match
    for ai, it in enumerate(answer):
        for gi, aliases in enumerate(entries):
            if it in aliases:
                if gi in used:
                    redundant.append(it)
                else:
                    matched[ai] = gi
                    used.add(gi)
                break
    # pass 2: unique last segment
    for ai, it in enumerate(answer):
        if ai in matched or it in redundant:
            continue
        owners = seg_owners.get(last_segment(it), set())
        if len(owners) == 1:
            gi = next(iter(owners))
            if gi not in used:
                matched[ai] = gi
                used.add(gi)
    tp = len(matched)
    # an item that is another alias of an already-matched entry is neither right nor wrong
    denom = len(answer) - len(redundant)
    precision = tp / denom if denom else 0.0
    recall = tp / len(entries) if entries else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {
        "precision": round(precision, 4),
        "recall": round(recall, 4),
        "f1": round(f1, 4),
        "pass": f1 >= pass_f1 - 1e-9,
        "answer": answer,
        "matched": {answer[ai]: gi for ai, gi in matched.items()},
        "false_positives": [a for i, a in enumerate(answer) if i not in matched and a not in redundant],
        "missed": [entries[gi][0] for gi in range(len(entries)) if gi not in used],
        "redundant": redundant,
    }


def grade_answer(result_text: str | None, gold: list, pass_f1: float = 0.9) -> dict:
    items, err = parse_answer(result_text)
    if items is None:
        return {"precision": 0.0, "recall": 0.0, "f1": 0.0, "pass": False,
                "answer": None, "grade_error": err}
    g = score_answer(items, gold, pass_f1)
    g["grade_error"] = None
    return g


def grade_change(workdir: Path, hidden_dir: Path, test_cmd: str, env: dict,
                 timeout_s: int = 900) -> dict:
    """Copy hidden tests over the work tree and run the test command."""
    shutil.copytree(hidden_dir, workdir, dirs_exist_ok=True)
    with tempfile.TemporaryFile() as out_f:
        code, timed_out = run_killable(["bash", "-c", test_cmd], cwd=workdir, env=env,
                                       timeout=timeout_s, stdout=out_f, stderr=subprocess.STDOUT)
        out_f.seek(0, os.SEEK_END)
        size = out_f.tell()
        out_f.seek(max(0, size - 4096))
        tail = out_f.read().decode(errors="replace")
    return {"pass": (not timed_out) and code == 0, "exit_code": code, "output_tail": tail,
            "grade_error": f"test command timed out after {timeout_s}s" if timed_out else None}
