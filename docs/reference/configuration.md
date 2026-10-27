# Files & configuration

## Environment variables

| Variable | Effect |
|----------|--------|
| `AUTHORKIT_HOME` | Overrides the global state directory (default `~/.authorkit-rust`). Used by tests to isolate settings and secrets. |
| `RUST_LOG` | Not used (no logging framework is wired yet). |

## Global state — `~/.authorkit-rust/`

| Path | Format | Written by | Content |
|------|--------|------------|---------|
| `ui.json` | JSON | `theme::UiPrefs::save` | Theme, panel visibility/sizes, onboarding flag, recent projects (max 8), locale |
| `llm_settings.json` | JSON | `ak_llm::LlmSettings::save` | Active profile + profiles (no keys) |
| `secrets/<account>` | plain text, `0600` | `LlmSettings::set_api_key` | One API key per credential account |

### `UiPrefs` fields

| Field | Default | Notes |
|-------|---------|-------|
| `theme` | `Light` | `Light` / `Dark` |
| `show_nav`, `show_sidebar` | `true` | |
| `nav_width` | 280 | clamped 280–420 |
| `sidebar_width` | 340 | clamped 280–520 |
| `manuscript_height` | 320 | nav split |
| `onboarding_done` | `false` | |
| `recent_projects` | `[]` | max 8 |
| `locale` | `fr` | `fr` / `en` |

### `LlmProfile` fields

| Field | Example |
|-------|---------|
| `name` | `"Par défaut"` |
| `provider` | `"openaicompatible"` / `"anthropic"` / `"ollama"` |
| `base_url` | `"https://api.openai.com/v1"` |
| `model` | `"gpt-5-mini"` |
| `keyring_account` | `"openai"` (derived; empty for Ollama) |

## Per-project state — `<project>/.authorkit-rust/`

| Path | Content |
|------|---------|
| `state.json` | `{"format_version": 1}` |
| `chat/index.json` | Active conversation per context |
| `chat/<conversation-id>.json` | Conversation (messages, composer draft/mode/attachment) |
| `versions/<scene\|note>/<doc-id>/<uuid>.json` | Version snapshots |
| `trash/<entry-id>.json` | Soft-deleted items |
| `reviews/<scene-id>.json` | Coherence review report |

This folder is added to the project's `.gitignore` on creation.

## Build & tooling configuration

| File | Purpose |
|------|---------|
| `Cargo.toml` | Workspace members, `[workspace.package]` metadata, `[workspace.dependencies]` |
| `clippy.toml` | `too-many-arguments-threshold = 10` |
| `bacon.toml` | Dev loop jobs: `check`, `run`, `release` |
| `apps/desktop/build.rs` | Embeds Windows icon/metadata via `winres` (Windows only) |
| `apps/desktop/Cargo.toml` `[package.metadata.bundle]` | Bundle id `fr.supralab.authorkit`, icons (for `cargo-bundle`) |
| `.github/workflows/ci.yml` | fmt, clippy, tests (Linux/macOS/Windows), coverage gates |
| `.github/workflows/release.yml` | Tag-triggered multi-platform build + GitHub Release |
