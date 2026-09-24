import unittest

import helpers  # noqa: F401
from lib.bashclass import classify


class Classify(unittest.TestCase):
    def check(self, cmd, expected):
        self.assertEqual(classify(cmd), set(expected), cmd)

    def test_single_programs(self):
        self.check("smartgrep refs Index", {"smartgrep"})
        self.check("/usr/local/bin/smartgrep ls functions", {"smartgrep"})
        self.check("./target/debug/smartgrep show X", {"smartgrep"})
        self.check("grep -rn foo .", {"grep"})
        self.check("rg foo", {"grep"})
        self.check("ag foo", {"grep"})
        self.check("ack foo", {"grep"})
        self.check("git grep -n foo", {"grep"})
        self.check("find . -name '*.py'", {"find"})
        self.check("fd Plugin", {"find"})
        self.check("ls -la src", {"find"})
        self.check("tree -L 2", {"find"})
        self.check("cat a.py", {"read"})
        self.check("head -50 a.py", {"read"})
        self.check("tail -n 5 a.py", {"read"})
        self.check("sed -n '10,20p' a.py", {"read"})
        self.check("less a.py", {"read"})
        self.check("bat a.py", {"read"})
        self.check("python3 -m pytest", {"other"})
        self.check("sed -i 's/a/b/' x", {"other"})
        self.check("git status", {"other"})

    def test_compound(self):
        self.check("grep -rn foo . | head -20", {"grep"})           # head is a filter here
        self.check("cat a.py | grep foo", {"read", "grep"})
        self.check("cd src && grep -rn foo . 2>&1 | sort | uniq", {"grep"})
        self.check("find . -name '*.go' | xargs grep -l Handler", {"find", "grep"})
        self.check("smartgrep refs X; cat a.py", {"smartgrep", "read"})
        self.check("ls src\ncat src/a.py", {"find", "read"})
        self.check("FOO=1 timeout 10 rg x || echo none", {"grep"})
        self.check("sudo -E smartgrep index", {"smartgrep"})
        self.check("grep 'a|b;c && d' file", {"grep"})           # operators inside quotes
        self.check("echo hi", {"other"})
        self.check("", {"other"})

    def test_unbalanced_quotes_do_not_crash(self):
        self.assertIn("grep", classify("grep 'unterminated | head"))


if __name__ == "__main__":
    unittest.main()
