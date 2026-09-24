"""Aggregating runs.jsonl into summary.json, report.md and index.html.

Two outputs:
  <out>/summary.json, report.md, index.html      private: includes prompts, gold, answers
  <out>/public/summary.json, public/index.html   publishable: ids, kinds and metrics only

The public summary is built from an explicit whitelist of fields, never by deleting
fields from the private one.
"""
from __future__ import annotations

import html
import json
import math
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

from .store import Store, latest_ok

SCHEMA_VERSION = 1


def _stats(values: list) -> dict:
    vals = [v for v in values if isinstance(v, (int, float)) and not isinstance(v, bool)]
    if not vals:
        return {"mean": None, "min": None, "max": None}
    return {"mean": round(sum(vals) / len(vals), 4), "min": min(vals), "max": max(vals)}


def _mean(values: list):
    vals = [v for v in values if isinstance(v, (int, float)) and not isinstance(v, bool)]
    return sum(vals) / len(vals) if vals else None


def _ratio(a, b):
    return round(a / b, 4) if a is not None and b not in (None, 0) else None


def _total_tokens(r):
    return (r.get("tokens") or {}).get("total")


def _duration_s(r):
    if r.get("duration_ms") is not None:
        return r["duration_ms"] / 1000
    return r.get("wall_s")


def _tool(r, key):
    t = r.get("tools") or {}
    if key == "total":
        return t.get("total")
    return (t.get("bash") or {}).get(key)


def _variant_block(rs: list[dict]) -> dict:
    n = len(rs)
    passes = [r for r in rs if r.get("pass")]
    tokens_sum = sum(_total_tokens(r) or 0 for r in rs)
    return {
        "n": n,
        "pass_rate": round(len(passes) / n, 4) if n else None,
        "mean_total_tokens": _mean([_total_tokens(r) for r in rs]),
        "mean_cost_usd": _mean([r.get("cost_usd") for r in rs]),
        "total_cost_usd": round(sum(float(r.get("cost_usd") or 0) for r in rs), 4),
        "tokens_per_pass": round(tokens_sum / len(passes), 1) if passes else None,
        "smartgrep_use_share": round(sum(1 for r in rs if r.get("used_smartgrep")) / n, 4) if n else None,
    }


def aggregate(records: list[dict]) -> dict:
    """Build the full (private) summary from raw run records."""
    all_n = len(records)
    infra = sum(1 for r in records if r.get("status") == "infra_error")
    runs = latest_ok(records)
    cells: dict[tuple, list] = defaultdict(list)
    for r in runs:
        cells[(r["model"], r["task_id"], r["variant"])].append(r)

    cell_rows = []
    for (model, task_id, variant), rs in sorted(cells.items()):
        first = rs[0]
        cell_rows.append({
            "model": model, "task_id": task_id, "repo_id": first.get("repo_id"),
            "kind": first.get("task_kind"), "variant": variant, "n": len(rs),
            "pass_rate": round(sum(1 for r in rs if r.get("pass")) / len(rs), 4),
            "total_tokens": _stats([_total_tokens(r) for r in rs]),
            "input_tokens": _stats([(r.get("tokens") or {}).get("input") for r in rs]),
            "output_tokens": _stats([(r.get("tokens") or {}).get("output") for r in rs]),
            "cache_read_tokens": _stats([(r.get("tokens") or {}).get("cache_read") for r in rs]),
            "cache_creation_tokens": _stats([(r.get("tokens") or {}).get("cache_creation") for r in rs]),
            "cost_usd": _stats([r.get("cost_usd") for r in rs]),
            "turns": _stats([r.get("num_turns") for r in rs]),
            "duration_s": _stats([_duration_s(r) for r in rs]),
            "tool_calls": _stats([_tool(r, "total") for r in rs]),
            "bash_smartgrep": _stats([_tool(r, "smartgrep") for r in rs]),
            "bash_grep": _stats([_tool(r, "grep") for r in rs]),
            "bash_find": _stats([_tool(r, "find") for r in rs]),
            "bash_read": _stats([_tool(r, "read") for r in rs]),
            "read_tool": _stats([((r.get("tools") or {}).get("by_tool") or {}).get("Read", 0) for r in rs]),
            "subagent_calls": _stats([(r.get("tools") or {}).get("subagent_calls") for r in rs]),
            "used_smartgrep_share": round(sum(1 for r in rs if r.get("used_smartgrep")) / len(rs), 4),
            "f1": _stats([(r.get("grade") or {}).get("f1") for r in rs]),
            "cost_sources": sorted({r.get("cost_source") or "none" for r in rs}),
        })

    models = []
    for model in sorted({r["model"] for r in runs}):
        mr = [r for r in runs if r["model"] == model]
        a = [r for r in mr if r["variant"] == "A"]
        b = [r for r in mr if r["variant"] == "B"]
        va, vb = _variant_block(a), _variant_block(b)
        # per-task A/B ratio of mean tokens, then geometric mean over tasks (each task
        # weighs the same, unlike the ratio of overall means which big tasks dominate)
        logs = []
        for tid in sorted({r["task_id"] for r in mr}):
            ta = _mean([_total_tokens(r) for r in a if r["task_id"] == tid])
            tb = _mean([_total_tokens(r) for r in b if r["task_id"] == tid])
            if ta and tb:
                logs.append(math.log(ta / tb))
        models.append({
            "model": model,
            "variants": {"A": va, "B": vb},
            "token_ratio_a_over_b": _ratio(va["mean_total_tokens"], vb["mean_total_tokens"]),
            "cost_ratio_a_over_b": _ratio(va["mean_cost_usd"], vb["mean_cost_usd"]),
            "task_geomean_token_ratio_a_over_b": round(math.exp(sum(logs) / len(logs)), 4) if logs else None,
            "tasks_compared": len(logs),
        })

    tasks = {}
    for r in runs:
        tasks.setdefault(r["task_id"], {"task_id": r["task_id"], "repo_id": r.get("repo_id"),
                                        "kind": r.get("task_kind"), "task_hash": r.get("task_hash")})

    def uniq(key):
        return sorted({str(r.get(key)) for r in runs if r.get(key) is not None})

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "runs": {"records": all_n, "ok": len(runs), "infra_error_records": infra},
        "meta": {
            "agents": uniq("agent"), "agent_versions": uniq("agent_version"),
            "efforts": uniq("effort"), "smartgrep_versions": uniq("smartgrep_version"),
            "smartgrep_git_shas": uniq("smartgrep_git_sha"),
            "smartgrep_binary_sha256": uniq("smartgrep_binary_sha256"),
            "tasks_repo_shas": uniq("tasks_repo_sha"),
            "cost_sources": uniq("cost_source"),
            "repo_shas": {r["repo_id"]: r.get("repo_sha") for r in runs if r.get("repo_id")},
        },
        "models": models,
        "tasks": sorted(tasks.values(), key=lambda t: t["task_id"]),
        "cells": cell_rows,
    }


# Whitelists for the public summary (metrics and identifiers only).
PUBLIC_META = ("agents", "agent_versions", "efforts", "smartgrep_versions", "smartgrep_git_shas",
               "cost_sources", "repo_shas")
PUBLIC_CELL = ("model", "task_id", "repo_id", "kind", "variant", "n", "pass_rate", "total_tokens",
               "input_tokens", "output_tokens", "cache_read_tokens", "cache_creation_tokens",
               "cost_usd", "turns", "duration_s", "tool_calls", "bash_smartgrep", "bash_grep",
               "bash_find", "bash_read", "read_tool", "subagent_calls", "used_smartgrep_share", "f1")
PUBLIC_TASK = ("task_id", "repo_id", "kind")


def public_summary(summary: dict) -> dict:
    return {
        "schema_version": summary["schema_version"],
        "generated_at": summary["generated_at"],
        "runs": {k: summary["runs"][k] for k in ("ok", "infra_error_records")},
        "meta": {k: summary["meta"][k] for k in PUBLIC_META},
        "models": json.loads(json.dumps(summary["models"])),  # numbers + model ids only
        "tasks": [{k: t[k] for k in PUBLIC_TASK} for t in summary["tasks"]],
        "cells": [{k: c[k] for k in PUBLIC_CELL} for c in summary["cells"]],
    }


def private_details(records: list[dict], taskset) -> dict:
    """Per-task prompt/gold (if the task set is available) and per-run answers."""
    runs = latest_ok(records)
    details: dict[str, dict] = {}
    for r in sorted(runs, key=lambda r: (r["task_id"], r["model"], r["variant"], r.get("repeat", 0))):
        d = details.setdefault(r["task_id"], {"prompt": None, "gold": None, "runs": []})
        g = r.get("grade") or {}
        d["runs"].append({"model": r["model"], "variant": r["variant"], "repeat": r.get("repeat"),
                          "pass": r.get("pass"), "f1": g.get("f1"), "answer": g.get("answer"),
                          "missed": g.get("missed"), "false_positives": g.get("false_positives"),
                          "grade_error": g.get("grade_error"), "run_key": r["run_key"]})
    if taskset is not None:
        for tid, d in details.items():
            try:
                t = taskset.task(tid)
            except Exception:
                continue
            d["prompt"] = t.prompt
            d["gold"] = t.gold
            d["test_cmd"] = t.test_cmd
    return details


# ---------------------------------------------------------------- rendering -----------

def _fmt(v, kind="num"):
    if v is None:
        return "–"
    if kind == "usd":
        return f"${v:.4f}" if v < 1 else f"${v:.2f}"
    if kind == "pct":
        return f"{v * 100:.0f}%"
    if kind == "ratio":
        return f"{v:.2f}×"
    if isinstance(v, float):
        return f"{v:,.1f}" if abs(v) < 1000 else f"{v:,.0f}"
    return f"{v:,}" if isinstance(v, int) else str(v)


def _headline_rows(summary):
    rows = []
    for m in summary["models"]:
        a, b = m["variants"]["A"], m["variants"]["B"]
        rows.append([m["model"], _fmt(m["token_ratio_a_over_b"], "ratio"),
                     _fmt(m["task_geomean_token_ratio_a_over_b"], "ratio"),
                     _fmt(m["cost_ratio_a_over_b"], "ratio"),
                     f"{_fmt(a['pass_rate'], 'pct')} / {_fmt(b['pass_rate'], 'pct')}",
                     f"{_fmt(a['tokens_per_pass'])} / {_fmt(b['tokens_per_pass'])}",
                     _fmt(a["smartgrep_use_share"], "pct"), f"{a['n']} / {b['n']}",
                     f"{_fmt(a['total_cost_usd'], 'usd')} / {_fmt(b['total_cost_usd'], 'usd')}"])
    return rows


HEADLINE_COLS = ["model", "tokens A/B", "per-task geomean A/B", "cost A/B", "pass A / B",
                 "tokens per pass A / B", "A runs using smartgrep", "n A / B", "spent A / B"]
CELL_COLS = ["model", "task", "repo", "kind", "var", "n", "pass", "tokens mean (min–max)",
             "cost mean", "turns", "time s", "tool calls", "bash sg/grep/find/read", "Read"]


def _cell_rows(summary):
    rows = []
    for c in summary["cells"]:
        t = c["total_tokens"]
        rows.append([c["model"], c["task_id"], c["repo_id"], c["kind"], c["variant"], c["n"],
                     _fmt(c["pass_rate"], "pct"),
                     f"{_fmt(t['mean'])} ({_fmt(t['min'])}–{_fmt(t['max'])})",
                     _fmt(c["cost_usd"]["mean"], "usd"), _fmt(c["turns"]["mean"]),
                     _fmt(c["duration_s"]["mean"]), _fmt(c["tool_calls"]["mean"]),
                     "/".join(_fmt(c[k]["mean"]) for k in ("bash_smartgrep", "bash_grep", "bash_find", "bash_read")),
                     _fmt(c["read_tool"]["mean"])])
    return rows


def render_md(summary: dict, details: dict | None) -> str:
    def table(cols, rows):
        out = ["| " + " | ".join(cols) + " |", "|" + "---|" * len(cols)]
        out += ["| " + " | ".join(str(x).replace("|", "\\|") for x in r) + " |" for r in rows]
        return "\n".join(out)
    meta = summary["meta"]
    lines = [
        "# smartgrep token benchmark", "",
        f"Generated {summary['generated_at']} · {summary['runs']['ok']} graded runs "
        f"({summary['runs']['infra_error_records']} infra-error attempts excluded)", "",
        f"- agent: {', '.join(meta['agents'])} {', '.join(meta['agent_versions'])}; effort: {', '.join(meta['efforts'])}",
        f"- smartgrep: {', '.join(meta['smartgrep_versions']) or 'version n/a'} @ {', '.join(s[:10] for s in meta['smartgrep_git_shas'])}",
        f"- cost sources: {', '.join(meta['cost_sources'])}", "",
        "A = smartgrep on PATH + `smartgrep init`; B = same repo without smartgrep. "
        "Ratios < 1 mean A used less. Tokens = input + cache write + cache read + output.", "",
        "## Headline", "", table(HEADLINE_COLS, _headline_rows(summary)), "",
        "## Per task", "", table(CELL_COLS, _cell_rows(summary)), "",
    ]
    if details:
        lines += ["## Task details (private)", ""]
        for tid, d in sorted(details.items()):
            lines += [f"### {tid}", ""]
            if d.get("prompt"):
                lines += ["Prompt:", "", "> " + d["prompt"].replace("\n", "\n> "), ""]
            if d.get("gold") is not None:
                lines += [f"Gold: `{json.dumps(d['gold'])}`", ""]
            rows = [[r["model"], r["variant"], r["repeat"], r["pass"], _fmt(r["f1"]),
                     json.dumps(r["answer"]) if r["answer"] is not None else (r["grade_error"] or "–")]
                    for r in d["runs"]]
            lines += [table(["model", "var", "rep", "pass", "F1", "answer"], rows), ""]
    return "\n".join(lines)


CSS = """
:root{--bg:#fbfbfa;--fg:#1d1d1b;--muted:#6b6b66;--line:#e3e2dd;--head:#f1f0ec;--accent:#2f6f4f;--bad:#a33b2b}
@media (prefers-color-scheme: dark){:root{--bg:#141413;--fg:#ecebe6;--muted:#a09f99;--line:#2e2d2a;--head:#1e1e1c;--accent:#7fc59e;--bad:#e08a7a}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif}
main{max-width:1200px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:17px;margin:32px 0 8px}h3{font-size:15px;margin:24px 0 6px}
p,.meta{color:var(--muted)}
.scroll{overflow-x:auto;border:1px solid var(--line);border-radius:6px}
table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}
th,td{padding:6px 10px;border-bottom:1px solid var(--line);text-align:left;white-space:nowrap}
th{background:var(--head);font-weight:600;font-size:12px;color:var(--muted)}
tr:last-child td{border-bottom:none}
td.wrap{white-space:normal;max-width:520px;word-break:break-word}
code,pre{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:12px}
pre{white-space:pre-wrap;background:var(--head);padding:8px 10px;border-radius:6px}
.good{color:var(--accent)}.bad{color:var(--bad)}
"""


def _html_table(cols, rows, wrap_last=False):
    h = ["<div class='scroll'><table><thead><tr>"]
    h += [f"<th>{html.escape(str(c))}</th>" for c in cols]
    h.append("</tr></thead><tbody>")
    for r in rows:
        h.append("<tr>")
        for i, x in enumerate(r):
            cls = " class='wrap'" if wrap_last and i == len(r) - 1 else ""
            h.append(f"<td{cls}>{html.escape(str(x))}</td>")
        h.append("</tr>")
    h.append("</tbody></table></div>")
    return "".join(h)


def render_html(summary: dict, details: dict | None, title: str) -> str:
    meta = summary["meta"]
    parts = [
        "<!doctype html><html lang='en'><head><meta charset='utf-8'>",
        "<meta name='viewport' content='width=device-width,initial-scale=1'>",
        f"<title>{html.escape(title)}</title><style>{CSS}</style></head><body><main>",
        f"<h1>{html.escape(title)}</h1>",
        f"<div class='meta'>Generated {html.escape(summary['generated_at'])} · "
        f"{summary['runs']['ok']} graded runs · {summary['runs']['infra_error_records']} infra-error attempts excluded<br>"
        f"agent {html.escape(', '.join(meta['agents']))} {html.escape(', '.join(meta['agent_versions']))} · "
        f"effort {html.escape(', '.join(meta['efforts']))} · smartgrep "
        f"{html.escape(', '.join(meta['smartgrep_versions']) or 'n/a')} @ "
        f"{html.escape(', '.join(s[:10] for s in meta['smartgrep_git_shas']))} · "
        f"cost from {html.escape(', '.join(meta['cost_sources']))}</div>",
        "<p>A = smartgrep on PATH + <code>smartgrep init</code>; B = the same repository without smartgrep. "
        "Same agent, model, prompt and effort. Ratios below 1 mean A used less. "
        "Tokens = input + cache write + cache read + output.</p>",
        "<h2>Headline</h2>", _html_table(HEADLINE_COLS, _headline_rows(summary)),
        "<h2>Per task</h2>", _html_table(CELL_COLS, _cell_rows(summary)),
    ]
    if details:
        parts.append("<h2>Task details (private)</h2>")
        for tid, d in sorted(details.items()):
            parts.append(f"<h3>{html.escape(tid)}</h3>")
            if d.get("prompt"):
                parts.append(f"<pre>{html.escape(d['prompt'])}</pre>")
            if d.get("gold") is not None:
                parts.append(f"<p>Gold: <code>{html.escape(json.dumps(d['gold']))}</code></p>")
            rows = [[r["model"], r["variant"], r["repeat"], r["pass"], _fmt(r["f1"]),
                     json.dumps(r["answer"]) if r["answer"] is not None else (r["grade_error"] or "–")]
                    for r in d["runs"]]
            parts.append(_html_table(["model", "var", "rep", "pass", "F1", "answer"], rows, wrap_last=True))
    parts.append("</main></body></html>")
    return "".join(parts)


def write_report(out: Path, taskset=None) -> dict:
    records = Store(out).records()
    if not records:
        raise FileNotFoundError(f"no runs recorded in {out / 'runs.jsonl'}")
    summary = aggregate(records)
    details = private_details(records, taskset)
    private = dict(summary, task_details=details)
    (out / "summary.json").write_text(json.dumps(private, indent=2, ensure_ascii=False) + "\n")
    (out / "report.md").write_text(render_md(summary, details) + "\n")
    (out / "index.html").write_text(render_html(summary, details, "smartgrep token benchmark (private)"))
    pub = public_summary(summary)
    pub_dir = out / "public"
    pub_dir.mkdir(exist_ok=True)
    (pub_dir / "summary.json").write_text(json.dumps(pub, indent=2, ensure_ascii=False) + "\n")
    (pub_dir / "index.html").write_text(render_html(pub, None, "smartgrep token benchmark"))
    return summary
