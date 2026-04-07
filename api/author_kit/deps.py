"""FastAPI dependencies."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated, Optional

from fastapi import Header, HTTPException, Query, Request

from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.core.settings_store import SettingsStore


def get_settings_store(request: Request) -> SettingsStore:
    return request.app.state.settings_store


def get_aggregator(request: Request) -> LLMAPIAggregator:
    return request.app.state.aggregator


def workspace_from_query(
    workspace_root: Annotated[str, Query(..., description="Absolute path to novel workspace")],
) -> Path:
    p = Path(workspace_root).expanduser().resolve()
    if not p.is_dir():
        raise HTTPException(status_code=400, detail=f"Not a directory: {p}")
    return p


def workspace_optional(
    x_workspace_root: Annotated[Optional[str], Header()] = None,
    workspace_root: Annotated[Optional[str], Query()] = None,
) -> Optional[Path]:
    raw = workspace_root or x_workspace_root
    if not raw:
        return None
    p = Path(raw).expanduser().resolve()
    if not p.is_dir():
        raise HTTPException(status_code=400, detail=f"Not a directory: {p}")
    return p
