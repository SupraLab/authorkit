# 0006. Character-based context budget

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

Each request mixes instructions, styles, premise, linked notes, scene summaries, the current scene
or passage, and chat history. Models and tokenizers vary across providers, and a tokenizer per
provider would add heavy dependencies.

## Decision

We will budget context in **characters** (`ContextBudget`, `RequestBudget`), with per-piece caps
and a fixed fill priority (passage, scene, premise, notes, summaries). Truncated pieces keep head
and tail with an explicit omission marker. Each piece's status (included, truncated, skipped) is
reported and shown in the Context panel.

## Consequences

- Deterministic, provider-independent and testable.
- Budgets are approximate relative to model token limits; defaults are conservative.
- The Context panel can explain exactly what was sent.
- Long-thread support (roadmap) may need token-aware budgets for providers that expose them.
