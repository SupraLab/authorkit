# AuthorKit documentation

Technical documentation for AuthorKit, a local-first desktop writing studio written in Rust (egui / eframe).

The documentation is organised following the [Diátaxis](https://diataxis.fr/) framework:

| Section | Purpose | Start here |
|---------|---------|------------|
| **Explanation** — [`architecture/`](architecture/) | How the system is built and why | [Architecture overview](architecture/overview.md) |
| **How-to guides** — [`guides/`](guides/) | Task-oriented recipes for contributors | [Development setup](guides/development.md) |
| **Reference** — [`reference/`](reference/) | Exact paths, formats, enums, settings | [Files & configuration](reference/configuration.md) |
| **Decisions** — [`adr/`](adr/) | Architecture Decision Records (Nygard format) | [ADR index](adr/README.md) |

## Map

### Architecture (explanation)

- [Overview](architecture/overview.md) — crates, dependency graph, runtime model
- [Desktop app](architecture/desktop-app.md) — frame loop, panels, action dispatch
- [Data model & storage](architecture/data-model.md) — project layout, file formats
- [Persistence & safety](architecture/persistence-and-safety.md) — saves, conflicts, trash, versions
- [LLM layer](architecture/llm.md) — providers, secrets, HTTP client, background worker
- [Context assembly](architecture/context-assembly.md) — what the model receives, budgets
- [Chat, tools & edit proposals](architecture/chat-and-edit-proposals.md) — end-to-end flows
- [Coherence](architecture/coherence.md) — reviews, ideas, novel blueprints
- [Export](architecture/export.md) — Markdown, EPUB 3, DOCX pipeline

### Guides (how-to)

- [Development setup](guides/development.md)
- [Testing & coverage](guides/testing.md)
- [Adding an AI tool (brush)](guides/adding-an-ai-tool.md)
- [Adding a UI action](guides/adding-a-ui-action.md)
- [Adding an LLM job](guides/adding-an-llm-job.md)
- [Releasing](guides/releasing.md)
- [AI coding assistants](guides/ai-assistants.md) — Cursor, Claude Code, Codex setup

### Reference

- [Files & configuration](reference/configuration.md)
- [File formats](reference/file-formats.md)
- [Known issues & divergences](reference/known-issues.md)
- [Glossary](reference/glossary.md)

### Project

- [Roadmap](roadmap.md)
- [Changelog](../CHANGELOG.md)
- [Contributing](../CONTRIBUTING.md)

## Conventions used in these docs

- Diagrams use [Mermaid](https://mermaid.js.org/) (rendered natively by GitHub).
- Paths are relative to the repository root unless stated otherwise.
- Code identifiers are written exactly as in source (`AuthorKitApp`, `JobPurpose::Edit`).
- When the code and these docs disagree, the code wins: open an issue or fix the doc. Known
  divergences are tracked in [known-issues.md](reference/known-issues.md).
