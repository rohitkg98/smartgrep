"""Agent adapter interface. One adapter per coding agent (Claude Code, ...)."""
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class AgentResult:
    """What an adapter reports about one headless agent run.

    ``metrics`` is the adapter's parsed view of the transcript with (at least) these keys,
    any of which may be None when the agent did not report it:
      result_found, result_text, is_error, subtype, terminal_reason, api_error_status,
      num_turns, duration_ms, cc_cost_usd, tokens{input,output,cache_creation,cache_read,total},
      tokens_source, models_used, generation_ids, tools{...}, api_retries
    """
    exit_code: int | None
    timed_out: bool
    wall_s: float
    transcript_path: Path
    stderr_path: Path
    metrics: dict = field(default_factory=dict)
    # "ok" = the agent finished (graded even if the model misbehaved);
    # "infra_error" = rate limit / HTTP error / timeout / crash -> retried later.
    status: str = "ok"
    error: str | None = None
    fatal: bool = False  # e.g. auth failure: stop the whole invocation


class Agent:
    name = "base"

    def version(self, env: dict) -> str | None:
        raise NotImplementedError

    def run(self, prompt: str, workdir: Path, env: dict, model: str, effort: str | None,
            max_turns: int, timeout_s: float, max_budget_usd: float | None,
            transcript_path: Path, stderr_path: Path) -> AgentResult:
        raise NotImplementedError

    def build_env(self, base_env: dict, model: str) -> dict:
        """Add agent/provider-specific variables (API endpoint, auth) to ``base_env``."""
        return dict(base_env)
