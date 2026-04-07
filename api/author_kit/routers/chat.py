"""Raw chat completion (sync + SSE)."""

from __future__ import annotations

import json
from typing import Iterator, List, Optional

from fastapi import APIRouter, Depends, HTTPException
from fastapi.responses import StreamingResponse

from author_kit.deps import get_aggregator
from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.schemas import ChatRequest, ChatResponse

router = APIRouter(tags=["chat"])


def _overrides_from_request(req: ChatRequest) -> dict:
    o: dict = {}
    if req.provider is not None:
        o["provider"] = req.provider
    if req.model is not None:
        o["model"] = req.model
    if req.temperature is not None:
        o["temperature"] = req.temperature
    if req.max_tokens is not None:
        o["max_tokens"] = req.max_tokens
    if req.api_key is not None:
        o["api_key"] = req.api_key
    return o


@router.post("/v1/chat", response_model=ChatResponse)
def chat_completion(
    req: ChatRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
) -> ChatResponse:
    messages = [{"role": m.role, "content": m.content} for m in req.messages]
    if not messages:
        raise HTTPException(status_code=400, detail="messages must not be empty")
    last = messages[-1]
    if last.get("role") != "user":
        raise HTTPException(status_code=400, detail="last message must have role user")
    prior: Optional[List[dict]] = messages[:-1] if len(messages) > 1 else None
    overrides = _overrides_from_request(req)
    text = agg.send_prompt_to_llm(
        last["content"],
        overrides=overrides or None,
        conversation_history=prior,
    )
    return ChatResponse(content=text)


def _sse_chunks(agg: LLMAPIAggregator, req: ChatRequest) -> Iterator[bytes]:
    messages = [{"role": m.role, "content": m.content} for m in req.messages]
    if not messages:
        yield b"data: {\"error\":\"empty messages\"}\n\n"
        return
    last = messages[-1]
    if last.get("role") != "user":
        yield b"data: {\"error\":\"last message must be user\"}\n\n"
        return
    prior = messages[:-1] if len(messages) > 1 else None
    overrides = _overrides_from_request(req)
    try:
        for chunk in agg.stream_prompt_to_llm(
            last["content"],
            overrides=overrides or None,
            conversation_history=prior,
        ):
            yield f"data: {json.dumps({'text': chunk})}\n\n".encode()
    except Exception as e:
        yield f"data: {json.dumps({'error': str(e)})}\n\n".encode()
    yield b"data: [DONE]\n\n"


@router.post("/v1/chat/stream")
def chat_stream(
    req: ChatRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
):
    return StreamingResponse(
        _sse_chunks(agg, req),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )
