#!/bin/sh
set -e
SKILL_SRC="$(brew --prefix)/share/smartgrep/SKILL.md"
SKILL_DST="$HOME/.claude/skills/smartgrep/SKILL.md"

if [ ! -f "$SKILL_SRC" ]; then
  echo "Error: SKILL.md not found at $SKILL_SRC"
  echo "Reinstall smartgrep: brew reinstall smartgrep"
  exit 1
fi

mkdir -p "$(dirname "$SKILL_DST")"
cp "$SKILL_SRC" "$SKILL_DST"
echo "Claude Code skill installed at $SKILL_DST"
