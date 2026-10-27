---
name: add-ai-tool
description: Add a new AI writing tool (brush) that transforms selected text in AuthorKit, with icon, tests and docs. Use when asked to add, rename or change an AI tool, brush or selection action.
---

# Add an AI tool (brush)

Full guide: `docs/guides/adding-an-ai-tool.md`.

1. In `crates/ak-llm/src/brushes.rs`, add a `Brush { id, label, instruction }` to
   `builtin_brushes()`.
   - `id`: kebab-case, stable forever (persisted in chat history as `brush:<id>`).
   - `instruction`: the transform instruction sent to the model; ask for the rewritten text only.
2. In `crates/ak-llm/src/brushes.rs` tests, add the id to the expected set in
   `builtin_brushes_are_non_empty_with_unique_expected_ids`.
3. In `apps/desktop/src/panels/sidebar.rs`, map the id in `brush_icon` to a
   `lucide` icon; keep `tool_and_brush_icons_are_distinct` passing.
4. No dispatch code is needed: selection toolbar → `EditorAction::Brush` → `stage_brush` →
   `run_brush_from_attachment` → `JobKind::Transform` → edit proposal.
5. Update the tools table in `docs/architecture/llm.md` and add a `CHANGELOG.md` entry.
6. Run the `verify-changes` skill.
