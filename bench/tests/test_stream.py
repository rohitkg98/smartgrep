import unittest

import helpers  # noqa: F401  (sets sys.path)
from adapters.claude_code import ClaudeCodeAgent, classify_outcome, parse_stream
from helpers import STREAMS


def parse(name):
    with open(STREAMS / name) as f:
        return parse_stream(f)


class RealTranscripts(unittest.TestCase):
    """Fixtures captured from Claude Code 2.1.281 against a local fake API server."""

    def test_success(self):
        m = parse("real_success.jsonl")
        self.assertTrue(m["result_found"])
        self.assertEqual(m["tokens"], {"input": 200, "output": 40, "cache_creation": 100,
                                       "cache_read": 60, "total": 400})
        self.assertEqual(m["tokens_source"], "modelUsage")
        self.assertEqual(m["num_turns"], 2)
        self.assertEqual(m["generation_ids"], ["gen-1-fake", "gen-2-fake"])
        self.assertAlmostEqual(m["cc_cost_usd"], 0.002112)
        self.assertEqual(m["tools"]["by_tool"], {"Bash": 1})
        # "grep -rn 'class' . | head -5 && ls"
        self.assertEqual(m["tools"]["bash"]["grep"], 1)
        self.assertEqual(m["tools"]["bash"]["find"], 1)
        self.assertEqual(m["tools"]["bash"]["read"], 0)
        self.assertIn('ANSWER: ["m.B"]', m["result_text"])
        self.assertEqual(m["agent_reported_version"], "2.1.281")
        self.assertEqual(classify_outcome(m, 0, False), ("ok", None, False))

    def test_api_error_is_infra(self):
        m = parse("real_api_error_400.jsonl")
        self.assertTrue(m["is_error"])
        self.assertEqual(m["subtype"], "success")  # Claude Code says "success" even here
        self.assertEqual(m["generation_ids"], [])  # <synthetic> message ignored
        status, err, fatal = classify_outcome(m, 1, False)
        self.assertEqual(status, "infra_error")
        self.assertIn("400", err)
        self.assertFalse(fatal)

    def test_killed_while_rate_limited(self):
        m = parse("real_rate_limited_killed.jsonl")
        self.assertFalse(m["result_found"])
        self.assertEqual(m["api_retries"], 8)
        self.assertEqual(m["last_retry_status"], 429)
        self.assertIsNone(m["tokens"])
        status, err, _ = classify_outcome(m, None, True)
        self.assertEqual((status, err), ("infra_error", "timeout"))
        status, err, _ = classify_outcome(m, 1, False)
        self.assertEqual(status, "infra_error")
        self.assertIn("429", err)


class SyntheticTranscript(unittest.TestCase):
    def setUp(self):
        self.m = parse("synthetic_tools.jsonl")

    def test_tool_counts(self):
        t = self.m["tools"]
        self.assertEqual(t["total"], 8)  # duplicate emission of toolu_5 counted once
        self.assertEqual(t["by_tool"], {"Bash": 6, "Read": 1, "Task": 1})
        self.assertEqual(t["bash"], {"smartgrep": 1, "grep": 4, "find": 1, "read": 2, "other": 1})
        self.assertEqual(t["subagent_calls"], 1)
        self.assertEqual(t["sidechain_calls"], 1)
        self.assertEqual(self.m["smartgrep_calls"], 1)
        self.assertEqual(self.m["subagents_spawned"], 1)

    def test_tokens_summed_across_models(self):
        self.assertEqual(self.m["tokens"], {"input": 1100, "output": 350, "cache_creation": 5200,
                                            "cache_read": 20000, "total": 26650})
        self.assertIn("anthropic/claude-haiku", self.m["models_used"])

    def test_generation_ids(self):
        self.assertEqual(self.m["generation_ids"],
                         ["gen-a1", "gen-a2", "gen-a3", "gen-a4", "gen-a5", "gen-sub1", "gen-a6", "gen-a7"])
        self.assertEqual(self.m["parse_errors"], 1)

    def test_missing_fields_tolerated(self):
        m = parse_stream(['{"type":"result"}', '{"type":"assistant"}', '[]', ''])
        self.assertTrue(m["result_found"])
        self.assertIsNone(m["tokens"])
        self.assertIsNone(m["num_turns"])
        self.assertEqual(m["tools"]["total"], 0)

    def test_approx_tokens_without_result(self):
        m = parse_stream(['{"type":"assistant","message":{"id":"gen-1","usage":{"input_tokens":7,"output_tokens":2}}}'])
        self.assertEqual(m["tokens_source"], "assistant_events_approx")
        self.assertEqual(m["tokens"]["total"], 9)

    def test_auth_error_is_fatal(self):
        m = {"result_found": True, "api_error_status": 401, "terminal_reason": "api_error", "result_text": "x"}
        self.assertEqual(classify_outcome(m, 1, False)[::2], ("infra_error", True))


class Command(unittest.TestCase):
    def test_command_flags(self):
        cmd = ClaudeCodeAgent("claude").command("do it", "anthropic/claude-opus-5.5", "high", 60, 2.0)
        self.assertEqual(cmd[:3], ["claude", "-p", "do it"])
        for flag, val in (("--model", "anthropic/claude-opus-5.5"), ("--output-format", "stream-json"),
                          ("--setting-sources", "project"), ("--permission-mode", "bypassPermissions"),
                          ("--disallowedTools", "WebFetch,WebSearch"), ("--max-turns", "60"),
                          ("--effort", "high"), ("--max-budget-usd", "2")):
            self.assertEqual(cmd[cmd.index(flag) + 1], val, flag)
        self.assertIn("--verbose", cmd)

    def test_env(self):
        import os
        from unittest import mock
        with mock.patch.dict(os.environ, {"OPENROUTER_API_KEY": "sk-test", "BENCH_ANTHROPIC_BASE_URL": "http://proxy/api"}):
            env = ClaudeCodeAgent().build_env({"HOME": "/h", "PATH": "/usr/bin"}, "m/x")
        self.assertEqual(env["ANTHROPIC_BASE_URL"], "http://proxy/api")
        self.assertEqual(env["ANTHROPIC_AUTH_TOKEN"], "sk-test")
        self.assertEqual(env["ANTHROPIC_API_KEY"], "")
        self.assertEqual(env["CLAUDE_CODE_SUBAGENT_MODEL"], "m/x")
        self.assertNotIn("OPENROUTER_API_KEY", env)


if __name__ == "__main__":
    unittest.main()
