---
name: verify-changes
description: Run the same checks as AuthorKit CI (rustfmt, clippy -D warnings, tests, optional coverage gates) and fix failures before declaring a change done. Use after editing Rust code, before committing, or when asked to verify, lint or test.
---

# Verify changes

Run from the repository root, in order. Stop and fix at the first failure.

1. Format:

   ```bash
   cargo fmt --all
   cargo fmt --all -- --check
   ```

2. Lint (CI denies warnings):

   ```bash
   cargo clippy --workspace --all-targets -- -D warnings
   ```

   Fix the cause. Only add `#[allow(clippy::…)]` with a one-line justification.

3. Tests:

   ```bash
   cargo test --workspace
   ```

   For a narrower loop: `cargo test -p <crate>` or `cargo test -p <crate> <test_name>`.

4. Coverage (when logic was added or tests changed; needs `cargo install cargo-llvm-cov`):

   ```bash
   ./scripts/coverage.sh
   ```

   If a gate fails, add tests. Never lower thresholds.

5. Agent config (when `.agents/skills/` or `.cursor/rules/` changed):

   ```bash
   ./scripts/sync-agent-config.sh --check
   ```

6. Housekeeping:
   - User-visible change → entry under `[Unreleased]` in `CHANGELOG.md`.
   - Behaviour or format change → update the page in `docs/` (and `docs/reference/known-issues.md`
     if you fixed a listed issue).
   - `git status`: no `target/`, `dist/`, `.authorkit-rust/` or secrets staged.

Report what ran and the result of each step.
