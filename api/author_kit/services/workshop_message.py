"""Augment workshop user text with context and optional RAG."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from author_kit.core.rag_index import PersistentRagIndex

_SELECTION_REPLY_HINT = (
    "\n\n[AuthorKit instruction: The Context above includes excerpt(s) selected from the project. "
    "Reply with ONLY the revised excerpt text (in the same order if several excerpts were attached), "
    "applying the user's message. Do not return the full file, scene, or surrounding unchanged text. "
    "No preamble or explanation unless the user explicitly asks for commentary.]"
)

_SELECTION_PRIMACY_OVER_REFS_HINT = (
    "\n\n[AuthorKit instruction: The Context starts with editor-selected excerpt(s), and may also "
    "include full scene and/or compendium text below for **background only**. "
    "**The selected excerpt(s) define the only passage(s) to revise** in your answer—this takes precedence "
    "over the full sheets or scenes. Do not return or rewrite entire compendium entries or scenes unless "
    "the user explicitly asks. Match the scope of the selection(s). No preamble unless requested.]"
)

_LANGUAGE_HINT = (
    "\n\n[AuthorKit — language: Reply in the same natural language as the user's instruction "
    "(the message text they wrote). If they write in French, answer entirely in French; if the context "
    "below is in another language but the user's instruction is in French, still answer in French. "
    "Do not default to English unless the user is clearly writing in English.]"
)


def _locale_reinforcement_hint(locale_tag: str) -> str:
    return (
        f"\n\n[AuthorKit — preferred locale: {locale_tag}. "
        "Use this when the instruction language is mixed or unclear.]"
    )


def build_augmented_user_content(
    user_message: str,
    *,
    extra_context: Optional[str] = None,
    use_rag: bool = False,
    workspace_root: Optional[Path] = None,
    selection_scope_reply: bool = False,
    selection_with_reference_material: bool = False,
    user_language: Optional[str] = None,
) -> str:
    """Return a single user message string for the LLM (legacy workshop `construct_message` core)."""
    augmented = user_message.strip()
    if not augmented:
        return ""

    if extra_context:
        augmented += "\n\nContext:\n" + extra_context

    if use_rag and workspace_root is not None:
        index = PersistentRagIndex(workspace_root)
        if index.is_available():
            retrieved = index.query(augmented, k=3)
            if retrieved:
                augmented += "\n[Retrieved Context]:\n" + "\n".join(retrieved)

    if selection_scope_reply and augmented:
        if selection_with_reference_material:
            augmented += _SELECTION_PRIMACY_OVER_REFS_HINT
        else:
            augmented += _SELECTION_REPLY_HINT

    if augmented:
        augmented += _LANGUAGE_HINT
        ul = (user_language or "").strip()
        if ul:
            augmented += _locale_reinforcement_hint(ul)

    return augmented
