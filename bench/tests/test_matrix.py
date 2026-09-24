import unittest

import helpers  # noqa: F401
from lib.config import Task
from lib.matrix import build_prompt, plan


def task(i, kind="answer", **kw):
    return Task(id=f"t{i}", repo="r", kind=kind, prompt=f"question {i}", source="x", **kw)


class Plan(unittest.TestCase):
    def setUp(self):
        self.tasks = [task(i) for i in range(4)]

    def test_full_matrix_and_adjacency(self):
        p = plan(["m1", "m2"], self.tasks, ["A", "B"], 3, seed=7)
        self.assertEqual(len(p), 2 * 4 * 2 * 3)
        self.assertEqual(len({(r.model, r.task_id, r.variant, r.repeat) for r in p}), len(p))
        for i in range(0, len(p), 2):
            a, b = p[i], p[i + 1]
            self.assertEqual((a.model, a.task_id, a.repeat), (b.model, b.task_id, b.repeat))
            self.assertNotEqual(a.variant, b.variant)

    def test_variant_order_alternates_between_repeats(self):
        p = plan(["m1"], self.tasks, ["A", "B"], 3, seed=1)
        firsts = {}
        for i in range(0, len(p), 2):
            firsts.setdefault(p[i].repeat, set()).add(p[i].variant)
        self.assertEqual(firsts, {1: {"A"}, 2: {"B"}, 3: {"A"}})

    def test_repeat_outer_and_deterministic(self):
        p1 = plan(["m1", "m2"], self.tasks, ["A", "B"], 2, seed=3)
        p2 = plan(["m1", "m2"], self.tasks, ["A", "B"], 2, seed=3)
        self.assertEqual(p1, p2)
        self.assertEqual([r.repeat for r in p1], sorted(r.repeat for r in p1))
        orders = {tuple(r.task_id for r in plan(["m1"], self.tasks, ["A"], 1, seed=s)) for s in range(20)}
        self.assertGreater(len(orders), 1)  # the seed shuffles the task order

    def test_run_key(self):
        r = plan(["deepseek/deepseek-v4.1-flash"], [task(0)], ["A"], 1, 0)[0]
        self.assertEqual(r.run_key("claude_code", "high"),
                         "claude_code__high__deepseek_deepseek-v4.1-flash__t0__A__r1")

    def test_bad_variant(self):
        with self.assertRaises(ValueError):
            plan(["m"], self.tasks, ["C"], 1, 0)


class Prompt(unittest.TestCase):
    def test_answer_prompt(self):
        p = build_prompt(task(1, answer_format="dotted names"))
        self.assertTrue(p.startswith("question 1\n\n"))
        self.assertIn("Do not modify files.", p)
        self.assertIn("`ANSWER: <JSON array of strings>` (items: dotted names).", p)

    def test_change_prompt(self):
        p = build_prompt(task(2, kind="change"))
        self.assertTrue(p.endswith("Make the change in the repository. Do not ask questions."))


if __name__ == "__main__":
    unittest.main()
