# Testing & coverage

## Commands

```bash
cargo test --workspace                         # all unit + integration tests
cargo test -p ak-core                          # one crate
cargo test -p authorkit-desktop --test stabilisation
./scripts/coverage.sh                          # coverage gates (needs cargo-llvm-cov)
```

## Layout

Rust's standard layout is used ([Cargo book: tests](https://doc.rust-lang.org/cargo/guide/tests.html)):

- **Unit tests** in `#[cfg(test)] mod tests` at the bottom of the source file, for pure logic.
- **Integration tests** in `<crate>/tests/*.rs`, using only the public API.

| File | Covers |
|------|--------|
| `crates/ak-core/tests/project_smoke.rs` | Create, note, link, reopen |
| `crates/ak-core/tests/manuscript_ops.rs` | Rename, link, moves, word counts, versions |
| `crates/ak-core/tests/persistence_safety.rs` | Create refusal, atomic write, trash order, version safety copy |
| `crates/ak-core/tests/trash_ops.rs` | Trash / restore / purge for every kind |
| `crates/ak-core/tests/diagnostics_ops.rs` | Every diagnostic code |
| `crates/ak-llm/tests/worker_mock.rs` | Worker lifecycle with `JobKind::Mock`, cancellation, error redaction |
| `crates/ak-export/tests/export_smoke.rs`, `export_content.rs` | Unzipped DOCX / EPUB structure, options, text parity |
| `apps/desktop/tests/stabilisation.rs` | App-level flows via `AuthorKitApp::for_tests` (anchors, undo, chat routing, proposals, staging) |
| `apps/desktop/tests/unicode_selection.rs` | Char-index selection with accents |

## Rules for tests

- **No network.** Use `JobKind::Mock` for worker tests; never call a real provider.
- **No real home directory.** Set `AUTHORKIT_HOME` to a temp dir (settings tests serialise access
  with a mutex because env vars are process-global). `AuthorKitApp::for_tests` does this for you.
- **Temp projects** with [`tempfile`](https://docs.rs/tempfile) + `Project::create`.
- **Deterministic**: no sleeps for synchronisation except bounded polling of worker events.
- Text positions are **char indices**, not byte offsets — include non-ASCII text in tests that
  touch selections.

## Coverage gates

CI runs `cargo llvm-cov` with per-crate line thresholds:

| Target | Gate | Excluded |
|--------|------|----------|
| `ak-core` | 85 % | — |
| `ak-export` | 95 % | — |
| `ak-coherence` | 80 % | — |
| `ak-llm` | 85 % | `openai_compat.rs` (live HTTP) |
| `authorkit-desktop --lib` | 74 % | egui paint modules (`app`, panels, widgets, theme, brand, i18n, md_layout, providers) |

The intent is **full unit coverage of pure logic**; egui painting and live HTTP are covered by
integration tests and manual review rather than line coverage. Don't lower a gate to make CI
pass — add tests.
