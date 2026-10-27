# 0003. Local-first plain-file project storage

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

Authors must own their text, be able to back it up, sync it with any tool (Git, cloud drives) and
read it without AuthorKit. A database (SQLite) would ease queries but make the manuscript opaque.

## Decision

We will store a project as a plain folder: `authorkit.toml` for metadata, one Markdown file per
scene and note with a JSON frontmatter header, `manuscript/index.json` for order. App state
(chat, trash, versions, reviews) lives in a hidden `.authorkit-rust/` subfolder. All project writes
go through `ak_core::fsutil::write_atomic`; deletions are soft (trash) and risky operations take a
version snapshot.

## Consequences

- Files are human-readable and diff-friendly; no migration engine is needed for content.
- External edits are possible, so the app must detect conflicts (mtime/hash check).
- Queries (word counts, links) are computed by loading the whole project in memory, which is fine
  for novel-sized projects.
- Hidden app state may be synced along with the project unless users exclude `.authorkit-rust/`.
