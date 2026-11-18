#!/usr/bin/env bash
# Link repo-owned skills into the project Claude view.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/.claude/skills"
for source in "$root/.agents/skills/"*; do
  [[ -f "$source/SKILL.md" ]] || continue
  name="$(basename "$source")"
  ln -sfn "../../.agents/skills/$name" "$root/.claude/skills/$name"
done
for link in "$root/.claude/skills/"*; do
  [[ -L "$link" && ! -e "$link" ]] || continue
  rm "$link"
done
