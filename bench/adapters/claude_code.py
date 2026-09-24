"""Claude Code adapter: ``claude -p ... --output-format stream-json`` via OpenRouter.

Shape of the stream (one JSON object per line), as observed with Claude Code 2.1.x:
  {"type":"system","subtype":"init", "tools":[...], "model":..., "claude_code_version":...}
  {"type":"system","subtype":"api_retry","attempt":n,"error_status":429|null,"error":"rate_limit"}
  {"type":"assistant","message":{"id":"gen-...","model":...,"content":[{"type":"tool_use",...}],
   "usage":{...}}, "parent_tool_use_id": null|"toolu_..."}
  {"type":"user","message":{"content":[{"type":"tool_result",...}]}}
  {"type":"result","subtype":"success","is_error":false,"terminal_reason":"completed",
   "api_error_status":null,"num_turns":2,"duration_ms":..., "total_cost_usd":...,
   "usage":{...}, "modelUsage":{"<model>":{"inputTokens":..,"outputTokens":..,
   "cacheReadInputTokens":..,"cacheCreationInputTokens":..,"costUSD":..}},
   "subagent_stats":{"spawned":n,...}, "result":"<final text>"}
Note: on API errors ``subtype`` is still "success"; ``is_error``/``terminal_reason``/
``api_error_status`` carry the failure. The parser tolerates any missing field.
"""
from __future__ import annotations

import json
import os
import subprocess
import time
from pathlib import Path

from adapters.base import Agent, AgentResult
from lib.bashclass import CATEGORIES, classify
from lib.proc import run_killable

DEFAULT_BASE_URL = "https://openrouter.ai/api"
SUBAGENT_TOOLS = {"Task", "Agent"}


def _int(v):
    return v if isinstance(v, int) and not isinstance(v, bool) else None


def _sum_tokens(parts: list[dict]) -> dict:
    t = {"input": 0, "output": 0, "cache_creation": 0, "cache_read": 0}
    for p in parts:
        for k in t:
            t[k] += p.get(k) or 0
    t["total"] = t["input"] + t["output"] + t["cache_creation"] + t["cache_read"]
    return t


def parse_stream(lines) -> dict:
    """Parse Claude Code stream-json lines into run metrics. Never raises on bad input."""
    m: dict = {
        "result_found": False, "result_text": None, "is_error": None, "subtype": None,
        "terminal_reason": None, "api_error_status": None, "num_turns": None,
        "duration_ms": None, "cc_cost_usd": None, "tokens": None, "tokens_source": None,
        "models_used": [], "generation_ids": [], "session_id": None, "init_tools": None,
        "agent_reported_version": None, "api_retries": 0, "last_retry_status": None,
        "parse_errors": 0, "subagents_spawned": None,
    }
    by_tool: dict[str, int] = {}
    bash = {c: 0 for c in CATEGORIES}
    tool_ids: set[str] = set()
    msg_usage: dict[str, dict] = {}
    gen_ids: list[str] = []
    sidechain_calls = 0
    bash_commands = 0
    bash_samples: list[str] = []
    result = None

    for raw in lines:
        raw = raw.strip()
        if not raw:
            continue
        try:
            ev = json.loads(raw)
        except ValueError:
            m["parse_errors"] += 1
            continue
        if not isinstance(ev, dict):
            continue
        typ = ev.get("type")
        if typ == "system":
            if ev.get("subtype") == "init":
                m["init_tools"] = ev.get("tools")
                m["agent_reported_version"] = ev.get("claude_code_version")
                m["session_id"] = ev.get("session_id")
            elif ev.get("subtype") == "api_retry":
                m["api_retries"] += 1
                m["last_retry_status"] = ev.get("error_status")
        elif typ == "assistant":
            msg = ev.get("message") or {}
            mid = msg.get("id")
            model = msg.get("model")
            synthetic = model == "<synthetic>"
            if isinstance(mid, str) and not synthetic:
                if mid not in gen_ids:
                    gen_ids.append(mid)
                if isinstance(msg.get("usage"), dict):
                    msg_usage[mid] = msg["usage"]
            if model and not synthetic and model not in m["models_used"]:
                m["models_used"].append(model)
            for block in msg.get("content") or []:
                if not isinstance(block, dict) or block.get("type") != "tool_use":
                    continue
                tid = block.get("id")
                if tid in tool_ids:
                    continue
                if tid:
                    tool_ids.add(tid)
                name = block.get("name") or "?"
                by_tool[name] = by_tool.get(name, 0) + 1
                if ev.get("parent_tool_use_id"):
                    sidechain_calls += 1
                if name == "Bash":
                    cmd = (block.get("input") or {}).get("command") or ""
                    bash_commands += 1
                    for cat in classify(cmd):
                        bash[cat] += 1
                    if len(bash_samples) < 200:
                        bash_samples.append(cmd[:300])
        elif typ == "result":
            result = ev

    if result is not None:
        m["result_found"] = True
        m["result_text"] = result.get("result") if isinstance(result.get("result"), str) else None
        m["is_error"] = result.get("is_error")
        m["subtype"] = result.get("subtype")
        m["terminal_reason"] = result.get("terminal_reason")
        m["api_error_status"] = result.get("api_error_status")
        m["num_turns"] = _int(result.get("num_turns"))
        m["duration_ms"] = _int(result.get("duration_ms"))
        cost = result.get("total_cost_usd")
        m["cc_cost_usd"] = float(cost) if isinstance(cost, (int, float)) else None
        ss = result.get("subagent_stats")
        if isinstance(ss, dict):
            m["subagents_spawned"] = _int(ss.get("spawned"))
        mu = result.get("modelUsage")
        u = result.get("usage")
        if isinstance(mu, dict) and mu:
            m["tokens"] = _sum_tokens([{
                "input": v.get("inputTokens"), "output": v.get("outputTokens"),
                "cache_creation": v.get("cacheCreationInputTokens"),
                "cache_read": v.get("cacheReadInputTokens")} for v in mu.values() if isinstance(v, dict)])
            m["tokens_source"] = "modelUsage"
            for name in mu:
                if name not in m["models_used"]:
                    m["models_used"].append(name)
        elif isinstance(u, dict):
            m["tokens"] = _sum_tokens([{
                "input": u.get("input_tokens"), "output": u.get("output_tokens"),
                "cache_creation": u.get("cache_creation_input_tokens"),
                "cache_read": u.get("cache_read_input_tokens")}])
            m["tokens_source"] = "usage"
    if m["tokens"] is None and msg_usage:
        # No final result (killed / crashed): best effort from per-message usage. Output
        # tokens here are often partial (stream start), so this is marked as approximate.
        m["tokens"] = _sum_tokens([{
            "input": u.get("input_tokens"), "output": u.get("output_tokens"),
            "cache_creation": u.get("cache_creation_input_tokens"),
            "cache_read": u.get("cache_read_input_tokens")} for u in msg_usage.values()])
        m["tokens_source"] = "assistant_events_approx"

    m["generation_ids"] = gen_ids
    m["tools"] = {
        "total": sum(by_tool.values()),
        "by_tool": dict(sorted(by_tool.items())),
        "bash_commands": bash_commands,
        "bash": bash,
        "subagent_calls": sum(by_tool.get(t, 0) for t in SUBAGENT_TOOLS),
        "sidechain_calls": sidechain_calls,
        "bash_samples": bash_samples,
    }
    m["smartgrep_calls"] = bash["smartgrep"]
    return m


def classify_outcome(m: dict, exit_code: int | None, timed_out: bool) -> tuple[str, str | None, bool]:
    """(status, error, fatal). API/infra failures are 'infra_error'; model misbehaviour is 'ok'."""
    if timed_out:
        return "infra_error", "timeout", False
    if not m.get("result_found"):
        extra = f", last retry status {m['last_retry_status']}" if m.get("api_retries") else ""
        return "infra_error", f"no result event (exit {exit_code}{extra})", False
    status = m.get("api_error_status")
    if status is not None or m.get("terminal_reason") == "api_error":
        fatal = status in (401, 403)
        return "infra_error", f"api error {status}: {(m.get('result_text') or '')[:200]}", fatal
    return "ok", None, False


class ClaudeCodeAgent(Agent):
    name = "claude_code"

    def __init__(self, binary: str = "claude", extra_args: list[str] | None = None):
        self.binary = binary
        self.extra_args = extra_args or []

    def version(self, env: dict) -> str | None:
        try:
            out = subprocess.run([self.binary, "--version"], capture_output=True, text=True,
                                 env=env, timeout=30, stdin=subprocess.DEVNULL)
        except (OSError, subprocess.TimeoutExpired):
            return None
        if out.returncode != 0:
            return None
        return out.stdout.strip() or None

    def build_env(self, base_env: dict, model: str) -> dict:
        env = dict(base_env)
        env["ANTHROPIC_BASE_URL"] = os.environ.get("BENCH_ANTHROPIC_BASE_URL") or DEFAULT_BASE_URL
        # With an auth-injecting proxy the key may be absent; Claude Code still wants a token.
        env["ANTHROPIC_AUTH_TOKEN"] = os.environ.get("OPENROUTER_API_KEY") or "proxy-injected"
        env["ANTHROPIC_API_KEY"] = ""
        env["DISABLE_AUTOUPDATER"] = "1"
        env["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"] = "1"
        # Every model role (background/haiku tasks, subagents) uses the model under test, so
        # nothing is routed to a model name OpenRouter may not know and cost is single-model.
        for var in ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL",
                    "ANTHROPIC_DEFAULT_OPUS_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL"):
            env[var] = model
        return env

    def command(self, prompt: str, model: str, effort: str | None, max_turns: int,
                max_budget_usd: float | None) -> list[str]:
        cmd = [self.binary, "-p", prompt, "--model", model,
               "--output-format", "stream-json", "--verbose",
               "--setting-sources", "project",
               "--permission-mode", "bypassPermissions",
               "--disallowedTools", "WebFetch,WebSearch",
               "--max-turns", str(max_turns),
               "--no-session-persistence"]
        if effort:
            cmd += ["--effort", effort]
        if max_budget_usd is not None:
            cmd += ["--max-budget-usd", f"{max_budget_usd:g}"]
        return cmd + self.extra_args

    def run(self, prompt, workdir, env, model, effort, max_turns, timeout_s, max_budget_usd,
            transcript_path, stderr_path) -> AgentResult:
        transcript_path.parent.mkdir(parents=True, exist_ok=True)
        stderr_path.parent.mkdir(parents=True, exist_ok=True)
        cmd = self.command(prompt, model, effort, max_turns, max_budget_usd)
        t0 = time.monotonic()
        with open(transcript_path, "wb") as out, open(stderr_path, "wb") as err:
            code, timed_out = run_killable(cmd, cwd=workdir, env=env, timeout=timeout_s,
                                           stdout=out, stderr=err)
        wall = time.monotonic() - t0
        with open(transcript_path, encoding="utf-8", errors="replace") as f:
            metrics = parse_stream(f)
        status, error, fatal = classify_outcome(metrics, code, timed_out)
        return AgentResult(exit_code=code, timed_out=timed_out, wall_s=round(wall, 2),
                           transcript_path=transcript_path, stderr_path=stderr_path,
                           metrics=metrics, status=status, error=error, fatal=fatal)
