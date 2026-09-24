import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

import helpers  # noqa: F401
from adapters.base import Agent, AgentResult
from adapters.claude_code import classify_outcome, parse_stream
from helpers import BASE_PATH, STREAMS, install_fake_bins, make_taskset, make_tiny_repo
from lib import prepare as prep
from lib.config import load_taskset
from lib.openrouter import OpenRouterCost
from lib.runner import RunConfig, Runner
from lib.store import Store

import bench


class InProcessAgent(Agent):
    """Replays a fixture transcript; optional per-call outcome script."""
    name = "fake"

    def __init__(self, cost=0.1, fail_first=0):
        self.cost = cost
        self.fail_first = fail_first
        self.calls = []

    def version(self, env):
        return "fake-1"

    def run(self, prompt, workdir, env, model, effort, max_turns, timeout_s, max_budget_usd,
            transcript_path, stderr_path):
        self.calls.append({"prompt": prompt, "workdir": workdir, "env": env, "model": model})
        transcript_path.parent.mkdir(parents=True, exist_ok=True)
        stderr_path.write_text("")
        if len(self.calls) <= self.fail_first:
            src = STREAMS / "real_rate_limited_killed.jsonl"
        else:
            src = STREAMS / "real_success.jsonl"
        lines = src.read_text().splitlines()
        if src.name == "real_success.jsonl":
            res = json.loads(lines[-1])
            res["total_cost_usd"] = self.cost
            lines[-1] = json.dumps(res)
        transcript_path.write_text("\n".join(lines) + "\n")
        m = parse_stream(lines)
        status, err, fatal = classify_outcome(m, 0, False)
        return AgentResult(0, False, 0.01, transcript_path, stderr_path, m, status, err, fatal)


class RunnerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.td = tempfile.TemporaryDirectory()
        tmp = Path(cls.td.name)
        url, sha = make_tiny_repo(tmp)
        cls.taskset_dir = make_taskset(tmp, url, sha)
        cls.cache = tmp / "cache"
        cls.claude, cls.smartgrep = install_fake_bins(tmp)
        ts = load_taskset(cls.taskset_dir)
        prep.prepare_repo(cls.cache, ts.repos["tiny"], log=lambda *a: None)

    @classmethod
    def tearDownClass(cls):
        cls.td.cleanup()

    def config(self, out, **kw):
        ts = load_taskset(self.taskset_dir)
        base = dict(taskset=ts, tasks=[ts.task("tiny-shape-subclasses")], models=["m1"],
                    variants=["A", "B"], repeats=2, out=out, cache=self.cache,
                    smartgrep=self.smartgrep, budget_usd=100, max_run_usd=0.1,
                    base_path=BASE_PATH, log=lambda *a: None)
        base.update(kw)
        return RunConfig(**base)

    def test_prepare_idempotent_and_setup_ran(self):
        ts = load_taskset(self.taskset_dir)
        msgs = []
        prep.prepare_repo(self.cache, ts.repos["tiny"], log=msgs.append)
        self.assertIn("already prepared", msgs[0])
        pristine = prep.pristine_path(self.cache, "tiny")
        self.assertTrue((pristine / ".setup-ran").exists())
        self.assertTrue((pristine / "pkg" / "shapes.py").exists())

    def test_budget_stop(self):
        with tempfile.TemporaryDirectory() as out:
            agent = InProcessAgent(cost=0.1)
            s = Runner(self.config(Path(out), budget_usd=0.25), agent, OpenRouterCost.disabled()).run()
            recs = Store(Path(out)).records()
            self.assertEqual(len(recs), 2)  # 0 + .1 <= .25, .1 + .1 <= .25, .2 + .1 > .25
            self.assertIn("budget stop", s["stopped"])
            self.assertEqual({r["cost_source"] for r in recs}, {"claude_code_estimate"})
            # a new invocation with the same budget does nothing more
            Runner(self.config(Path(out), budget_usd=0.25), agent, OpenRouterCost.disabled()).run()
            self.assertEqual(len(Store(Path(out)).records()), 2)

    def test_resume_and_infra_retry(self):
        with tempfile.TemporaryDirectory() as out:
            out = Path(out)
            agent = InProcessAgent(fail_first=1)
            s = Runner(self.config(out), agent, OpenRouterCost.disabled()).run()
            self.assertEqual((s["executed"], s["ok"], s["infra_error"]), (4, 3, 1))
            recs = Store(out).records()
            first = recs[0]
            self.assertEqual(first["status"], "infra_error")
            self.assertIsNone(first.get("grade"))
            # second invocation retries only the infra error
            agent2 = InProcessAgent()
            s = Runner(self.config(out), agent2, OpenRouterCost.disabled()).run()
            self.assertEqual(s["executed"], 1)
            self.assertEqual(len(agent2.calls), 1)
            recs = Store(out).records()
            self.assertEqual(recs[-1]["run_key"], first["run_key"])
            self.assertEqual(recs[-1]["status"], "ok")
            # third invocation: nothing to do
            agent3 = InProcessAgent()
            s = Runner(self.config(out), agent3, OpenRouterCost.disabled()).run()
            self.assertEqual((s["executed"], len(agent3.calls)), (0, 0))

    def test_gives_up_after_max_attempts_and_stops_on_consecutive_errors(self):
        with tempfile.TemporaryDirectory() as out:
            out = Path(out)
            for _ in range(2):
                s = Runner(self.config(out, max_attempts=2, max_consecutive_infra=2),
                           InProcessAgent(fail_first=99), OpenRouterCost.disabled()).run()
                self.assertIn("consecutive infra errors", s["stopped"])
            s = Runner(self.config(out, max_attempts=2), InProcessAgent(), OpenRouterCost.disabled()).run()
            self.assertEqual(s["gave_up"], 2)  # the two keys that failed twice
            self.assertEqual(s["executed"], 2)

    def test_environment_isolation(self):
        with tempfile.TemporaryDirectory() as out:
            agent = InProcessAgent()
            Runner(self.config(Path(out), repeats=1), agent, OpenRouterCost.disabled()).run()
            a, b = agent.calls
            self.assertEqual(Path(a["workdir"]), prep.work_path(self.cache, "tiny"))
            self.assertTrue(a["env"]["PATH"].endswith(BASE_PATH))
            self.assertIn("bin-A", a["env"]["PATH"].split(":")[0])
            self.assertEqual(b["env"]["PATH"], BASE_PATH)
            self.assertNotEqual(a["env"]["HOME"], b["env"]["HOME"])
            self.assertEqual(a["prompt"], b["prompt"])
            self.assertFalse(prep.work_path(self.cache, "tiny").exists())  # cleaned up
            recs = Store(Path(out)).records()
            self.assertEqual([r["variant"] for r in recs], ["A", "B"])
            self.assertEqual(recs[0]["agent_version"], "fake-1")
            self.assertIn("tasks_repo_sha", recs[0])  # None here: the temp task set is not a git repo
            self.assertTrue(recs[0]["task_hash"].startswith("sha256:"))


class EndToEnd(unittest.TestCase):
    """bench.py run/report through the CLI, with a fake `claude` and fake `smartgrep`."""

    def test_cli_run_resume_report(self):
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            url, sha = make_tiny_repo(tmp)
            tasks = make_taskset(tmp, url, sha)
            claude, smartgrep = install_fake_bins(tmp)
            cache, out = tmp / "cache", tmp / "results"
            common = ["--tasks-dir", str(tasks), "--cache", str(cache)]
            self.assertEqual(self._main(["prepare", *common]), 0)
            run = ["run", *common, "--models", "fake/model-1", "--repeats", "1",
                   "--smartgrep", str(smartgrep), "--agent-bin", str(claude), "--out", str(out),
                   "--no-openrouter-cost", "--base-path", BASE_PATH, "--budget-usd", "5",
                   "--timeout-s", "60"]
            self.assertEqual(self._main(run), 0)
            recs = Store(out).records()
            self.assertEqual(len(recs), 4)
            for r in recs:
                self.assertEqual(r["status"], "ok", (r["run_key"], r.get("error")))
                self.assertTrue(r["pass"], r["run_key"])
                self.assertEqual(r["agent_version"], "9.9.9 (Fake Claude)")
                self.assertEqual(r["smartgrep_calls"], 1 if r["variant"] == "A" else 0)
                self.assertEqual(r["used_smartgrep"], r["variant"] == "A")
                self.assertTrue((out / r["transcript"]).is_file())
            change = [r for r in recs if r["task_kind"] == "change"]
            for r in change:
                diff = (out / r["diff"]).read_text()
                self.assertIn("+def double(x):", diff)
                self.assertNotIn("CLAUDE.md", diff)  # init artifacts are not part of the agent's diff
                self.assertNotIn("test_hidden", diff)
            answer = [r for r in recs if r["task_kind"] == "answer"]
            self.assertEqual({r["grade"]["f1"] for r in answer}, {1.0})
            a = next(r for r in answer if r["variant"] == "A")
            b = next(r for r in answer if r["variant"] == "B")
            self.assertLess(a["tokens"]["total"], b["tokens"]["total"])

            # resume: nothing re-run
            self.assertEqual(self._main(run), 0)
            self.assertEqual(len(Store(out).records()), 4)

            # report
            self.assertEqual(self._main(["report", *common, "--out", str(out)]), 0)
            summary = json.loads((out / "summary.json").read_text())
            m = summary["models"][0]
            self.assertEqual(m["variants"]["A"]["smartgrep_use_share"], 1.0)
            self.assertEqual(m["variants"]["B"]["pass_rate"], 1.0)
            self.assertAlmostEqual(m["token_ratio_a_over_b"], 1 / 3, places=2)
            self.assertTrue((out / "public" / "index.html").is_file())
            self.assertIn("SECRET-PROMPT-MARKER", (out / "report.md").read_text())
            for f in ("summary.json", "index.html"):
                self.assertNotIn("SECRET-PROMPT-MARKER", (out / "public" / f).read_text())

    def test_infra_error_from_fake_agent(self):
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            url, sha = make_tiny_repo(tmp)
            tasks = make_taskset(tmp, url, sha)
            claude, smartgrep = install_fake_bins(tmp)
            common = ["--tasks-dir", str(tasks), "--cache", str(tmp / "c")]
            self._main(["prepare", *common])
            out = tmp / "r"
            self._main(["check-model", *common, "--model", "fake/ratelimit", "--smartgrep", str(smartgrep),
                        "--agent-bin", str(claude), "--out", str(out), "--no-openrouter-cost",
                        "--base-path", BASE_PATH])
            recs = Store(out).records()
            self.assertEqual([r["status"] for r in recs], ["infra_error", "infra_error"])
            self.assertTrue(all("no result event" in r["error"] for r in recs))
            self.assertEqual({r["task_id"] for r in recs}, {"tiny-shape-subclasses"})

    def test_refuses_out_inside_repo(self):
        with tempfile.TemporaryDirectory() as td:
            tasks = make_taskset(Path(td))
            inside = Path(bench.__file__).resolve().parent / "results-test"
            err = io.StringIO()
            with redirect_stderr(err), redirect_stdout(io.StringIO()):
                rc = bench.main(["run", "--tasks-dir", str(tasks), "--models", "m", "--out", str(inside),
                                 "--dry-run"])
            self.assertEqual(rc, 2)
            self.assertIn("inside the smartgrep repository", err.getvalue())
            self.assertFalse(inside.exists())

    def test_dry_run_plans_without_prepared_repos(self):
        with tempfile.TemporaryDirectory() as td:
            tasks = make_taskset(Path(td))
            buf = io.StringIO()
            with redirect_stdout(buf), redirect_stderr(io.StringIO()):
                rc = bench.main(["run", "--tasks-dir", str(tasks), "--cache", str(Path(td, "c")),
                                 "--models", "m1,m2", "--repeats", "2", "--out", str(Path(td, "o")),
                                 "--dry-run", "--no-openrouter-cost"])
            self.assertEqual(rc, 0)
            self.assertEqual(buf.getvalue().count(" todo "), 2 * 2 * 2 * 2)

    def _main(self, argv):
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()) as err:
            rc = bench.main(argv)
        if rc != 0:
            print(err.getvalue())
        return rc


if __name__ == "__main__":
    unittest.main()
