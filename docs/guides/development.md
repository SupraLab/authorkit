# Development setup

## Prerequisites

- Rust **stable** via [rustup](https://rustup.rs/), with `rustfmt` and `clippy` components
  (`rustup component add rustfmt clippy`).
- **Linux only** — system libraries required by eframe/winit and the file dialog
  (same list as CI):

  ```bash
  sudo apt-get install -y libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
    libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
  ```

- Optional: [`bacon`](https://dystroy.org/bacon/) for the dev loop,
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) for coverage.

## Build and run

```bash
cargo run -p authorkit-desktop            # debug
cargo run -p authorkit-desktop --release  # smoother UI
```

`authorkit-desktop` is the workspace default member, so plain `cargo run` works too.

## Dev loop

egui has no hot reload without splitting the UI into a dylib, so the project uses rebuild +
restart on save:

```bash
cargo install --locked bacon
bacon run        # debug, restarts the app on each change
bacon release    # release profile
```

In the bacon TUI: `r` = run, `c` = check.

## Isolating your settings

Set `AUTHORKIT_HOME` to keep test settings and keys away from your real ones:

```bash
AUTHORKIT_HOME=/tmp/ak-home cargo run -p authorkit-desktop
```

For a provider without a key, run [Ollama](https://ollama.com/) locally and pick the Ollama
preset.

## Before pushing

Run the same checks as CI (see [testing.md](testing.md)):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/coverage.sh   # optional, mirrors the CI coverage gates
```

## Code conventions

The project follows community standards rather than house rules:

- Formatting: [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/) via `rustfmt`
  (default configuration).
- Lints: [Clippy](https://doc.rust-lang.org/clippy/) default groups, warnings denied in CI.
- API design and naming: [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
  (RFC 430 casing, `as_`/`to_`/`into_` conversions, getters without `get_`, common traits).
- Errors: one `Error` enum per library crate with
  [`thiserror`](https://docs.rs/thiserror); no `unwrap()`/`expect()` on user data outside tests.
- Changelog: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
  versions: [SemVer](https://semver.org/).
- Language: code, comments and docs in English; UI strings localised through `i18n.rs`.

Machine-readable versions of these conventions for AI assistants live in `AGENTS.md`,
`.cursor/rules/` and `.claude/rules/` — see [ai-assistants.md](ai-assistants.md).
