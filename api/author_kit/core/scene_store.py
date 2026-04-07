"""Scene prose as Markdown under `.authorkit/scenes/{uuid}.md`."""

from __future__ import annotations

import uuid
from pathlib import Path

from author_kit.core.paths import scenes_dir


def _validate_uuid(scene_uuid: str) -> str:
    try:
        return str(uuid.UUID(scene_uuid))
    except ValueError as e:
        raise ValueError(f"invalid scene uuid: {scene_uuid}") from e


def scene_file_path(workspace_root: Path, scene_uuid: str) -> Path:
    uid = _validate_uuid(scene_uuid)
    return scenes_dir(workspace_root) / f"{uid}.md"


def read_scene(workspace_root: Path, scene_uuid: str) -> str:
    path = scene_file_path(workspace_root, scene_uuid)
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def write_scene(workspace_root: Path, scene_uuid: str, content: str) -> None:
    path = scene_file_path(workspace_root, scene_uuid)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
