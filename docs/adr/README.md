# Architecture Decision Records

Significant, hard-to-reverse decisions, recorded in Michael Nygard's format
([Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)):
**Title, Status, Context, Decision, Consequences**. See also [adr.github.io](https://adr.github.io/).

Records are immutable once accepted: to change a decision, add a new ADR that supersedes the old
one and update the old one's status.

| ADR | Title | Status |
|-----|-------|--------|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-native-egui-desktop-app.md) | Native desktop app with egui/eframe | Accepted |
| [0003](0003-local-first-plain-file-storage.md) | Local-first plain-file project storage | Accepted |
| [0004](0004-blocking-worker-threads-for-llm.md) | Blocking worker threads for LLM calls | Accepted |
| [0005](0005-openai-compatible-provider-layer.md) | Single OpenAI-compatible provider layer | Accepted |
| [0006](0006-character-based-context-budget.md) | Character-based context budget | Accepted |
| [0007](0007-ai-edits-as-proposals.md) | AI edits as reviewable, undoable proposals | Accepted |

New ADR: copy [template.md](template.md) to `NNNN-short-title.md` and add it to this table.
