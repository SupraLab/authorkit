---
paths:
  - "crates/ak-core/**/*.rs"
  - "apps/desktop/src/chat_store.rs"
---

<!-- Generated from .cursor/rules/persistence.mdc by scripts/sync-agent-config.sh. Do not edit. -->

# Persistence & data safety

Projects are plain folders the author owns (ADR 0003). See
`docs/architecture/persistence-and-safety.md` and `docs/reference/file-formats.md`.

- Every project write goes through `ak_core::fsutil::write_atomic` (temp file + rename).
- Deletions are soft: move to `.authorkit-rust/trash/` via the trash API, never `remove_file`
  on user content.
- Operations that rewrite text (tools, accept AI edit, blueprint, restore) take a version
  snapshot first with a reason (`avant outil`, `avant accept IA`, …).
- Persisted structs (`authorkit.toml`, scene/note frontmatter, `index.json`, chat JSON) must stay
  readable by older and newer versions: new fields get `#[serde(default)]`; never rename or
  remove a field without a migration. Update `docs/reference/file-formats.md`.
- App state lives in `<project>/.authorkit-rust/`; user-global state in `~/.authorkit-rust/`
  (or `AUTHORKIT_HOME`). Secrets never go in a project.
- Don't trust paths from data: ids are slugs; join them under the project root only.
- Tests use `tempfile::tempdir()` and `Project::create`; assert files on disk, not just memory.
