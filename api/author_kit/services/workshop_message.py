"""Augment workshop user text with context and optional RAG."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from author_kit.core.rag_index import PersistentRagIndex


def build_augmented_user_content(
    user_message: str,
    *,
    extra_context: Optional[str] = None,
    use_rag: bool = False,
    workspace_root: Optional[Path] = None,
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

    return augmented
