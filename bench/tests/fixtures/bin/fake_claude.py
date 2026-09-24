#!/usr/bin/env python3
"""Stand-in for `claude` in end-to-end tests. Emits canned stream-json, no network.

Checks the invocation the harness makes: flags, isolated HOME, and PATH per variant
(variant A is recognised by the CLAUDE.md that `smartgrep init` writes). A violated
check exits 3 without a result event, which the harness records as an infra error.
Model names containing "ratelimit" simulate a rate-limited run that never finishes.
"""
import json
import os
import shutil
import subprocess
import sys


def fail(msg):
    sys.stderr.write("fake_claude check failed: " + msg + "\n")
    sys.exit(3)


def emit(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


args = sys.argv[1:]
if args == ["--version"]:
    print("9.9.9 (Fake Claude)")
    sys.exit(0)


def opt(name):
    if name not in args:
        fail("missing flag " + name)
    return args[args.index(name) + 1]


prompt = opt("-p")
model = opt("--model")
if opt("--output-format") != "stream-json" or "--verbose" not in args:
    fail("stream-json/verbose not requested")
if opt("--setting-sources") != "project":
    fail("setting sources")
if opt("--permission-mode") != "bypassPermissions":
    fail("permission mode")
if opt("--disallowedTools") != "WebFetch,WebSearch":
    fail("disallowed tools")
int(opt("--max-turns"))
float(opt("--max-budget-usd"))
if os.environ.get("ANTHROPIC_API_KEY") != "" or not os.environ.get("ANTHROPIC_AUTH_TOKEN"):
    fail("auth env")
home = os.environ.get("HOME", "")
if not os.path.isdir(home) or os.listdir(home):
    fail("HOME is not a fresh empty dir: " + home)

cwd = os.getcwd()
variant_a = os.path.exists(os.path.join(cwd, "CLAUDE.md")) and "smartgrep" in open(os.path.join(cwd, "CLAUDE.md")).read()
sg = shutil.which("smartgrep")
if variant_a and not sg:
    fail("variant A but smartgrep is not on PATH")
if not variant_a and sg:
    fail("variant B but smartgrep resolves to " + sg)

emit({"type": "system", "subtype": "init", "cwd": cwd, "session_id": "s-1", "tools": ["Bash", "Read", "Edit"],
      "model": model, "claude_code_version": "9.9.9"})

if "ratelimit" in model:
    for i in range(3):
        emit({"type": "system", "subtype": "api_retry", "attempt": i + 1, "error_status": 429, "error": "rate_limit"})
    sys.exit(1)

if variant_a:
    cmd = "smartgrep ls classes"
    out = subprocess.run(["smartgrep", "ls", "classes"], capture_output=True, text=True).stdout
else:
    cmd = "grep -rn 'class ' . | head -20"
    out = subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout
emit({"type": "assistant", "message": {"id": "gen-fake-1", "model": model, "content": [
    {"type": "tool_use", "id": "toolu_1", "name": "Bash", "input": {"command": cmd}}],
    "usage": {"input_tokens": 10, "output_tokens": 1}}, "parent_tool_use_id": None})
emit({"type": "user", "message": {"role": "user", "content": [
    {"type": "tool_result", "tool_use_id": "toolu_1", "content": out}]}, "parent_tool_use_id": None})

if "Make the change" in prompt:
    with open(os.path.join(cwd, "pkg", "util.py"), "a") as f:
        f.write("\n\ndef double(x):\n    return 2 * x\n")
    text = "Added double()."
else:
    text = 'Found them.\nANSWER: ["pkg.shapes.Circle", "RoundedSquare"]'
emit({"type": "assistant", "message": {"id": "gen-fake-2", "model": model, "content": [
    {"type": "text", "text": text}], "usage": {"input_tokens": 10, "output_tokens": 1}}, "parent_tool_use_id": None})
scale = 1 if variant_a else 3
emit({"type": "result", "subtype": "success", "is_error": False, "terminal_reason": "completed",
      "api_error_status": None, "num_turns": 2, "duration_ms": 1234, "total_cost_usd": 0.01 * scale,
      "usage": {"input_tokens": 100 * scale, "output_tokens": 20, "cache_creation_input_tokens": 1000 * scale,
                "cache_read_input_tokens": 500 * scale},
      "modelUsage": {model: {"inputTokens": 100 * scale, "outputTokens": 20, "cacheReadInputTokens": 500 * scale,
                             "cacheCreationInputTokens": 1000 * scale, "costUSD": 0.01 * scale}},
      "subagent_stats": {"spawned": 0}, "result": text})
