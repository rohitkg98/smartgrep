import json
import unittest

import helpers  # noqa: F401
from lib.openrouter import OpenRouterCost

BASE = "http://proxy.test/api"


class FakeHTTP:
    """Maps URL -> list of (status, payload) served in order (last one repeats)."""

    def __init__(self, routes):
        self.routes = {k: list(v) for k, v in routes.items()}
        self.calls = []

    def __call__(self, url, headers):
        self.calls.append((url, headers))
        seq = self.routes.get(url)
        if not seq:
            return 404, b"{}"
        status, payload = seq[0] if len(seq) == 1 else seq.pop(0)
        return status, json.dumps(payload).encode()


def gen(cost):
    return 200, {"data": {"id": "x", "total_cost": cost}}


def key(usage):
    return 200, {"data": {"label": "k", "usage": usage}}


def client(http, **kw):
    return OpenRouterCost(api_base=BASE, api_key="sk-secret", http_get=http, sleep=lambda s: None, **kw)


class Cost(unittest.TestCase):
    def test_generation_sum_with_lag(self):
        http = FakeHTTP({
            f"{BASE}/v1/generation?id=gen-1": [(404, {}), (404, {}), gen(0.01)],  # stats lag
            f"{BASE}/v1/generation?id=gen-2": [gen(0.02)],
            f"{BASE}/v1/key": [key(10.5)],
        })
        c = client(http)
        before = c.key_usage()
        out = c.run_cost(["gen-1", "gen-2"], before, cc_cost_usd=0.5)
        self.assertEqual(out["cost_source"], "openrouter_generation")
        self.assertAlmostEqual(out["cost_usd"], 0.03)
        self.assertEqual(out["generation_missing"], 0)
        self.assertEqual(out["key_delta_usd"], 0.0)
        self.assertTrue(all(h.get("Authorization") == "Bearer sk-secret" for _, h in http.calls))

    def test_fallback_to_key_delta_when_ids_unusable(self):
        http = FakeHTTP({f"{BASE}/v1/key": [key(1.0), key(1.25)]})
        c = client(http, retries=2)
        before = c.key_usage()
        out = c.run_cost(["gen-a", "gen-b", "gen-c"], before, cc_cost_usd=0.3)
        self.assertEqual(out["cost_source"], "openrouter_key_delta")
        self.assertAlmostEqual(out["cost_usd"], 0.25)
        self.assertEqual(out["generation_missing"], 3)
        gen_calls = [u for u, _ in http.calls if "generation" in u]
        self.assertEqual(len(gen_calls), 3)  # first id retried, the rest not attempted

    def test_non_openrouter_ids_use_key_delta(self):
        http = FakeHTTP({f"{BASE}/v1/key": [key(2.0), key(2.5)]})
        c = client(http)
        out = c.run_cost(["msg_01abc"], c.key_usage(), cc_cost_usd=0.4)
        self.assertEqual((out["cost_source"], out["cost_usd"]), ("openrouter_key_delta", 0.5))

    def test_subagents_prefer_key_delta(self):
        http = FakeHTTP({f"{BASE}/v1/generation?id=gen-1": [gen(0.1)], f"{BASE}/v1/key": [key(0.0), key(0.3)]})
        c = client(http)
        out = c.run_cost(["gen-1"], c.key_usage(), 0.2, subagents_spawned=1)
        self.assertEqual(out["cost_source"], "openrouter_key_delta")
        self.assertEqual(out["generation_cost_usd"], 0.1)

    def test_partial_generation_without_key(self):
        http = FakeHTTP({f"{BASE}/v1/generation?id=gen-1": [gen(0.1)]})
        out = client(http, retries=0).run_cost(["gen-1", "gen-2"], None, 0.2)
        self.assertEqual(out["cost_source"], "openrouter_generation_partial")
        self.assertEqual(out["generation_missing"], 1)

    def test_auth_error_not_retried_and_estimate_used(self):
        http = FakeHTTP({f"{BASE}/v1/generation?id=gen-1": [(401, {})], f"{BASE}/v1/key": [(401, {})]})
        c = client(http)
        out = c.run_cost(["gen-1"], c.key_usage(), 0.7)
        self.assertEqual((out["cost_source"], out["cost_usd"]), ("claude_code_estimate", 0.7))
        self.assertEqual(len(http.calls), 2)  # key, generation: no retries on 401

    def test_disabled(self):
        c = OpenRouterCost.disabled()
        self.assertIsNone(c.key_usage())
        self.assertEqual(c.run_cost(["gen-1"], None, 0.1)["cost_source"], "claude_code_estimate")


if __name__ == "__main__":
    unittest.main()
