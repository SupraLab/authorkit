"""RAG index and query (FAISS + tiktoken embeddings)."""

from __future__ import annotations

from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException

from author_kit.core.rag_index import PersistentRagIndex
from author_kit.deps import workspace_from_query
from author_kit.schemas import RagIndexRequest, RagQueryRequest, RagQueryResponse

router = APIRouter(tags=["rag"])


@router.post("/v1/rag/index")
def rag_index(body: RagIndexRequest) -> dict:
    ws = Path(body.workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    idx = PersistentRagIndex(ws)
    try:
        idx.rebuild(body.chunks, embedding_backend=body.embedding_backend)
    except RuntimeError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    return {"ok": True, "count": len(body.chunks), "embedding_backend": body.embedding_backend}


@router.post("/v1/rag/query", response_model=RagQueryResponse)
def rag_query(body: RagQueryRequest) -> RagQueryResponse:
    ws = Path(body.workspace_root).expanduser().resolve()
    if not ws.is_dir():
        raise HTTPException(status_code=400, detail="invalid workspace_root")
    idx = PersistentRagIndex(ws)
    if not idx.is_available():
        raise HTTPException(status_code=404, detail="no RAG index for workspace; POST /v1/rag/index first")
    chunks = idx.query(body.query, k=body.k)
    return RagQueryResponse(chunks=chunks)


@router.get("/v1/rag/status")
def rag_status(ws: Path = Depends(workspace_from_query)) -> dict:
    idx = PersistentRagIndex(ws)
    return {"available": idx.is_available()}
