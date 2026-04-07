"""Conversation summarization (LLM-based)."""

from __future__ import annotations

from fastapi import APIRouter, Depends, HTTPException

from author_kit.deps import get_aggregator
from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.schemas import SummarizeRequest, SummarizeResponse

router = APIRouter(tags=["conversation"])


@router.post("/v1/conversation/summarize", response_model=SummarizeResponse)
def summarize_conversation(
    req: SummarizeRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
) -> SummarizeResponse:
    if not req.messages:
        raise HTTPException(status_code=400, detail="messages must not be empty")

    lines = []
    for m in req.messages:
        lines.append(f'{m.role}: {m.content}')
    conversation_text = "\n".join(lines)

    final_prompt = (
        f"Summarize the following conversation in up to {req.max_tokens} tokens. "
        "Do not role-play or continue the dialogue; output only a concise summary.\n\n"
        f"{conversation_text}"
    )
    overrides: dict = {}
    if req.provider:
        overrides["provider"] = req.provider
    if req.model:
        overrides["model"] = req.model
    if req.temperature is not None:
        overrides["temperature"] = req.temperature
    overrides["max_tokens"] = req.max_tokens

    summary = agg.send_prompt_to_llm(final_prompt, overrides=overrides or None)
    return SummarizeResponse(summary=summary)
