# 0005. Single OpenAI-compatible provider layer

- **Status:** Accepted
- **Date:** 2026-10-27

## Context

Users bring their own key (BYOK) and choose between hosted providers (OpenAI, Mistral,
OpenRouter, Groq, DeepSeek…) and local runtimes (Ollama, LM Studio). Most expose the OpenAI
Chat Completions API.

## Decision

We will implement one HTTP client, `OpenAiCompatClient`, targeting `/chat/completions` with SSE
streaming, and model providers as presets (`base_url`, default model, whether a key is required).
Keys are stored outside projects under the user's home and redacted from every surfaced error.

## Consequences

- Adding a provider is usually a preset entry, not code.
- Provider-specific features (native tool calling, prompt caching, server-side threads, Anthropic's
  Messages API) are not used yet; the roadmap will require per-provider capabilities on top of
  this layer.
- Model-specific quirks (e.g. temperature restrictions) are handled in small helpers.
