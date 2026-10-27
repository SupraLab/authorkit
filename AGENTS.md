# AGENTS.md

Guidance for AI coding agents (Codex, Cursor, Claude Code, …) working on AuthorKit.
Format: [agents.md](https://agents.md/). Human contributors: see [CONTRIBUTING.md](CONTRIBUTING.md).

## Project

AuthorKit is a local-first desktop writing studio in pure Rust (egui/eframe 0.34). Cargo
workspace:

- `apps/desktop` — `authorkit-desktop` (bin + lib): UI, chat store, action dispatch (`app.rs`).
- `crates/ak-core` — projects, manuscript, notes, styles, trash, versions, atomic writes.
- `crates/ak-llm` — providers, settings/secrets, OpenAI-compatible client, context assembly,
  background `LlmWorker`.
- `crates/ak-coherence` — reviews, ideas, novel blueprints.
- `crates/ak-export` — Markdown, EPUB 3, DOCX.

Dependency direction: `ak-llm`, `ak-export` → `ak-core`; `ak-coherence` → `ak-core`, `ak-llm`;
desktop → all. Never make a library crate depend on `authorkit-desktop`.

Read [docs/architecture/overview.md](docs/architecture/overview.md) before structural changes.

## Commands

```bash
cargo run -p authorkit-desktop                         # run the app
cargo fmt --all -- --check                             # format check (CI)
cargo clippy --workspace --all-targets -- -D warnings  # lint (CI)
cargo test --workspace                                 # tests (CI, 3 OSes)
./scripts/coverage.sh                                  # coverage gates (cargo-llvm-cov)
```

A change is done only when fmt, clippy and tests pass. Linux needs GTK/xcb dev packages (see
[docs/guides/development.md](docs/guides/development.md)).

## Conventions

Follow community standards; don't invent house style.

- **Style:** `rustfmt` defaults ([Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/)).
- **Lints:** Clippy, warnings denied. Fix the cause; `#[allow]` only with a justification comment.
- **API design:** [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
  — RFC 430 casing, `as_/to_/into_` conversions, no `get_` prefix on getters, derive common traits.
- **Errors:** `thiserror` enum per library crate, `Result<T, Error>`; no `unwrap()`/`expect()` on
  I/O or user/model data outside tests.
- **Language:** code, comments, docs and commit messages in English. User-facing strings go through
  `apps/desktop/src/i18n.rs` (French is the default UI locale).
- **Comments:** explain constraints the code can't show; no narration.
- **Changelog:** add user-visible changes to `CHANGELOG.md` under `[Unreleased]`
  ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/), [SemVer](https://semver.org/)).
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`,
  `docs:`, `chore:` …).

## Architecture invariants

- **UI thread never blocks.** All model calls go through `ak_llm::worker::LlmWorker::submit`;
  no HTTP or long work in egui code.
- **Panels don't mutate app state.** They return actions (`NavAction`, `EditorAction`,
  `SidebarAction`, `OnboardingAction`, `WelcomeAction`) handled in `app.rs`.
- **Text positions are char indices**, not byte offsets.
- **Project writes use `ak_core::fsutil::write_atomic`.** Deletions go to trash; risky rewrites
  take a version snapshot first.
- **AI never writes text directly.** It produces edit proposals the author accepts
  ([ADR 0007](docs/adr/0007-ai-edits-as-proposals.md)).
- **Secrets:** API keys live under `~/.authorkit-rust/secrets/`, never in projects, logs or UI;
  pass errors through `redact_secrets`.

## Tests

- Unit tests in `#[cfg(test)] mod tests`; integration tests in `<crate>/tests/`.
- No network: use `JobKind::Mock`. No real home: set `AUTHORKIT_HOME` to a temp dir
  (`AuthorKitApp::for_tests` does it).
- Don't lower coverage gates in `scripts/coverage.sh` / `.github/workflows/ci.yml`; add tests.

## Area rules and skills

Scoped rules (also readable as plain Markdown): `.cursor/rules/*.mdc` — `rust-style`,
`egui-ui`, `persistence`, `llm`, `testing`, `docs`.

Skills (same content for every tool): `.agents/skills/` (Codex), `.cursor/skills/`,
`.claude/skills/` — `verify-changes`, `add-ai-tool`, `add-ui-action`, `add-llm-job`,
`cut-release`. Edit `.agents/skills/` or `.cursor/rules/`, then run
`./scripts/sync-agent-config.sh`; CI fails if copies drift.

## Don't

- Commit secrets, `.authorkit-rust/` project state, `target/` or `dist/`.
- Add an async runtime, a database, or a new GUI toolkit without an ADR in `docs/adr/`.
- Change persisted formats (`authorkit.toml`, frontmatter, `index.json`, chat JSON) without
  `#[serde(default)]` compatibility and an update to
  [docs/reference/file-formats.md](docs/reference/file-formats.md).

## Docs

Technical docs in [docs/](docs/README.md) (Diátaxis). Update the relevant page in the same PR
as the code. Known code/doc gaps: [docs/reference/known-issues.md](docs/reference/known-issues.md).
