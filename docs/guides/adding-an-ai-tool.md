# Adding an AI tool (brush)

Tools are one-shot transformations of the selected text. They appear in the selection toolbar,
the "…" menu and the composer mode menu, and produce an [edit proposal](../architecture/chat-and-edit-proposals.md).

## Steps

1. **Declare the tool** in `crates/ak-llm/src/brushes.rs` — add a `Brush` to `builtin_brushes()`:

   ```rust
   Brush {
       id: "tighten-dialogue".into(),   // stable, kebab-case, never reused
       label: "Dialogues".into(),       // UI label (French default)
       instruction: "Resserre les dialogues…".into(), // sent as the Transform instruction
   },
   ```

   The `id` is persisted in chat history (`tool: "brush:<id>"`, `EditRegenSpec::Brush`), so treat
   it as a public identifier.

2. **Pick an icon** in `apps/desktop/src/panels/sidebar.rs` → `brush_icon(id)`, using a
   `shadcn_egui::icons::lucide` constant. `tool_icon` delegates to it, so the user bubble and the
   proposal card stay consistent.

3. **Localise** the label if it must differ in English (route through `i18n.rs`).

4. **Test**:
   - `brushes.rs` unit test already checks ids are unique — extend the expected set.
   - `sidebar.rs` test `tool_and_brush_icons_are_distinct` if you add a dedicated icon.

5. **Document** the new id in [architecture/llm.md](../architecture/llm.md#built-in-tools-brushes)
   and add a `CHANGELOG.md` entry under `[Unreleased]`.

## What you get for free

No dispatch code is needed: `EditorAction::Brush(id)` → `stage_brush` → composer
`ComposerMode::Brush { id }` → `run_brush_from_attachment` → `JobKind::Transform` with your
`instruction`, plus snapshot "avant outil", anchors, accept/regenerate/ignore.
