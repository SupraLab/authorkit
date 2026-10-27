# Adding an LLM job

Every model call goes through `ak_llm::worker::LlmWorker` so the UI never blocks.

## Decide the purpose

`JobPurpose` controls **mutual cancellation**: submitting a job cancels active jobs with the same
purpose. Reuse a purpose only if the new job should replace those jobs; otherwise add a variant
(as was done for `Title`, so titles never cancel chat turns).

## Steps

1. **(If needed) add a `JobPurpose` variant** in `crates/ak-llm/src/worker.rs`.
2. **Pick a `JobKind`**: `Complete { system, user }` covers most one-shot prompts; `Chat`,
   `Continue` and `Transform` exist for their flows. Add a kind only if the request shape differs.
3. **Build the prompt** — for scene-bound work, start from `build_request_context()` /
   `current_system_prompt()` so styles, notes and budgets apply. Put reusable prompts and parsers in
   the relevant crate (`ak-coherence` for editorial features), not in `app.rs`.
4. **Submit** from `app.rs`:

   ```rust
   let job_id = format!("myjob{}", now_ms());
   self.llm_worker.submit(LlmJob {
       ids: JobIds { job_id: job_id.clone(), purpose: JobPurpose::MyPurpose, /* project_root, … */ },
       kind: JobKind::Complete { system, user },
       profile,
       api_key,
   });
   ```

5. **Route results** in `handle_llm_event` / `apply_llm_done`: match on `ids.purpose` (or your
   job-id prefix), ignore `Token`s if you only need the final text, handle `Error` and `Cancelled`.
   Check `ids.project_root` against the open project before applying.
6. **Parse defensively** (fenced JSON, partial output) and never trust ids/status coming from the
   model.
7. **Test** with `JobKind::Mock` in `crates/ak-llm/tests/worker_mock.rs` style, and unit-test the
   parser in its crate.

## Don'ts

- Don't call `OpenAiCompatClient::complete` from UI code.
- Don't log or display raw errors without `redact_secrets`.
- Don't send the full manuscript; use `assemble` budgets.
