# Contributing

Thanks for helping improve AuthorKit. Product home: [SupraLab](https://supralab.fr/en).

## Language

- **Source comments, rustdoc, README, and CI docs** are written in **English**.
- The app UI ships with French and English strings (`apps/desktop/src/i18n.rs`); product copy may stay localized.

## Setup

```bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Optional: `./scripts/coverage.sh` (needs `cargo-llvm-cov`).

## Pull requests

1. Keep changes focused; match existing style.
2. Add unit tests for pure logic; avoid relying on live network LLM calls.
3. Update `CHANGELOG.md` under `[Unreleased]` when user-facing.
4. Do not commit secrets or project `.authorkit-rust/` state.
5. Update the relevant page in [`docs/`](docs/README.md) when behaviour or file formats change; significant design changes need an [ADR](docs/adr/README.md).
6. Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages.

Start with the [architecture overview](docs/architecture/overview.md) and the [development guide](docs/guides/development.md).

## AI assistants

Project rules and skills for Cursor, Claude Code and Codex live in `AGENTS.md`, `.cursor/rules/` and `.agents/skills/`. After editing them, run `./scripts/sync-agent-config.sh` (CI checks for drift). See [docs/guides/ai-assistants.md](docs/guides/ai-assistants.md).

## Releases

See [docs/guides/releasing.md](docs/guides/releasing.md). In short: bump `[workspace.package] version`, update `CHANGELOG.md`, tag `vX.Y.Z`, and push the tag. GitHub Actions builds and publishes binaries.
