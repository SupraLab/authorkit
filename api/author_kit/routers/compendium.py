"""Compendium CRUD."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel

from author_kit.core import compendium_store
from author_kit.deps import workspace_from_query

router = APIRouter(tags=["compendium"])


def _ws(path: str) -> Path:
    p = Path(path).expanduser().resolve()
    if not p.is_dir():
        raise HTTPException(status_code=400, detail=f"Not a directory: {p}")
    return p


class CompendiumPutBody(BaseModel):
    workspace_root: str
    data: Dict[str, Any]


@router.get("/v1/compendium")
def get_compendium(ws: Path = Depends(workspace_from_query)) -> Dict[str, Any]:
    return compendium_store.load_compendium(ws)


@router.put("/v1/compendium")
def put_compendium(body: CompendiumPutBody) -> dict:
    ws = _ws(body.workspace_root)
    compendium_store.save_compendium(ws, body.data)
    return {"ok": True}
