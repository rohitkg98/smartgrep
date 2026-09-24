"""The results directory: runs.jsonl (append-only, one record per attempt) + artifacts."""
from __future__ import annotations

import json
import os
from pathlib import Path

RECORD_SCHEMA_VERSION = 1


class Store:
    def __init__(self, out: Path):
        self.out = out
        self.runs_path = out / "runs.jsonl"

    def init(self) -> None:
        for sub in ("transcripts", "diffs", "stderr"):
            (self.out / sub).mkdir(parents=True, exist_ok=True)

    def transcript_path(self, run_key: str) -> Path:
        return self.out / "transcripts" / f"{run_key}.jsonl"

    def stderr_path(self, run_key: str) -> Path:
        return self.out / "stderr" / f"{run_key}.txt"

    def diff_path(self, run_key: str) -> Path:
        return self.out / "diffs" / f"{run_key}.diff"

    def records(self) -> list[dict]:
        if not self.runs_path.is_file():
            return []
        recs = []
        with open(self.runs_path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    recs.append(json.loads(line))
                except ValueError:
                    continue  # e.g. a line truncated by a crash mid-write
        return recs

    def append(self, record: dict) -> None:
        self.out.mkdir(parents=True, exist_ok=True)
        line = json.dumps(record, ensure_ascii=False, sort_keys=True)
        with open(self.runs_path, "a", encoding="utf-8") as f:
            f.write(line + "\n")
            f.flush()
            os.fsync(f.fileno())


def completed_keys(records: list[dict]) -> set[str]:
    return {r["run_key"] for r in records if r.get("status") == "ok"}


def infra_attempts(records: list[dict]) -> dict[str, int]:
    n: dict[str, int] = {}
    for r in records:
        if r.get("status") == "infra_error":
            n[r["run_key"]] = n.get(r["run_key"], 0) + 1
    return n


def total_spent(records: list[dict]) -> float:
    return sum(float(r.get("cost_usd") or 0) for r in records)


def latest_ok(records: list[dict]) -> list[dict]:
    """One record per run_key: the last successful one (infra errors excluded)."""
    by_key: dict[str, dict] = {}
    for r in records:
        if r.get("status") == "ok":
            by_key[r["run_key"]] = r
    return list(by_key.values())
