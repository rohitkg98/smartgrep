"""Planning the run matrix (model x task x variant x repeat) and the prompt text."""
from __future__ import annotations

import random
import re
from dataclasses import dataclass

from .config import Task

VARIANTS = ("A", "B")

ANSWER_INSTRUCTIONS = (
    "Do not modify files. When you are done, end your reply with exactly one line of the form "
    "`ANSWER: <JSON array of strings>`{fmt}."
)
CHANGE_INSTRUCTIONS = "Make the change in the repository. Do not ask questions."


def build_prompt(task: Task) -> str:
    """Task prompt + standard instructions. Identical for both variants."""
    if task.kind == "answer":
        fmt = f" (items: {task.answer_format})" if task.answer_format else ""
        return f"{task.prompt}\n\n{ANSWER_INSTRUCTIONS.format(fmt=fmt)}"
    return f"{task.prompt}\n\n{CHANGE_INSTRUCTIONS}"


def slug(s: str) -> str:
    return re.sub(r"[^A-Za-z0-9._-]+", "_", s).strip("_")


@dataclass(frozen=True)
class PlannedRun:
    index: int
    model: str
    task_id: str
    variant: str
    repeat: int  # 1-based

    def run_key(self, agent: str, effort: str | None) -> str:
        return "__".join([slug(agent), slug(effort or "default"), slug(self.model),
                          slug(self.task_id), self.variant, f"r{self.repeat}"])


def plan(models: list[str], tasks: list[Task], variants: list[str], repeats: int,
         seed: int) -> list[PlannedRun]:
    """Deterministic order.

    repeat (outer) -> model -> task (seeded shuffle per repeat) -> variant.
    Both variants of one (model, task, repeat) are adjacent; which goes first alternates
    between repeats (A,B then B,A ...). Repeat-outer means a budget stop leaves a
    balanced partial result (every task seen once before any is seen twice).
    """
    for v in variants:
        if v not in VARIANTS:
            raise ValueError(f"unknown variant {v!r} (expected one of {VARIANTS})")
    if repeats < 1:
        raise ValueError("repeats must be >= 1")
    out: list[PlannedRun] = []
    for r in range(1, repeats + 1):
        order = list(variants) if r % 2 == 1 else list(reversed(variants))
        for model in models:
            ts = list(tasks)
            random.Random(f"{seed}:{r}:{model}").shuffle(ts)
            for t in ts:
                for v in order:
                    out.append(PlannedRun(len(out), model, t.id, v, r))
    return out
