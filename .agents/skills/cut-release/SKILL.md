---
name: cut-release
description: Prepare and tag an AuthorKit release (SemVer bump, Keep a Changelog section, annotated tag that triggers the multi-OS GitHub release workflow). Use when asked to release, publish a version, or bump the version.
---

# Cut a release

Full guide: `docs/guides/releasing.md`.

1. Confirm `main` is clean and green: run the `verify-changes` skill, including coverage.
2. Pick the version per [SemVer](https://semver.org/) (pre-1.0: minor may break).
3. Set `version = "X.Y.Z"` under `[workspace.package]` in the root `Cargo.toml`; run
   `cargo check --workspace` so `Cargo.lock` updates.
4. `CHANGELOG.md` ([Keep a Changelog](https://keepachangelog.com/en/1.1.0/)):
   - rename `## [Unreleased]` content into `## [X.Y.Z] — YYYY-MM-DD`;
   - add a fresh empty `## [Unreleased]`;
   - update the link references at the bottom.
   The release workflow extracts this section as release notes, so the heading must match.
5. Optional local smoke test: `./scripts/package-release.sh X.Y.Z` → `dist/`.
6. Commit `chore(release): vX.Y.Z`, then **ask the user before pushing**:

   ```bash
   git tag -a vX.Y.Z -m "AuthorKit vX.Y.Z"
   git push origin main vX.Y.Z
   ```

7. `.github/workflows/release.yml` builds macOS (arm64, x64), Windows (x64) and Linux (x64) and
   publishes the GitHub Release. Check the run with `gh run list --workflow release.yml`.
