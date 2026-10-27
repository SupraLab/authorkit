---
name: add-llm-job
description: Add a new background LLM request to AuthorKit through LlmWorker (purpose, job kind, prompt, result routing, mock tests). Use when a feature needs to call the model, e.g. a new generation, analysis or summary.
---

# Add an LLM job

Full guide: `docs/guides/adding-an-llm-job.md`.

1. Choose `JobPurpose` (`crates/ak-llm/src/worker.rs`). Same purpose = mutual cancellation.
   Add a variant if the job must not cancel or be cancelled by existing ones.
2. Choose `JobKind`; prefer `Complete { system, user }` for one-shot prompts.
3. Build prompts:
   - scene-aware → start from `build_request_context()` / `current_system_prompt()` in `app.rs`
     so styles, notes and budgets apply;
   - reusable prompt + parser → put them in the owning crate (`ak-coherence` for editorial
     features) with unit tests.
4. Submit `LlmJob { ids: JobIds { job_id, project_root, conversation_id, doc_id, text_version,
   purpose }, kind, profile, api_key }` via `self.llm_worker.submit(...)`.
5. Route results in `handle_llm_event` / `apply_llm_done`: match the purpose, drop results whose
   `project_root` isn't the open project, handle `Error` (already redacted) and `Cancelled`.
6. Never call `OpenAiCompatClient` from UI code; never log keys.
7. Tests: parser unit tests + a `JobKind::Mock` worker test (see
   `crates/ak-llm/tests/worker_mock.rs`).
8. Update `docs/architecture/llm.md` (types table) and run the `verify-changes` skill.
