# 0002. Native desktop app with egui/eframe

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

AuthorKit is a writing tool that must start fast, work offline and run on macOS, Windows and
Linux from one codebase. An earlier prototype existed as a web/TypeScript app. Options considered:
Electron/Tauri with a web UI, or a pure-Rust GUI (egui, iced, Slint).

## Decision

We will build the app as a single native Rust binary using [eframe](https://docs.rs/eframe)
(egui, immediate mode) with the `shadcn_egui` component kit. Panels are functions that draw from
state and return action enums; `AuthorKitApp` owns all state and applies actions.

## Consequences

- One language and toolchain; small binary; no JS runtime or webview.
- Immediate mode makes state flow explicit and easy to test outside painting
  (`AuthorKitApp::for_tests`).
- Rich text editing is limited to what egui's `TextEdit` offers; Markdown preview uses a custom
  layouter (`md_layout.rs`).
- Painting code is hard to unit-test; coverage gates exclude it (see
  [testing guide](../guides/testing.md)).
- No hot reload: the dev loop is rebuild + restart (`bacon`).
