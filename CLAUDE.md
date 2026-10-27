# CLAUDE.md

Project instructions are shared with other agents in `AGENTS.md`:

@AGENTS.md

## Claude Code specifics

- Path-scoped rules live in `.claude/rules/` (generated from `.cursor/rules/` by
  `scripts/sync-agent-config.sh` — edit the `.cursor/rules/*.mdc` source).
- Skills live in `.claude/skills/` (copied from `.agents/skills/`).
- Before finishing a task, run the `verify-changes` skill.
