"""Classify Bash tool commands into coarse categories.

A command is split into segments at ``|``, ``||``, ``&&``, ``;``, ``&`` and
newlines (quote-aware). Each segment's program is mapped to a category; the
command then counts once per category it contains.

Segments after a ``|`` are filters (``grep x | head -5``): there, read/other
programs are ignored, while smartgrep/grep/find still count.
"""
from __future__ import annotations

import os
import re
import shlex

CATEGORIES = ("smartgrep", "grep", "find", "read", "other")

GREP = {"grep", "egrep", "fgrep", "rg", "ag", "ack", "ack-grep", "git-grep"}
FIND = {"find", "fd", "fdfind", "ls", "tree", "git-ls-files", "locate"}
READ = {"cat", "head", "tail", "less", "more", "bat", "batcat", "nl", "sed-n", "view"}
WRAPPERS = {"sudo", "time", "nice", "command", "exec", "builtin", "env", "nohup", "xargs", "timeout"}
NEUTRAL = {"cd", "pushd", "popd", "echo", "printf", "true", "pwd", "export", "set", ":"}
SEPARATORS = {"|", "||", "&&", ";", "&", "\n", "|&", ";;"}


def _tokens(command: str) -> list[str]:
    # Drop fd redirections like 2>&1 / >&2 / &> so their '&' isn't read as a separator.
    command = re.sub(r"\d*>&\d*-?", " ", command)
    command = re.sub(r"&>>?", ">", command)
    try:
        lex = shlex.shlex(command.replace("\n", " ; "), posix=True, punctuation_chars=";&|")
        lex.whitespace_split = True
        lex.commenters = ""
        return list(lex)
    except ValueError:
        # Unbalanced quotes etc.: fall back to a crude split.
        return re.split(r"\s+|(\|\||&&|[|;&])", command.replace("\n", " ; "))


def _segments(command: str) -> list[tuple[str, list[str]]]:
    """Return [(separator_before, words)]."""
    segs: list[tuple[str, list[str]]] = []
    sep, cur = "", []
    for tok in _tokens(command):
        if tok is None or tok == "":
            continue
        if tok in SEPARATORS:
            if cur:
                segs.append((sep, cur))
            sep, cur = tok, []
        else:
            cur.append(tok)
    if cur:
        segs.append((sep, cur))
    return segs


def _program(words: list[str]) -> str | None:
    i = 0
    while i < len(words):
        w = words[i].lstrip("(").lstrip("{")
        if not w or re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", w):
            i += 1
            continue
        base = os.path.basename(w)
        if base in WRAPPERS:
            i += 1
            # skip wrapper options and a numeric timeout argument
            while i < len(words) and (words[i].startswith("-") or re.match(r"^\d+[smhd]?$", words[i])):
                i += 1
            continue
        if base == "git":
            rest = [x for x in words[i + 1:] if not x.startswith("-")]
            if rest and rest[0] in ("grep", "ls-files"):
                return "git-" + rest[0]
            return "git"
        if base == "sed":
            if any(x == "-n" or (x.startswith("-") and not x.startswith("--") and "n" in x) for x in words[i + 1:]):
                return "sed-n"
            return "sed"
        return base
    return None


def category_of(program: str | None) -> str:
    if program is None:
        return "other"
    if program == "smartgrep":
        return "smartgrep"
    if program in GREP:
        return "grep"
    if program in FIND:
        return "find"
    if program in READ:
        return "read"
    return "other"


def classify(command: str) -> set[str]:
    """Set of categories contained in one Bash command string."""
    cats: set[str] = set()
    for sep, words in _segments(command or ""):
        prog = _program(words)
        if prog in NEUTRAL:
            continue
        cat = category_of(prog)
        if sep in ("|", "|&") and cat in ("read", "other"):
            continue
        cats.add(cat)
    return cats or {"other"}
