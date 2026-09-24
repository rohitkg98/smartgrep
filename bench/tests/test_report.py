import json
import shutil
import tempfile
import unittest
from pathlib import Path

import helpers  # noqa: F401
from helpers import FIXTURES
from lib.config import load_taskset
from lib.report import aggregate, write_report
from lib.store import Store


class Aggregate(unittest.TestCase):
    def setUp(self):
        self.records = Store(FIXTURES).records()
        self.s = aggregate(self.records)

    def test_counts_exclude_infra_and_superseded(self):
        self.assertEqual(self.s["schema_version"], 1)
        self.assertEqual(self.s["runs"], {"records": 8, "ok": 6, "infra_error_records": 1})

    def test_headline(self):
        m = self.s["models"][0]
        a, b = m["variants"]["A"], m["variants"]["B"]
        self.assertEqual((a["n"], b["n"]), (3, 3))
        self.assertAlmostEqual(a["mean_total_tokens"], 7000 / 3)
        self.assertAlmostEqual(b["mean_total_tokens"], 4000)
        self.assertAlmostEqual(m["token_ratio_a_over_b"], 0.5833, places=4)
        self.assertAlmostEqual(m["cost_ratio_a_over_b"], 0.5833, places=3)
        self.assertAlmostEqual(m["task_geomean_token_ratio_a_over_b"], 0.375 ** 0.5, places=3)
        self.assertEqual((a["pass_rate"], b["pass_rate"]), (0.6667, 0.6667))
        self.assertEqual((a["tokens_per_pass"], b["tokens_per_pass"]), (3500.0, 6000.0))
        self.assertEqual(a["smartgrep_use_share"], 0.6667)
        self.assertEqual(b["smartgrep_use_share"], 0.0)

    def test_cells(self):
        cells = {(c["task_id"], c["variant"]): c for c in self.s["cells"]}
        c = cells[("tiny-shape-subclasses", "B")]
        self.assertEqual(c["n"], 2)
        self.assertEqual(c["total_tokens"], {"mean": 4000, "min": 3000, "max": 5000})
        self.assertEqual(c["pass_rate"], 1.0)
        self.assertEqual(cells[("tiny-shape-subclasses", "A")]["bash_smartgrep"]["max"], 2)
        self.assertEqual(cells[("tiny-add-double", "B")]["kind"], "change")


class PublicOutput(unittest.TestCase):
    def test_public_output_has_no_private_strings(self):
        taskset = load_taskset(FIXTURES / "taskset")
        with tempfile.TemporaryDirectory() as td:
            out = Path(td)
            shutil.copy(FIXTURES / "runs.jsonl", out / "runs.jsonl")
            write_report(out, taskset)
            private = (out / "report.md").read_text() + (out / "index.html").read_text()
            public = (out / "public" / "summary.json").read_text() + (out / "public" / "index.html").read_text()
            secrets = ["SECRET-PROMPT-MARKER", "derive (directly or indirectly)", "pkg.shapes.Circle",
                       "pkg.shapes.RoundedSquare", "pkg.square.RoundedSquare", "ANSWER", "python3 -m unittest",
                       "HiddenTest", "double(x)", "stale"]
            for t in taskset.tasks:
                secrets.append(t.prompt[:40])
            for s in secrets:
                self.assertNotIn(s, public, s)
            # ... while the private report has them
            for s in ("SECRET-PROMPT-MARKER", "pkg.shapes.RoundedSquare"):
                self.assertIn(s, private)
            pub = json.loads((out / "public" / "summary.json").read_text())
            self.assertEqual(pub["schema_version"], 1)
            self.assertEqual({t["task_id"] for t in pub["tasks"]}, {"tiny-shape-subclasses", "tiny-add-double"})
            self.assertNotIn("task_details", pub)
            self.assertIn("task_details", json.loads((out / "summary.json").read_text()))
            html = (out / "public" / "index.html").read_text()
            self.assertIn("prefers-color-scheme: dark", html)
            self.assertNotIn("http", html.replace("http-equiv", ""))  # no external assets


if __name__ == "__main__":
    unittest.main()
