"""Pydantic request/response models."""

from __future__ import annotations

from typing import Any, Dict, List, Literal, Optional

from pydantic import BaseModel, Field


class CompendiumExcerptRef(BaseModel):
    category: str
    name: str = Field(..., description="Entry name within the category")


class SelectionAttachment(BaseModel):
    """Editor selection resolved to a scene sheet, compendium entry, or generic file."""

    label: str = Field(..., description="Display title (scene name, entry name, or file stem)")
    kind: Literal["scene", "compendium", "file"] = "file"
    scene_uuid: Optional[str] = None
    compendium_category: Optional[str] = None
    compendium_name: Optional[str] = None
    range_label: Optional[str] = Field(
        None,
        description="Visible span (e.g. line + char offsets) to disambiguate multiple excerpts.",
    )


class WorkshopMessageContext(BaseModel):
    """Structured context attached to a user turn (persisted with the message)."""

    scene_uuids: List[str] = Field(default_factory=list)
    compendium_excerpts: List[CompendiumExcerptRef] = Field(default_factory=list)
    selection_labels: List[str] = Field(
        default_factory=list,
        description="Short labels for editor-attached selections (full text is in extra_context for the model only).",
    )
    selection_attachments: List[SelectionAttachment] = Field(
        default_factory=list,
        description="Structured selection targets for UI tags and insert actions.",
    )


class ChatMessage(BaseModel):
    role: str
    content: str
    context: Optional[WorkshopMessageContext] = Field(
        None,
        description="Scene + compendium refs for user messages; omitted for assistant.",
    )


class ChatRequest(BaseModel):
    messages: List[ChatMessage] = Field(default_factory=list)
    provider: Optional[str] = None
    model: Optional[str] = None
    temperature: Optional[float] = None
    max_tokens: Optional[int] = None
    api_key: Optional[str] = None
    workspace_root: Optional[str] = Field(
        None, description="Optional; same as X-Workspace-Root header"
    )


class ChatResponse(BaseModel):
    content: str


class PromptPreviewRequest(BaseModel):
    prompt_config: Dict[str, Any]
    user_input: str
    additional_vars: Optional[Dict[str, str]] = None
    current_scene_text: Optional[str] = None
    extra_context: Optional[str] = None
    workspace_root: Optional[str] = None


class ProseGenerateRequest(BaseModel):
    prompt_config: Dict[str, Any]
    user_input: str
    additional_vars: Optional[Dict[str, str]] = None
    current_scene_text: Optional[str] = None
    extra_context: Optional[str] = None
    provider: Optional[str] = None
    model: Optional[str] = None
    temperature: Optional[float] = None
    workspace_root: Optional[str] = None


class WorkshopChatRequest(BaseModel):
    user_message: str
    extra_context: Optional[str] = None
    system_prompt: str = ""
    conversation_history: List[ChatMessage] = Field(default_factory=list)
    use_rag: bool = False
    compendium_excerpts: List[CompendiumExcerptRef] = Field(
        default_factory=list,
        description="Optional compendium entries to inject as context",
    )
    provider: Optional[str] = None
    model: Optional[str] = None
    workspace_root: Optional[str] = None
    thread_id: Optional[str] = Field(
        None,
        description="When set, server loads conversation history from SQLite for this thread",
    )
    scene_uuids: List[str] = Field(
        default_factory=list,
        description="Scene UUIDs; server resolves prose from .authorkit/scenes/{uuid}.md",
    )
    selection_labels: List[str] = Field(
        default_factory=list,
        description="Labels for editor selection context (persisted for thread UI; text is in extra_context).",
    )
    selection_attachments: List[SelectionAttachment] = Field(
        default_factory=list,
        description="Resolved selection targets (scene/compendium/file) for UI and insert.",
    )
    user_language: Optional[str] = Field(
        None,
        description="Client UI locale (e.g. fr, en-US) to reinforce reply language for the model.",
    )


class WorkshopThreadCreate(BaseModel):
    workspace_root: str
    title: Optional[str] = None
    seed_type: Optional[str] = Field(
        None, description="scene | character | world | none"
    )
    seed_ref: Optional[str] = Field(None, description="Scene UUID or compendium key")


class WorkshopThreadOut(BaseModel):
    thread_id: str
    title: str
    created_at: str
    updated_at: str
    seed_type: Optional[str] = None
    seed_ref: Optional[str] = None


class WorkshopThreadUpdate(BaseModel):
    title: str = Field(..., min_length=1, max_length=240, description="New display name for the thread")


class WorkshopMessagesOut(BaseModel):
    messages: List[ChatMessage]


class RagIndexRequest(BaseModel):
    chunks: List[str] = Field(..., min_length=1)
    workspace_root: str
    embedding_backend: str = Field(
        "tiktoken",
        description="`tiktoken` (offline) or `openai` (requires OPENAI_API_KEY)",
    )


class RagQueryRequest(BaseModel):
    query: str
    k: int = 3
    workspace_root: str


class RagQueryResponse(BaseModel):
    chunks: List[str]


class SummarizeRequest(BaseModel):
    messages: List[ChatMessage]
    max_tokens: int = 500
    provider: Optional[str] = None
    model: Optional[str] = None
    temperature: Optional[float] = None


class SummarizeResponse(BaseModel):
    summary: str
