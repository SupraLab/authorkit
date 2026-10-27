<p align="center">
  <img src="apps/desktop/assets/brand/authorkit.png" alt="AuthorKit" width="520" />
</p>

<p align="center">
  <a href="https://github.com/SupraLab/authorkit/actions/workflows/ci.yml"><img src="https://github.com/SupraLab/authorkit/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/SupraLab/authorkit/releases"><img src="https://img.shields.io/github/v/release/SupraLab/authorkit?include_prereleases" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License" /></a>
</p>

# AuthorKit

Local-first writing studio for storytellers — **BYOK**, **100% Rust** (egui / eframe).

Built by [SupraLab](https://supralab.fr/en). Inspired by [Type.ai](https://type.ai/) (draft + chat + Notes + tools), as a native desktop app.

## Status — v0.0.1

AuthorKit is an **early public preview**. The core loop works end-to-end:

- Manuscript editing (acts / chapters / scenes) with Markdown formatting
- Linked Notes (characters, settings, lore) and project writing styles
- Sidebar chat with scene context, tools (“brushes”), continue modes, and edit proposals
- Context panel that shows what the model actually receives
- BYOK providers (OpenAI-compatible, Anthropic, Ollama, and common hosts)
- Coherence reviews, content ideas, novel blueprint bootstrap
- Export to Markdown, EPUB 3, and DOCX
- Local project layout, trash, versions, and atomic saves

Expect rough edges in the UI and packaging. macOS / Windows / Linux binaries are published from CI; **code signing and notarization are not included yet** (Gatekeeper / SmartScreen warnings are expected).

### Roadmap — agent context & long threads (next major evolution)

The next major milestone focuses on how AuthorKit talks to the model:

1. **Tool-calling agent loop** — expose manuscript/notes actions as real provider tools (read scene, link note, propose edit, search project…) instead of prompt-only “brushes”, so the model can request structured work mid-turn.
2. **Long-thread support** — when the provider allows it (native long context, prompt caching, or server-side threads), keep multi-turn chats coherent without aggressive history drops; otherwise fall back to smarter summarization + the existing character budget.
3. **Richer context assembly** — tighter coupling between the Context panel, tool results, and what gets sent each turn (citations, selective include/exclude, per-tool budgets).

Tracking issue / design notes will land with that milestone; v0.0.1 ships the current assemble-and-chat foundation. See [docs/roadmap.md](docs/roadmap.md).

## Documentation

Technical docs live in [`docs/`](docs/README.md): architecture (with Mermaid diagrams), how-to guides, reference (file formats, configuration, known issues) and Architecture Decision Records.

AI coding assistants (Cursor, Claude Code, Codex) get project rules and skills from [`AGENTS.md`](AGENTS.md), [`CLAUDE.md`](CLAUDE.md), `.cursor/`, `.claude/` and `.agents/` — see [docs/guides/ai-assistants.md](docs/guides/ai-assistants.md).

## Stack

| Crate | Role |
|-------|------|
| `apps/desktop` | egui UI |
| `crates/ak-core` | Project, manuscript, Notes, links, styles |
| `crates/ak-llm` | BYOK providers + context assembly + worker |
| `crates/ak-coherence` | Reviews, content ideas, novel blueprint |
| `crates/ak-export` | Markdown, EPUB, DOCX |

## Run

```bash
cargo run -p authorkit-desktop --release
```

### Dev loop

egui has no true hot-reload without splitting the UI into a dylib. In practice, rebuild + restart on save with [bacon](https://dystroy.org/bacon/):

```bash
cargo install --locked bacon   # once
bacon run                      # debug, auto-restart
# bacon release                # slower compile, smoother runtime
```

In the bacon TUI: `r` = run, `c` = check. The previous window is killed before restart (`kill_then_restart`).

## Project on disk

```
my-novel/
  authorkit.toml
  manuscript/index.json
  manuscript/scenes/*.md
  notes/characters|settings|lore/*.md
  prompts/brushes/
  .authorkit-rust/          # local app state (gitignored)
    versions/ trash/ chat/ reviews/
```

## Configuration & security

- **LLM settings** (global): `~/.authorkit-rust/llm_settings.json` — never stores API keys.
- **API keys (BYOK)**: stored in `~/.authorkit-rust/secrets/<account>` (`0600` on Unix), never inside projects. An OS-keychain mirror is wired through the `keyring` crate but no platform backend is enabled yet, so the file is the only persistent store (see [known issues](docs/reference/known-issues.md)). Errors, toasts, and `Debug` never print secrets (`redact_secrets`).
- **`SaveStatus`**: `Clean` → `Unsaved` → `Saving` → `Saved` / `Failed(msg)`. Failures keep the document in memory, block project switch / export, and refuse to overwrite if the file changed on disk. Writes are atomic (`write_atomic`).
- **Trash**: deleting a scene, chapter, act, or note moves it to `.authorkit-rust/trash/` (restorable). Restoring a version keeps a safety copy of the current state.
- **LLM worker** (`ak_llm::LlmWorker`): calls run on a dedicated thread; each job carries `JobIds` (project, conversation, document, text version); replies for another project are dropped and AI edits are re-anchored before applying. Conversations live under `.authorkit-rust/chat/`.
- **Reviews**: `.authorkit-rust/reviews/<scene>.json` (ToDo / Done / Ignored; marked *stale* when scene text changes).
- **Exports**: Markdown, EPUB 3, and DOCX share one pipeline (`ExportOptions`). HTML comments `<!-- -->` and `%% %%` are stripped.
- **AI context**: `ak_llm::assemble` (character budget, instructions vs documentary text, included / truncated / omitted pieces for the UI).

## Icons

Platform assets live in `apps/desktop/assets/brand/`:

| File | Use |
|------|-----|
| `icon-*.png` | Window icon + Linux sizes |
| `AuthorKit.icns` | macOS `.app` bundle |
| `authorkit.ico` | Windows executable / installer |
| `mark-*.png` | In-app quill mark |

Regenerate from `icon-512.png` with `./scripts/gen-icons.sh` (requires Pillow in a venv, plus `sips`/`iconutil` on macOS).

## Tests & coverage

```bash
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/coverage.sh          # local CI coverage gates (needs cargo-llvm-cov)
```

Goal: **unit-test all pure logic**, with CI line-coverage gates on domain crates.

| Target | Line gate | Notes |
|--------|-----------|--------|
| `ak-core` | ≥ 85% | Project, manuscript, notes, trash, versions, diagnostics |
| `ak-export` | ≥ 95% | Parse + Markdown / EPUB / DOCX |
| `ak-coherence` | ≥ 80% | Parse/apply reviews & blueprints (no live network) |
| `ak-llm` | ≥ 85% | Context, SSE, file settings, mock worker — **excl.** HTTP client (`openai_compat.rs`) |
| Desktop helpers | ≥ 74% | `chat_store`, `ms_tools`, `chat_md` — **excl.** egui paint |

Pure sidebar helpers are fully unit-tested; egui paint in the same files is excluded from the gate.

**Intentionally out of the gate:** egui paint (`app`, panels, widgets, theme, brand), live HTTP (`openai_compat`). Covered by targeted integration tests (`stabilisation`, `unicode_selection`) and manual UI review.

## Release

Tags `v*` (e.g. `v0.0.1`) trigger [.github/workflows/release.yml](.github/workflows/release.yml):

1. Build release binaries on macOS (arm64 + x64), Windows (x64), Linux (x64)
2. Package with platform icons (`.app` + `.icns` on macOS, `.ico` on Windows, PNG + `.desktop` on Linux)
3. Attach archives to a GitHub Release; changelog body from `CHANGELOG.md`

```bash
# After updating CHANGELOG.md and Cargo.toml version:
git tag -a v0.0.1 -m "AuthorKit v0.0.1"
git push origin v0.0.1
```

## License

[MIT](LICENSE) — Copyright © 2026 Julien Moulin / [SupraLab](https://supralab.fr/en)

Bundled IBM Plex fonts (`apps/desktop/assets/fonts/`) are licensed under the [SIL Open Font License 1.1](apps/desktop/assets/fonts/OFL.txt).
