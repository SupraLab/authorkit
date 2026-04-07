"""Structure and project settings under `.authorkit/`."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel

from author_kit.core import project_store, scene_store
from author_kit.core.paths import prompts_path
from author_kit.deps import workspace_from_query

router = APIRouter(tags=["projects"])


class StructurePutBody(BaseModel):
    workspace_root: str
    structure: Dict[str, Any]


class SettingsPutBody(BaseModel):
    workspace_root: str
    settings: Dict[str, Any]


class PromptsPutBody(BaseModel):
    workspace_root: str
    prompts: Any


class ScenePutBody(BaseModel):
    workspace_root: str
    content: str = ""


def _ws(path: str) -> Path:
    p = Path(path).expanduser().resolve()
    if not p.is_dir():
        raise HTTPException(status_code=400, detail=f"Not a directory: {p}")
    return p


@router.get("/v1/projects/structure")
def get_structure(ws: Path = Depends(workspace_from_query)) -> Dict[str, Any]:
    return project_store.load_structure(ws)


@router.put("/v1/projects/structure")
def put_structure(body: StructurePutBody) -> dict:
    ws = _ws(body.workspace_root)
    if "acts" not in body.structure:
        raise HTTPException(status_code=400, detail="structure must contain 'acts'")
    project_store.save_structure(ws, body.structure)
    return {"ok": True}


@router.get("/v1/projects/settings")
def get_project_settings(ws: Path = Depends(workspace_from_query)) -> Dict[str, Any]:
    return project_store.load_project_settings(ws)


@router.put("/v1/projects/settings")
def put_project_settings(body: SettingsPutBody) -> dict:
    ws = _ws(body.workspace_root)
    project_store.save_project_settings(ws, body.settings)
    return {"ok": True}


@router.get("/v1/projects/prompts-file")
def get_prompts_json(ws: Path = Depends(workspace_from_query)):
    p = prompts_path(ws)
    if not p.is_file():
        return []
    with open(p, encoding="utf-8") as f:
        return json.load(f)


@router.put("/v1/projects/prompts-file")
def put_prompts_json(body: PromptsPutBody) -> dict:
    ws = _ws(body.workspace_root)
    p = prompts_path(ws)
    p.parent.mkdir(parents=True, exist_ok=True)
    with open(p, "w", encoding="utf-8") as f:
        json.dump(body.prompts, f, indent=2, ensure_ascii=False)
    return {"ok": True}


@router.get("/v1/projects/scenes/{scene_uuid}")
def get_scene(scene_uuid: str, ws: Path = Depends(workspace_from_query)) -> dict:
    try:
        content = scene_store.read_scene(ws, scene_uuid)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    return {"scene_uuid": scene_uuid, "content": content}


@router.put("/v1/projects/scenes/{scene_uuid}")
def put_scene(scene_uuid: str, body: ScenePutBody) -> dict:
    ws = _ws(body.workspace_root)
    try:
        scene_store.write_scene(ws, scene_uuid, body.content)
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e)) from e
    return {"ok": True}
