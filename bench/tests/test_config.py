import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import helpers  # noqa: F401
from helpers import FIXTURES, make_taskset
from lib.config import ConfigError, load_taskset, resolve_tasks_dir, select_tasks

GOOD_TASK = '''
[[task]]
id = "t1"
repo = "tiny"
kind = "answer"
prompt = "q?"
gold = ["a.B"]
'''


class LoadFixture(unittest.TestCase):
    def test_fixture_taskset_loads(self):
        ts = load_taskset(FIXTURES / "taskset")
        self.assertEqual(list(ts.repos), ["tiny"])
        self.assertEqual(ts.repos["tiny"].setup, ["touch .setup-ran"])
        ids = [t.id for t in ts.tasks]
        self.assertEqual(ids, ["tiny-shape-subclasses", "tiny-add-double"])
        a = ts.task("tiny-shape-subclasses")
        self.assertEqual(a.gold[1], ["pkg.shapes.RoundedSquare", "pkg.square.RoundedSquare"])
        self.assertEqual(a.pass_f1, 0.9)
        c = ts.task("tiny-add-double")
        self.assertEqual((c.kind, c.timeout_s), ("change", 60))
        self.assertTrue(ts.hidden_dir(c).is_dir())
        self.assertTrue(a.definition_hash().startswith("sha256:"))
        self.assertNotEqual(a.definition_hash(), c.definition_hash())

    def test_select(self):
        ts = load_taskset(FIXTURES / "taskset")
        self.assertEqual([t.id for t in select_tasks(ts, ["tiny-add-double"], None)], ["tiny-add-double"])
        with self.assertRaisesRegex(ConfigError, "unknown task id"):
            select_tasks(ts, ["nope"], None)
        with self.assertRaisesRegex(ConfigError, "unknown repo id"):
            select_tasks(ts, None, ["nope"])


class Validation(unittest.TestCase):
    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.root = make_taskset(Path(self.td.name))
        for p in (self.root / "tasks").glob("*.toml"):
            p.unlink()

    def tearDown(self):
        self.td.cleanup()

    def write(self, body):
        (self.root / "tasks" / "x.toml").write_text(body)

    def err(self, body, pattern):
        self.write(body)
        with self.assertRaisesRegex(ConfigError, pattern):
            load_taskset(self.root)

    def test_good(self):
        self.write(GOOD_TASK)
        self.assertEqual(load_taskset(self.root).tasks[0].id, "t1")

    def test_bad_tasks_give_clear_errors(self):
        self.err(GOOD_TASK.replace('kind = "answer"', 'kind = "quiz"'), r"x.toml task 't1': kind must be one of")
        self.err(GOOD_TASK.replace('repo = "tiny"', 'repo = "nope"'), r"repo 'nope' is not defined in repos.toml")
        self.err(GOOD_TASK.replace('gold = ["a.B"]', ''), r"gold must be a non-empty array")
        self.err(GOOD_TASK.replace('gold = ["a.B"]', 'gold = ["a", [1]]'), r"alias list")
        self.err(GOOD_TASK.replace('prompt = "q?"', ''), r"missing required field 'prompt'")
        self.err(GOOD_TASK + 'pass_f1 = 2\n', r"pass_f1 must be a number")
        self.err(GOOD_TASK + 'colour = "red"\n', r"unknown field\(s\) \['colour'\]")
        self.err(GOOD_TASK + GOOD_TASK, r"duplicate task id 't1'")
        self.err('[[task]]\nid = "c"\nrepo = "tiny"\nkind = "change"\nprompt = "p"\n'
                 'hidden_tests = "missing"\ntest_cmd = "true"\n', r"hidden/missing/ not found")
        self.err('[[task]]\nid = "c"\nrepo = "tiny"\nkind = "change"\nprompt = "p"\nhidden_tests = "tiny-add-double"\n',
                 r"missing required field 'test_cmd'")
        self.err("[[task]\n", r"invalid TOML")

    def test_bad_repo(self):
        self.write(GOOD_TASK)
        p = self.root / "repos.toml"
        p.write_text(p.read_text().replace("0" * 40, "abc"))
        with self.assertRaisesRegex(ConfigError, "sha must be a full 40-char"):
            load_taskset(self.root)

    def test_tasks_dir_required(self):
        with mock.patch.dict(os.environ, {}, clear=False):
            os.environ.pop("SMARTGREP_BENCH_TASKS", None)
            with self.assertRaisesRegex(ConfigError, "--tasks-dir"):
                resolve_tasks_dir(None)
            os.environ["SMARTGREP_BENCH_TASKS"] = str(self.root)
            self.assertEqual(resolve_tasks_dir(None), self.root.resolve())
        with self.assertRaisesRegex(ConfigError, "no repos.toml"):
            resolve_tasks_dir(self.td.name)


if __name__ == "__main__":
    unittest.main()
