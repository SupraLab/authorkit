"""Prompt preview and prose generation."""

from __future__ import annotations

from fastapi import APIRouter, Depends, HTTPException

from author_kit.deps import get_aggregator
from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.schemas import PromptPreviewRequest, ProseGenerateRequest
from author_kit.services.prompt_assembly import preview_final_prompt

router = APIRouter(tags=["prompts"])


@router.post("/v1/prompts/preview")
def prompt_preview(req: PromptPreviewRequest) -> dict:
    try:
        text = preview_final_prompt(
            req.prompt_config,
            req.user_input,
            req.additional_vars,
            req.current_scene_text,
            req.extra_context,
        )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    return {"preview": text}


@router.post("/v1/generate/prose")
def generate_prose(
    req: ProseGenerateRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
) -> dict:
    from author_kit.services.prompt_assembly import assemble_final_prompt

    try:
        final = assemble_final_prompt(
            req.prompt_config,
            req.user_input,
            req.additional_vars,
            req.current_scene_text,
            req.extra_context,
        )
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    prompt_text = final.text
    overrides = {}
    if req.provider:
        overrides["provider"] = req.provider
    if req.model:
        overrides["model"] = req.model
    if req.temperature is not None:
        overrides["temperature"] = req.temperature
    content = agg.send_prompt_to_llm(prompt_text, overrides=overrides or None)
    return {"content": content}
