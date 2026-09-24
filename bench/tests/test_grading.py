import tempfile
import unittest
from pathlib import Path

import helpers  # noqa: F401
from lib.grading import grade_answer, grade_change, normalize, parse_answer, score_answer

GOLD = ["acme.core.Engine", ["acme.plugins.Plugin", "acme.base.plugins.Plugin"]]


class ParseAnswer(unittest.TestCase):
    def test_last_answer_line_wins(self):
        text = 'ANSWER: ["a"]\nthinking more...\nANSWER: ["b", "c"]'
        self.assertEqual(parse_answer(text), (["b", "c"], None))

    def test_tolerates_trailing_period_and_fences(self):
        self.assertEqual(parse_answer('ANSWER: ["a", "b"].')[0], ["a", "b"])
        self.assertEqual(parse_answer('ANSWER: `["a"]`')[0], ["a"])
        self.assertEqual(parse_answer('**ANSWER:** ["a"]')[0], ["a"])
        self.assertEqual(parse_answer('ANSWER:\n```json\n["a",\n "b"]\n```')[0], ["a", "b"])

    def test_missing_or_bad(self):
        self.assertEqual(parse_answer("no answer here")[0], None)
        self.assertEqual(parse_answer(None)[1], "no final result text")
        self.assertEqual(parse_answer("ANSWER: Engine and Plugin")[1], "ANSWER line is not valid JSON")
        self.assertEqual(parse_answer('ANSWER: {"a": 1}')[1], "ANSWER is not a JSON array")


class Score(unittest.TestCase):
    def test_normalize(self):
        self.assertEqual(normalize(" `acme.core.Engine` "), "acme.core.Engine")
        self.assertEqual(normalize('"get_user()"'), "get_user")
        self.assertEqual(normalize("'x'"), "x")

    def test_exact_and_alias(self):
        g = score_answer(["acme.core.Engine", "acme.base.plugins.Plugin"], GOLD)
        self.assertEqual((g["precision"], g["recall"], g["f1"], g["pass"]), (1.0, 1.0, 1.0, True))

    def test_unique_last_segment_fallback(self):
        g = score_answer(["Engine", "plugins::Plugin"], GOLD)
        self.assertEqual(g["f1"], 1.0)
        self.assertEqual(score_answer(["core/Engine()"], GOLD)["recall"], 0.5)

    def test_ambiguous_last_segment_does_not_match(self):
        gold = ["a.x.Handler", "b.y.Handler"]
        g = score_answer(["Handler"], gold)
        self.assertEqual(g["recall"], 0.0)
        self.assertEqual(g["false_positives"], ["Handler"])

    def test_each_gold_entry_matched_once(self):
        # duplicate items collapse; a second alias of an already-matched entry is
        # neither a hit nor a false positive
        g = score_answer(["acme.core.Engine", "`acme.core.Engine`", "acme.plugins.Plugin",
                          "acme.base.plugins.Plugin"], GOLD)
        self.assertEqual((g["precision"], g["recall"]), (1.0, 1.0))
        self.assertEqual(g["redundant"], ["acme.base.plugins.Plugin"])
        # fallback can't double-match either
        g = score_answer(["x.Engine", "y.Engine"], GOLD)
        self.assertEqual((g["precision"], g["recall"]), (0.5, 0.5))

    def test_false_positive_lowers_precision(self):
        g = score_answer(["acme.core.Engine", "acme.plugins.Plugin", "acme.ctx.Session"], GOLD)
        self.assertAlmostEqual(g["precision"], 2 / 3, places=3)
        self.assertEqual(g["recall"], 1.0)
        self.assertEqual(g["f1"], 0.8)
        self.assertFalse(g["pass"])
        self.assertTrue(score_answer(["acme.core.Engine", "acme.plugins.Plugin", "z"], GOLD, 0.8)["pass"])

    def test_grade_answer_missing(self):
        g = grade_answer("I think it is Engine.", GOLD)
        self.assertEqual((g["f1"], g["pass"]), (0.0, False))
        self.assertEqual(g["grade_error"], "no ANSWER: line found")

    def test_empty_answer(self):
        g = grade_answer("ANSWER: []", GOLD)
        self.assertEqual((g["f1"], g["grade_error"]), (0.0, None))


class Change(unittest.TestCase):
    def test_hidden_tests_copied_and_run(self):
        with tempfile.TemporaryDirectory() as td:
            work, hidden = Path(td, "w"), Path(td, "h")
            (hidden / "t").mkdir(parents=True)
            (hidden / "t" / "check.sh").write_text("test -f marker && echo ok-marker")
            work.mkdir()
            env = {"PATH": "/usr/bin:/bin"}
            g = grade_change(work, hidden, "bash t/check.sh", env, 30)
            self.assertFalse(g["pass"])
            (work / "marker").write_text("")
            g = grade_change(work, hidden, "bash t/check.sh", env, 30)
            self.assertTrue(g["pass"])
            self.assertIn("ok-marker", g["output_tail"])

    def test_timeout(self):
        with tempfile.TemporaryDirectory() as td:
            work, hidden = Path(td, "w"), Path(td, "h")
            work.mkdir(), hidden.mkdir()
            g = grade_change(work, hidden, "sleep 5", {"PATH": "/usr/bin:/bin"}, 0.5)
            self.assertFalse(g["pass"])
            self.assertIn("timed out", g["grade_error"])


if __name__ == "__main__":
    unittest.main()
