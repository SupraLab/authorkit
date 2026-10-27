---
name: add-ui-action
description: Add a user interaction to the AuthorKit egui desktop app using the panel-action-dispatch pattern, with a stabilisation test. Use when adding a button, menu item, shortcut or any UI behaviour in apps/desktop.
---

# Add a UI action

Full guide: `docs/guides/adding-a-ui-action.md`. Pattern: panels emit actions, `app.rs` applies
them.

1. Add a variant to the right enum, carrying ids/values (not references):
   `NavAction` (`panels/nav.rs`), `EditorAction` (`panels/editor.rs`),
   `SidebarAction` (`panels/sidebar.rs`) or `OnboardingAction` (`panels/onboarding.rs`). The
   welcome screen uses a `WelcomeAction` struct of flags instead (`panels/welcome.rs`).
2. Emit it in the panel draw code (`actions.push(...)`). Use `widgets.rs` helpers, `theme.rs`
   tokens, stable `egui::Id`s in loops, and `i18n::tr(lang, fr, en)` for labels.
3. Handle it in the matching `AuthorKitApp::handle_*` in `apps/desktop/src/app.rs`:
   - call `flush_editor()` first if the current document is read or rewritten;
   - persist changes (`persist_chat()`, manuscript save, …);
   - destructive → trash; text rewrite → version snapshot;
   - anything slow or networked → submit an `LlmJob` (see `add-llm-job` skill).
4. Add a test in `apps/desktop/tests/stabilisation.rs` using `AuthorKitApp::for_tests(dir)` and
   the `handle_*` method; assert resulting state and files.
5. Run the `verify-changes` skill.
