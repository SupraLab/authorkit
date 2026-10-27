# 0004. Blocking worker threads for LLM calls

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

LLM requests take seconds and stream tokens; the egui frame loop must never block. An async
runtime (Tokio) would bring a second concurrency model into an otherwise synchronous app.

## Decision

We will run LLM requests on plain OS threads: `LlmWorker` owns a dispatcher thread and spawns one
thread per job, using `reqwest::blocking` (rustls). Communication is via `std::sync::mpsc`
(`WorkerCmd` in, `LlmEvent` out); cancellation via `Arc<AtomicBool>` checked between stream chunks.
The UI drains events each frame and requests repaints while jobs run. A `JobPurpose` groups jobs
so a new one replaces an active job of the same purpose.

## Consequences

- Simple, debuggable concurrency with no async colouring in app code.
- One thread per in-flight request is acceptable for a desktop app with a handful of jobs.
- Cancellation is cooperative: a request blocked on connect waits for the timeout.
- Introducing tool-calling loops (see [roadmap](../roadmap.md)) will run inside the same job
  thread.
