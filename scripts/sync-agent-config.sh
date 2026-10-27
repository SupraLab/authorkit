#!/usr/bin/env bash
# Sync AI-assistant config from the canonical sources:
#   .agents/skills/<name>/   -> .cursor/skills/<name>/, .claude/skills/<name>/  (verbatim copies)
#   .cursor/rules/<name>.mdc -> .claude/rules/<name>.md  (globs -> `paths:` frontmatter)
#
# Usage: scripts/sync-agent-config.sh [--check]
#   --check  exit 1 if generated files differ from what is committed (used by CI)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

CHECK=0
[[ "${1:-}" == "--check" ]] && CHECK=1

OUT="$ROOT"
if [[ $CHECK -eq 1 ]]; then
  OUT="$(mktemp -d)"
  trap 'rm -rf "$OUT"' EXIT
fi

# --- skills ---------------------------------------------------------------
for dest in .cursor/skills .claude/skills; do
  rm -rf "${OUT:?}/$dest"
  mkdir -p "$OUT/$dest"
  cp -R .agents/skills/. "$OUT/$dest/"
done

# --- rules ----------------------------------------------------------------
rm -rf "${OUT:?}/.claude/rules"
mkdir -p "$OUT/.claude/rules"

for src in .cursor/rules/*.mdc; do
  name="$(basename "$src" .mdc)"
  dest="$OUT/.claude/rules/$name.md"

  # Frontmatter is the block between the first two `---` lines.
  globs="$(awk 'NR==1 && $0=="---"{f=1; next} f && $0=="---"{exit} f && /^globs:/{sub(/^globs:[ ]*/, ""); print}' "$src")"
  always="$(awk 'NR==1 && $0=="---"{f=1; next} f && $0=="---"{exit} f && /^alwaysApply:/{sub(/^alwaysApply:[ ]*/, ""); print}' "$src")"

  {
    if [[ "$always" != "true" && -n "$globs" ]]; then
      echo "---"
      echo "paths:"
      IFS=',' read -ra parts <<< "$globs"
      for g in "${parts[@]}"; do
        g="$(echo "$g" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
        [[ -n "$g" ]] && echo "  - \"$g\""
      done
      echo "---"
      echo
    fi
    echo "<!-- Generated from .cursor/rules/$name.mdc by scripts/sync-agent-config.sh. Do not edit. -->"
    echo
    # Body: everything after the closing frontmatter delimiter, minus leading blank lines.
    awk 'NR==1 && $0=="---"{f=1; next} f==1 && $0=="---"{f=2; next} f==2 || f==0 {print}' "$src" \
      | awk 'NF{p=1} p'
  } > "$dest"
done

if [[ $CHECK -eq 1 ]]; then
  status=0
  for d in .cursor/skills .claude/skills .claude/rules; do
    if ! diff -r "$OUT/$d" "$ROOT/$d" > /dev/null 2>&1; then
      echo "::error::$d is out of sync. Run scripts/sync-agent-config.sh and commit." >&2
      diff -r "$OUT/$d" "$ROOT/$d" >&2 || true
      status=1
    fi
  done
  [[ $status -eq 0 ]] && echo "Agent config in sync."
  exit $status
fi

echo "Synced skills to .cursor/skills, .claude/skills and rules to .claude/rules."
