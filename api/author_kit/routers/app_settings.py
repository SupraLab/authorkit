"""Global app / LLM settings (`~/.authorkit/settings.json`), shared with the VS Code extension."""

from __future__ import annotations

from typing import Any, Dict

from fastapi import APIRouter, HTTPException, Request

router = APIRouter(tags=["app-settings"])


@router.get("/v1/app/settings")
def get_app_settings(request: Request) -> Dict[str, Any]:
    """Return full settings dict (includes API keys; intended for localhost / trusted clients)."""
    store = request.app.state.settings_store
    return store.raw


@router.put("/v1/app/settings")
def put_app_settings(request: Request, body: Dict[str, Any]) -> dict:
    """Merge JSON into the in-memory store and persist to disk."""
    store = request.app.state.settings_store
    store.merge_from_dict(body)
    if not store.save():
        raise HTTPException(status_code=500, detail="Failed to save settings file")
    return {"ok": True}
