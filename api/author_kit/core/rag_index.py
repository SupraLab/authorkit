"""FAISS-backed RAG; embeddings via tiktoken (default) or OpenAI (`embedding_backend`)."""

from __future__ import annotations

import json
import pickle
from pathlib import Path
from typing import Callable, List, Optional

import faiss
import numpy as np

from author_kit.core.embeddings import get_embedding_fn
from author_kit.core.paths import rag_dir

META_NAME = "meta.json"


class EmbeddingIndex:
    """In-memory FAISS index with a fixed dimension and embed function."""

    def __init__(
        self,
        dim: int,
        embed_fn: Callable[[str], np.ndarray],
        index: Optional["faiss.Index"] = None,
    ):
        self.dim = dim
        self.embed_fn = embed_fn
        self.index = index if index is not None else faiss.IndexFlatL2(dim)
        self.texts: List[str] = []

    def add_text(self, text: str) -> None:
        vector = self.embed_fn(text)
        if vector.shape[0] != self.dim:
            raise ValueError(f"embedding dim {vector.shape[0]} != index dim {self.dim}")
        self.index.add(np.expand_dims(vector, axis=0))
        self.texts.append(text)

    def query(self, text: str, k: int = 3) -> List[str]:
        if self.index.ntotal == 0:
            return []
        vector = self.embed_fn(text)
        kk = min(k, self.index.ntotal)
        distances, indices = self.index.search(np.expand_dims(vector, axis=0), kk)
        results: List[str] = []
        for idx in indices[0]:
            if idx == -1:
                continue
            if idx < len(self.texts):
                results.append(self.texts[int(idx)])
        return results


class PersistentRagIndex:
    """Load/save FAISS + metadata under `.authorkit/rag/`."""

    def __init__(self, workspace_root: Path):
        self.workspace_root = workspace_root
        self._dir = rag_dir(workspace_root)
        self._index_path = self._dir / "faiss.index"
        self._texts_path = self._dir / "texts.json"
        self._meta_path = self._dir / META_NAME
        self._cache: Optional[EmbeddingIndex] = None

    def is_available(self) -> bool:
        return self._index_path.is_file() and self._texts_path.is_file()

    def _read_meta(self) -> dict:
        if self._meta_path.is_file():
            with open(self._meta_path, encoding="utf-8") as f:
                return json.load(f)
        return {"embedding_backend": "tiktoken", "dim": 128}

    def _load(self) -> EmbeddingIndex:
        if self._cache is not None:
            return self._cache
        if not self.is_available():
            d, fn = get_embedding_fn("tiktoken")
            self._cache = EmbeddingIndex(d, fn)
            return self._cache
        meta = self._read_meta()
        backend = meta.get("embedding_backend", "tiktoken")
        _, embed_fn = get_embedding_fn(backend)
        with open(self._index_path, "rb") as f:
            index = pickle.load(f)
        dim = int(index.d)
        ei = EmbeddingIndex(dim, embed_fn, index=index)
        with open(self._texts_path, encoding="utf-8") as f:
            ei.texts = json.load(f)
        self._cache = ei
        return ei

    def query(self, text: str, k: int = 3) -> List[str]:
        return self._load().query(text, k=k)

    def rebuild(self, chunks: List[str], embedding_backend: str = "tiktoken") -> None:
        self._dir.mkdir(parents=True, exist_ok=True)
        dim, embed_fn = get_embedding_fn(embedding_backend)
        ei = EmbeddingIndex(dim, embed_fn)
        ei.texts = []
        ei.index = faiss.IndexFlatL2(dim)
        for c in chunks:
            ei.add_text(c)
        with open(self._texts_path, "w", encoding="utf-8") as f:
            json.dump(ei.texts, f, ensure_ascii=False, indent=2)
        with open(self._index_path, "wb") as f:
            pickle.dump(ei.index, f)
        meta = {"embedding_backend": embedding_backend.lower(), "dim": dim}
        with open(self._meta_path, "w", encoding="utf-8") as f:
            json.dump(meta, f, indent=2)
        self._cache = ei
