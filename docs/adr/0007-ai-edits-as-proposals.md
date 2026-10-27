# 0007. AI edits as reviewable, undoable proposals

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

The author must stay in control of the text. Model output can be wrong, and the text can change
while a request is streaming.

## Decision

We will never write model output directly into a document. Tools and continue modes produce an
**edit proposal** in the chat (original, replacement, range, 40-character anchors). Accepting it
re-locates the range with `resolve_anchor`, takes a version snapshot ("avant accept IA"), applies
the change and pushes an undo entry. If the range can't be found, the proposal becomes
`Conflict` instead of being applied.

## Consequences

- Every AI change is explicit, reversible (undo and versions) and auditable in the chat history.
- An extra click is required compared to inline auto-apply.
- Anchor resolution is the staleness guard; it must stay well-tested with Unicode text.
