"""Actual billed cost of a run, from OpenRouter.

Primary: sum ``total_cost`` of ``GET /v1/generation?id=<id>`` for every assistant
message id of the run (OpenRouter passes its generation id through as the message id).
Fallback: delta of ``GET /v1/key`` -> ``data.usage`` before/after the run (runs are
sequential, but anything else using the same key in that window is included).
Last resort: Claude Code's own estimate.

All HTTP goes through an injectable ``http_get(url, headers) -> (status, body_bytes)``
so tests never touch the network. The API key is only ever put in a header.
"""
from __future__ import annotations

import json
import os
import time
import urllib.error
import urllib.request

DEFAULT_API_BASE = "https://openrouter.ai/api"


def default_api_base() -> str:
    return (os.environ.get("BENCH_OPENROUTER_API_BASE")
            or os.environ.get("BENCH_ANTHROPIC_BASE_URL")
            or DEFAULT_API_BASE).rstrip("/")


def urllib_get(url: str, headers: dict, timeout: float = 20) -> tuple[int, bytes]:
    req = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read() if e.fp else b""
    except (urllib.error.URLError, TimeoutError, OSError):
        return 0, b""


class OpenRouterCost:
    def __init__(self, api_base: str | None = None, api_key: str | None = None,
                 http_get=urllib_get, sleep=time.sleep, retries: int = 5,
                 backoff_s: float = 1.0, enabled: bool = True):
        self.api_base = (api_base or default_api_base()).rstrip("/")
        self.api_key = api_key if api_key is not None else os.environ.get("OPENROUTER_API_KEY")
        self.http_get = http_get
        self.sleep = sleep
        self.retries = retries
        self.backoff_s = backoff_s
        self.enabled = enabled

    @classmethod
    def disabled(cls) -> "OpenRouterCost":
        return cls(enabled=False, api_key="")

    def _headers(self) -> dict:
        h = {"Accept": "application/json"}
        if self.api_key:
            h["Authorization"] = f"Bearer {self.api_key}"
        return h

    def _get_json(self, path: str, retries: int) -> dict | None:
        delay = self.backoff_s
        for attempt in range(retries + 1):
            status, body = self.http_get(self.api_base + path, self._headers())
            if status == 200:
                try:
                    return json.loads(body)
                except ValueError:
                    return None
            if status in (400, 401, 403):
                return None  # won't get better by waiting
            if attempt < retries:
                self.sleep(delay)
                delay = min(delay * 2, 8)
        return None

    def key_usage(self) -> float | None:
        if not self.enabled:
            return None
        data = self._get_json("/v1/key", retries=2)
        try:
            return float(data["data"]["usage"])
        except (TypeError, KeyError, ValueError):
            return None

    def generation_cost(self, gen_id: str, retries: int | None = None) -> float | None:
        data = self._get_json(f"/v1/generation?id={urllib.request.quote(gen_id)}",
                              self.retries if retries is None else retries)
        try:
            return float(data["data"]["total_cost"])
        except (TypeError, KeyError, ValueError):
            return None

    def run_cost(self, generation_ids: list[str], key_usage_before: float | None,
                 cc_cost_usd: float | None, subagents_spawned: int | None = None) -> dict:
        """Pick the best available cost for a finished run. Returns a dict for the run record."""
        out = {"cost_usd": cc_cost_usd, "cost_source": "claude_code_estimate",
               "generation_cost_usd": None, "generation_ids": len(generation_ids),
               "generation_missing": None, "key_delta_usd": None}
        if not self.enabled:
            return out
        ids = [g for g in generation_ids if isinstance(g, str) and g.startswith("gen-")]
        total, missing = 0.0, 0
        if ids:
            first = self.generation_cost(ids[0])
            if first is None:
                missing = len(ids)  # ids not resolvable at all: don't spend retries on each
            else:
                total += first
                for gid in ids[1:]:
                    c = self.generation_cost(gid)
                    if c is None:
                        missing += 1
                    else:
                        total += c
            out["generation_missing"] = missing
            if missing < len(ids):
                out["generation_cost_usd"] = round(total, 6)
        if key_usage_before is not None:
            after = self.key_usage()
            if after is not None:
                out["key_delta_usd"] = round(after - key_usage_before, 6)
        # Subagent turns may not appear in the transcript, so their generations can't be
        # summed; the key delta covers them.
        complete = bool(ids) and missing == 0 and len(ids) == len(generation_ids) and not subagents_spawned
        if out["generation_cost_usd"] is not None and complete:
            out["cost_usd"], out["cost_source"] = out["generation_cost_usd"], "openrouter_generation"
        elif out["key_delta_usd"] is not None:
            out["cost_usd"], out["cost_source"] = out["key_delta_usd"], "openrouter_key_delta"
        elif out["generation_cost_usd"] is not None:
            out["cost_usd"], out["cost_source"] = out["generation_cost_usd"], "openrouter_generation_partial"
        return out
