"""Thin wrapper around `LLMAPIAggregator` for callers that prefer a service object."""

from __future__ import annotations

from typing import Any, Dict, Iterator, List, Optional

from author_kit.core.llm_aggregator import LLMAPIAggregator


class LLMService:
    def __init__(self, aggregator: LLMAPIAggregator):
        self._agg = aggregator

    def complete(
        self,
        prompt: str,
        *,
        overrides: Optional[Dict[str, Any]] = None,
        conversation_history: Optional[List[Dict[str, str]]] = None,
    ) -> str:
        return self._agg.send_prompt_to_llm(
            prompt, overrides=overrides, conversation_history=conversation_history
        )

    def stream(
        self,
        prompt: str,
        *,
        overrides: Optional[Dict[str, Any]] = None,
        conversation_history: Optional[List[Dict[str, str]]] = None,
    ) -> Iterator[str]:
        yield from self._agg.stream_prompt_to_llm(
            prompt, overrides=overrides, conversation_history=conversation_history
        )
