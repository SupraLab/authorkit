"""Workshop-style augmented chat."""

from __future__ import annotations

import json
from collections.abc import Iterator
from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException, Query
from fastapi.responses import StreamingResponse

from author_kit.core import compendium_store, scene_store, workshop_store
from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.deps import get_aggregator
from author_kit.schemas import (
    ChatMessage,
    ChatResponse,
    WorkshopChatRequest,
    WorkshopMessageContext,
    WorkshopMessagesOut,
    WorkshopThreadCreate,
    WorkshopThreadOut,
    WorkshopThreadUpdate,
)
from author_kit.services.workshop_message import build_augmented_user_content

router = APIRouter(tags=["workshop"])

# Max messages to send verbatim; older content is summarized for the model.
HOT_MESSAGE_COUNT = 20


def _overrides(req: WorkshopChatRequest) -> dict:
    o: dict = {}
    if req.provider:
        o["provider"] = req.provider
    if req.model:
        o["model"] = req.model
    return o


def _summarize_message_block(
    agg: LLMAPIAggregator,
    messages: list[dict],
    overrides: dict,
    max_tokens: int = 500,
) -> str:
    if not messages:
        return ""
    lines = [f"{m['role']}: {m['content']}" for m in messages]
    conversation_text = "\n".join(lines)
    final_prompt = (
        f"Summarize the following conversation in up to {max_tokens} tokens. "
        "Do not role-play or continue the dialogue; output only a concise summary.\n\n"
        f"{conversation_text}"
    )
    o = dict(overrides) if overrides else {}
    o["max_tokens"] = max_tokens
    return agg.send_prompt_to_llm(final_prompt, overrides=o or None)


def _scene_context_blocks(workspace_root: Path, scene_uuids: list[str]) -> str | None:
    if not scene_uuids:
        return None
    parts: list[str] = []
    for uid in scene_uuids:
        uid = uid.strip()
        if not uid:
            continue
        try:
            body = scene_store.read_scene(workspace_root, uid)
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e)) from e
        label = f"Scene {uid}"
        parts.append(f"### {label}\n{body}".strip())
    if not parts:
        return None
    return "Referenced scenes:\n\n" + "\n\n---\n\n".join(parts)


def _merged_extra_context(ws: Path | None, req: WorkshopChatRequest) -> str | None:
    parts: list[str] = []
    if req.extra_context:
        parts.append(req.extra_context)
    if req.scene_uuids and ws is not None:
        block = _scene_context_blocks(ws, req.scene_uuids)
        if block:
            parts.append(block)
    if req.compendium_excerpts and ws is not None:
        data = compendium_store.load_compendium(ws)
        block = compendium_store.format_compendium_excerpts(
            data, [r.model_dump() for r in req.compendium_excerpts], ws
        )
        parts.append("Compendium excerpts:\n" + block)
    if not parts:
        return None
    return "\n\n".join(parts)


def _build_prior_from_thread(
    workspace_root: Path,
    thread_id: str,
    req: WorkshopChatRequest,
    agg: LLMAPIAggregator,
) -> list[dict] | None:
    row = workshop_store.get_thread(workspace_root, thread_id)
    if row is None:
        raise HTTPException(status_code=404, detail="thread not found")
    stored = workshop_store.list_messages(workspace_root, thread_id)
    overrides = _overrides(req)
    prior: list[dict] = []

    if len(stored) <= HOT_MESSAGE_COUNT:
        if req.system_prompt:
            prior.append({"role": "system", "content": req.system_prompt})
        for m in stored:
            prior.append({"role": m["role"], "content": m["content"]})
        return prior if prior else None

    summary_text = row.rolling_summary
    if not summary_text:
        old = stored[:-HOT_MESSAGE_COUNT]
        summary_text = _summarize_message_block(agg, old, overrides)

    sys_parts: list[str] = []
    if req.system_prompt:
        sys_parts.append(req.system_prompt)
    sys_parts.append("Summary of earlier messages:\n" + summary_text)
    prior.append({"role": "system", "content": "\n\n".join(sys_parts)})
    for m in stored[-HOT_MESSAGE_COUNT:]:
        prior.append({"role": m["role"], "content": m["content"]})
    return prior


def _refresh_thread_summary(
    workspace_root: Path,
    thread_id: str,
    agg: LLMAPIAggregator,
    req: WorkshopChatRequest,
) -> None:
    stored = workshop_store.list_messages(workspace_root, thread_id)
    overrides = _overrides(req)
    if len(stored) <= HOT_MESSAGE_COUNT:
        workshop_store.update_rolling_summary(workspace_root, thread_id, None)
        return
    old = stored[:-HOT_MESSAGE_COUNT]
    summary_text = _summarize_message_block(agg, old, overrides)
    workshop_store.update_rolling_summary(workspace_root, thread_id, summary_text)


def _context_json_for_persist(req: WorkshopChatRequest) -> str | None:
    if (
        not req.scene_uuids
        and not req.compendium_excerpts
        and not req.selection_labels
        and not req.selection_attachments
    ):
        return None
    payload = WorkshopMessageContext(
        scene_uuids=list(req.scene_uuids or []),
        compendium_excerpts=list(req.compendium_excerpts or []),
        selection_labels=list(req.selection_labels or []),
        selection_attachments=list(req.selection_attachments or []),
    )
    return json.dumps(payload.model_dump())


def _resolve_workspace(req: WorkshopChatRequest) -> Path | None:
    if not req.workspace_root:
        return None
    ws = Path(req.workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    return ws


@router.get("/v1/workshop/threads", response_model=list[WorkshopThreadOut])
def list_workshop_threads(
    workspace_root: str = Query(..., description="Absolute workspace path"),
) -> list[WorkshopThreadOut]:
    ws = Path(workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    rows = workshop_store.list_threads(ws)
    return [
        WorkshopThreadOut(
            thread_id=r.id,
            title=r.title,
            created_at=r.created_at,
            updated_at=r.updated_at,
            seed_type=r.seed_type,
            seed_ref=r.seed_ref,
        )
        for r in rows
    ]


@router.post("/v1/workshop/threads", response_model=WorkshopThreadOut)
def create_workshop_thread(body: WorkshopThreadCreate) -> WorkshopThreadOut:
    ws = Path(body.workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    tid = workshop_store.create_thread(
        ws,
        title=body.title or "",
        seed_type=body.seed_type,
        seed_ref=body.seed_ref,
    )
    row = workshop_store.get_thread(ws, tid)
    assert row is not None
    return WorkshopThreadOut(
        thread_id=row.id,
        title=row.title,
        created_at=row.created_at,
        updated_at=row.updated_at,
        seed_type=row.seed_type,
        seed_ref=row.seed_ref,
    )


@router.get("/v1/workshop/threads/{thread_id}/messages", response_model=WorkshopMessagesOut)
def get_workshop_messages(
    thread_id: str,
    workspace_root: str = Query(..., description="Absolute workspace path"),
) -> WorkshopMessagesOut:
    ws = Path(workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    if workshop_store.get_thread(ws, thread_id) is None:
        raise HTTPException(status_code=404, detail="thread not found")
    raw = workshop_store.list_messages(ws, thread_id)
    return WorkshopMessagesOut(messages=[ChatMessage(**m) for m in raw])


@router.patch("/v1/workshop/threads/{thread_id}", response_model=WorkshopThreadOut)
def patch_workshop_thread(
    thread_id: str,
    body: WorkshopThreadUpdate,
    workspace_root: str = Query(..., description="Absolute workspace path"),
) -> WorkshopThreadOut:
    ws = Path(workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    title = body.title.strip()
    if not title:
        raise HTTPException(status_code=400, detail="title required")
    if not workshop_store.rename_thread(ws, thread_id, title):
        raise HTTPException(status_code=404, detail="thread not found")
    row = workshop_store.get_thread(ws, thread_id)
    assert row is not None
    return WorkshopThreadOut(
        thread_id=row.id,
        title=row.title,
        created_at=row.created_at,
        updated_at=row.updated_at,
        seed_type=row.seed_type,
        seed_ref=row.seed_ref,
    )


@router.delete("/v1/workshop/threads/{thread_id}")
def delete_workshop_thread(
    thread_id: str,
    workspace_root: str = Query(..., description="Absolute workspace path"),
) -> dict:
    ws = Path(workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    if not workshop_store.delete_thread(ws, thread_id):
        raise HTTPException(status_code=404, detail="thread not found")
    return {"ok": True}


@router.post("/v1/workshop/chat", response_model=ChatResponse)
def workshop_chat(
    req: WorkshopChatRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
) -> ChatResponse:
    if not req.user_message.strip():
        raise HTTPException(status_code=400, detail="user_message required")
    ws = _resolve_workspace(req)
    if req.compendium_excerpts and ws is None:
        raise HTTPException(
            status_code=400,
            detail="workspace_root required when compendium_excerpts is set",
        )
    if req.scene_uuids and ws is None:
        raise HTTPException(
            status_code=400,
            detail="workspace_root required when scene_uuids is set",
        )
    if req.thread_id and ws is None:
        raise HTTPException(status_code=400, detail="workspace_root required when thread_id is set")

    extra = _merged_extra_context(ws, req)
    selection_scope = bool(req.selection_labels) or bool(req.selection_attachments)
    has_ref_material = bool(req.scene_uuids) or bool(req.compendium_excerpts)
    augmented = build_augmented_user_content(
        req.user_message,
        extra_context=extra,
        use_rag=req.use_rag,
        workspace_root=ws,
        selection_scope_reply=selection_scope,
        selection_with_reference_material=selection_scope and has_ref_material,
        user_language=req.user_language,
    )

    prior: list[dict] | None = None
    if req.thread_id and ws is not None:
        prior = _build_prior_from_thread(ws, req.thread_id, req, agg)
    elif req.conversation_history:
        prior = []
        if req.system_prompt:
            prior.append({"role": "system", "content": req.system_prompt})
        for m in req.conversation_history:
            prior.append({"role": m.role, "content": m.content})
    elif req.system_prompt:
        prior = [{"role": "system", "content": req.system_prompt}]

    text = agg.send_prompt_to_llm(
        augmented,
        overrides=_overrides(req) or None,
        conversation_history=prior,
    )

    if req.thread_id and ws is not None:
        workshop_store.append_message(
            ws,
            req.thread_id,
            "user",
            req.user_message.strip(),
            context_json=_context_json_for_persist(req),
        )
        workshop_store.append_message(ws, req.thread_id, "assistant", text)
        _refresh_thread_summary(ws, req.thread_id, agg, req)
        workshop_store.touch_thread_json_after_message(ws, req.thread_id)

    return ChatResponse(content=text)


def _workshop_sse(
    agg: LLMAPIAggregator,
    req: WorkshopChatRequest,
) -> Iterator[bytes]:
    ws = Path(req.workspace_root).expanduser().resolve() if req.workspace_root else None
    if req.workspace_root and (ws is None or not ws.is_dir()):
        yield b'data: {"error":"invalid workspace_root"}\n\n'
        return
    if req.compendium_excerpts and ws is None:
        yield b'data: {"error":"workspace_root required for compendium_excerpts"}\n\n'
        return
    if req.scene_uuids and ws is None:
        yield b'data: {"error":"workspace_root required for scene_uuids"}\n\n'
        return
    if req.thread_id and ws is None:
        yield b'data: {"error":"workspace_root required for thread_id"}\n\n'
        return

    try:
        extra = _merged_extra_context(ws, req)
    except HTTPException as e:
        yield f"data: {json.dumps({'error': e.detail})}\n\n".encode()
        return

    selection_scope = bool(req.selection_labels) or bool(req.selection_attachments)
    has_ref_material = bool(req.scene_uuids) or bool(req.compendium_excerpts)
    augmented = build_augmented_user_content(
        req.user_message,
        extra_context=extra,
        use_rag=req.use_rag,
        workspace_root=ws,
        selection_scope_reply=selection_scope,
        selection_with_reference_material=selection_scope and has_ref_material,
        user_language=req.user_language,
    )

    prior: list[dict] | None = None
    if req.thread_id and ws is not None:
        try:
            prior = _build_prior_from_thread(ws, req.thread_id, req, agg)
        except HTTPException as e:
            yield f"data: {json.dumps({'error': e.detail})}\n\n".encode()
            return
    elif req.conversation_history:
        prior = []
        if req.system_prompt:
            prior.append({"role": "system", "content": req.system_prompt})
        for m in req.conversation_history:
            prior.append({"role": m.role, "content": m.content})
    elif req.system_prompt:
        prior = [{"role": "system", "content": req.system_prompt}]

    collected: list[str] = []
    try:
        for chunk in agg.stream_prompt_to_llm(
            augmented,
            overrides=_overrides(req) or None,
            conversation_history=prior,
        ):
            collected.append(chunk)
            yield f"data: {json.dumps({'text': chunk})}\n\n".encode()
    except Exception as e:
        yield f"data: {json.dumps({'error': str(e)})}\n\n".encode()
        yield b"data: [DONE]\n\n"
        return

    full_text = "".join(collected)
    if req.thread_id and ws is not None:
        workshop_store.append_message(
            ws,
            req.thread_id,
            "user",
            req.user_message.strip(),
            context_json=_context_json_for_persist(req),
        )
        workshop_store.append_message(ws, req.thread_id, "assistant", full_text)
        _refresh_thread_summary(ws, req.thread_id, agg, req)
        workshop_store.touch_thread_json_after_message(ws, req.thread_id)

    yield b"data: [DONE]\n\n"


@router.post("/v1/workshop/chat/stream")
def workshop_stream(
    req: WorkshopChatRequest,
    agg: LLMAPIAggregator = Depends(get_aggregator),
):
    if not req.user_message.strip():
        raise HTTPException(status_code=400, detail="user_message required")

    return StreamingResponse(
        _workshop_sse(agg, req),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )
