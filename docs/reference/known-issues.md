# Known issues & divergences

Places where the current code behaves differently from what one might expect, or from earlier
documentation. Each item is a candidate for an issue / PR. Keep this list honest: remove an entry
in the same PR that fixes it.

| # | Area | Behaviour today | Impact |
|---|------|-----------------|--------|
| 1 | Secrets | The `keyring` crate (v3) is built **without platform backends**, so the "keychain mirror" uses keyring's in-memory mock store. The only persistent key store is `~/.authorkit-rust/secrets/<account>` (`0600`). | Keys are not in the OS keychain. Either enable `apple-native` / `windows-native` / `linux-native` features or drop the mirror. |
| 2 | Stale replies | `JobIds.text_version` is recorded but never compared. Results are dropped only when `project_root` differs; edits rely on anchor resolution, synopses on `doc_id`. | A chat reply about an older text version is still shown (acceptable), edits are protected by anchors. |
| 3 | Chat request | The current user message is already in the history when `JobKind::Chat` appends `user` again, so it is sent **twice**. Edit-proposal messages are not filtered out of the history. | Wasted budget, slightly odd prompts. |
| 4 | Conflict check | The disk hash is computed on the body only, but the check hashes the whole file (with frontmatter), so in practice any newer mtime triggers a conflict. App-side writes outside `flush_editor` (link/unlink note, rename scene) don't refresh the stored mtime. "Reload" reloads from the in-memory project. | Possible false-positive conflict dialogs. |
| 5 | Context tab | While the Context tab is visible, `refresh_context_panel` → `build_request_context` → `flush_editor` runs **every frame**, bypassing the 0.7 s debounce. | Extra disk writes while that tab is open. |
| 6 | Stop button | "Stop" in the chat (`CancelChat`) calls `LlmWorker::cancel()`, which cancels **all** jobs (edits, title, review, blueprint). | Unrelated jobs get cancelled. |
| 7 | Synopsis vs tools | Synopsis generation uses `JobPurpose::Edit`, so it cancels / is cancelled by tool and continue jobs. | Can't run both at once. |
| 8 | Duplicated logic | The desktop re-implements review / ideas prompting on the worker instead of calling `ak-coherence` sync helpers; it has its own `parse_ideas_json`. | Two places to update prompts. |
| 9 | Dead code / unused | `EditApplyMode::AppendEnd`, `EditorAction::Save` (never emitted), `editor::show_styles` / `show_publish`, `NavSelection::{Styles, Publish}` (render nothing), `chat_store::active_or_new`, `prompts/brushes/` folder, `state.json` (never read), `open_diagnostics` (counted, not displayed), `OpenAiCompatClient::{chat, transform_selection, continue_writing}`, `bootstrap_novel`. | Noise for contributors. |
| 10 | Non-atomic writes | `ui.json`, `llm_settings.json` and Markdown export use `std::fs::write`; `UiPrefs::save` ignores errors. | Small risk of truncated settings on crash. |
| 11 | Markdown export | Ignores `ExportOptions::title_page` (always writes title/author/premise). | Option has no effect for Markdown. |
| 12 | `authorkit.toml` | `[styles]` fields have no per-field `#[serde(default)]`: a `[styles]` table missing one key fails to parse. | Hand-edited files can fail to open. |
| 13 | UI blocking | Settings → "Test connection" calls `OpenAiCompatClient::complete` on the UI thread. | UI freezes during the test (up to timeout). |
| 14 | i18n | Many UI strings in `app.rs` and panels are hard-coded in French instead of going through `i18n.rs`. | English locale is partial. |
| 15 | Packaging | `apps/desktop/scripts/package-macos.sh` hard-codes an older identifier/version; the canonical script is `scripts/package-release.sh`. | Use `scripts/package-release.sh`. |
| 16 | Signing | Release binaries are not code-signed or notarised. | Gatekeeper / SmartScreen warnings. |
