"""Pluggable text embeddings for RAG (tiktoken baseline vs OpenAI)."""

from __future__ import annotations

import os
from typing import Callable, Tuple

import numpy as np
import tiktoken

TIKTOKEN_DIM = 128


def _tiktoken_embed(text: str) -> np.ndarray:
    encoding = tiktoken.encoding_for_model("gpt-3.5-turbo")
    token_ids = encoding.encode(text)
    vector = np.array(token_ids, dtype=np.float32)
    if vector.shape[0] < TIKTOKEN_DIM:
        vector = np.pad(vector, (0, TIKTOKEN_DIM - vector.shape[0]), "constant")
    else:
        vector = vector[:TIKTOKEN_DIM]
    return vector


def get_embedding_fn(backend: str) -> Tuple[int, Callable[[str], np.ndarray]]:
    """Return (vector_dim, embed_fn)."""
    b = backend.lower().strip()
    if b == "openai":
        from langchain_openai import OpenAIEmbeddings

        key = os.environ.get("OPENAI_API_KEY", "")
        if not key:
            raise RuntimeError("OPENAI_API_KEY is required for OpenAI embeddings")
        model = os.environ.get("AUTHORKIT_OPENAI_EMBEDDING_MODEL", "text-embedding-3-small")
        emb = OpenAIEmbeddings(model=model, openai_api_key=key)
        v0 = np.array(emb.embed_query("probe"), dtype=np.float32)
        dim = int(v0.shape[0])

        def fn(t: str) -> np.ndarray:
            return np.array(emb.embed_query(t), dtype=np.float32)

        return dim, fn

    def fn_t(t: str) -> np.ndarray:
        return _tiktoken_embed(t)

    return TIKTOKEN_DIM, fn_t
