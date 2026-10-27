# Adding a UI action

Panels don't mutate the app; they emit actions that `app.rs` handles (see
[desktop-app.md](../architecture/desktop-app.md#action-dispatch-pattern)).

## Steps

1. **Add a variant** to the panel's action enum (`NavAction`, `EditorAction` or `SidebarAction`)
   with the data the handler needs (ids, not references):

   ```rust
   pub enum SidebarAction {
       // …
       PinMessage(String), // message id
   }
   ```

2. **Emit it** from the panel's draw code:

   ```rust
   if widgets::icon_ghost_xs(ui, &lucide::PIN, "Épingler").clicked() {
       actions.push(SidebarAction::PinMessage(msg.id.clone()));
   }
   ```

3. **Handle it** in the matching `AuthorKitApp::handle_*` method in `app.rs`. Keep the handler
   small; put reusable logic in a method or, if it is domain logic, in a `crates/` crate.

4. **Persist** if state changed (`persist_chat()`, `flush_editor()`, `manuscript.save_index()`…).
   Flush the editor before any operation that reads or rewrites the current document.

5. **Test** in `apps/desktop/tests/stabilisation.rs`:

   ```rust
   let mut app = AuthorKitApp::for_tests(dir.path());
   app.handle_sidebar(SidebarAction::PinMessage(id.clone()));
   assert!(/* expected state */);
   ```

## Checklist

- [ ] No network or long work in the handler — submit an `LlmJob` instead
  ([adding-an-llm-job.md](adding-an-llm-job.md)).
- [ ] Destructive operations go to trash or take a version snapshot.
- [ ] Strings visible to users go through `i18n.rs` when possible.
- [ ] Widgets created in loops use a stable `egui::Id` (e.g. `Id::new(("pin", &msg.id))`).
