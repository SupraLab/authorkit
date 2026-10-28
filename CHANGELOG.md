# Changelog

All notable changes to AuthorKit are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- A long manuscript in the left nav no longer paints over the Univers list. Selecting a scene or note scrolls it into view once, without overriding a later manual scroll.

## [0.0.1] — 2026-10-27

First public preview release.

### Added

- Native desktop writing studio (egui / eframe) for macOS, Windows, and Linux
- Local-first projects: manuscript (acts / chapters / scenes), linked Notes, writing styles
- Sidebar chat with scene context, tools, continue modes, and edit proposals
- Context panel showing what the model receives (budget, pieces, previews)
- BYOK LLM settings (OpenAI-compatible, Anthropic, Ollama, common hosts)
- Coherence reviews, content ideas, novel blueprint bootstrap
- Export to Markdown, EPUB 3, and DOCX
- Trash, scene versions, atomic saves, secret redaction
- CI: tests, fmt, clippy, coverage gates; tagged releases with platform binaries and icons
- Technical documentation in `docs/` (architecture, guides, reference, ADRs)
- AI assistant configuration: `AGENTS.md`, `CLAUDE.md`, Cursor / Claude Code rules and shared skills

### Planned (next major)

- Tool-calling agent loop for manuscript/notes actions
- Long-thread support when providers allow it; smarter summarization otherwise
- Richer context assembly tied to tools and the Context panel

[Unreleased]: https://github.com/SupraLab/authorkit/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/SupraLab/authorkit/releases/tag/v0.0.1
