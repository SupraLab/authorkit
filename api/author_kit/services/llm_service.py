"""Thin wrapper around `LLMAPIAggregator` for callers that prefer a service object."""

from __future__ import annotations

from collections.abc import Iterator
from typing import Any

from author_kit.core.llm_aggregator import LLMAPIAggregator


class LLMService:
    def __init__(self, aggregator: LLMAPIAggregator):
        self._agg = aggregator

    def complete(
        self,
        prompt: str,
        *,
        overrides: dict[str, Any] | None = None,
        conversation_history: list[dict[str, str]] | None = None,
    ) -> str:
        return self._agg.send_prompt_to_llm(
            prompt, overrides=overrides, conversation_history=conversation_history
        )

    def stream(
        self,
        prompt: str,
        *,
        overrides: dict[str, Any] | None = None,
        conversation_history: list[dict[str, str]] | None = None,
    ) -> Iterator[str]:
        yield from self._agg.stream_prompt_to_llm(
            prompt, overrides=overrides, conversation_history=conversation_history
        )
