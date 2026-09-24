#!/usr/bin/env python3
"""Stand-in for the smartgrep binary in end-to-end tests (init + ls only)."""
import os
import sys

args = sys.argv[1:]
if args[:1] == ["init"]:
    with open("CLAUDE.md", "a") as f:
        f.write("## Code navigation\nUse `smartgrep` for structural queries.\n")
    os.makedirs(".claude/skills/smartgrep", exist_ok=True)
    with open(".claude/skills/smartgrep/SKILL.md", "w") as f:
        f.write("---\nname: smartgrep\n---\n")
    os.makedirs(".smartgrep", exist_ok=True)
    with open(".gitignore", "a") as f:
        f.write(".smartgrep/\n")
    print("Created CLAUDE.md (smartgrep block)")
elif args[:1] == ["ls"]:
    print("class  Circle         pkg/shapes.py:6\nclass  RoundedSquare  pkg/shapes.py:14")
else:
    sys.stderr.write("error: unexpected argument\n")
    sys.exit(2)
