"""Novel structure JSON and optional project settings under `.authorkit/`."""

from __future__ import annotations

import json
import uuid
from pathlib import Path
from typing import Any, Dict, List, Optional

from author_kit.core.paths import project_settings_path, structure_path

DEFAULT_STRUCTURE: Dict[str, Any] = {
    "acts": [
        {
            "name": "Act 1",
            "summary": "This is the summary for Act 1.",
            "has_summary": False,
            "uuid": "",
            "chapters": [
                {
                    "name": "Chapter 1",
                    "summary": "This is the summary for Chapter 1.",
                    "has_summary": False,
                    "uuid": "",
                    "scenes": [{"name": "Scene 1", "uuid": ""}],
                }
            ],
        }
    ]
}


def _ensure_uuids(node: Dict[str, Any]) -> None:
    if "uuid" not in node or not node["uuid"]:
        node["uuid"] = str(uuid.uuid4())
    if "summary" in node and "has_summary" not in node:
        node["has_summary"] = not str(node["summary"]).startswith("This is the summary")
    for child in node.get("chapters", []) + node.get("scenes", []):
        if isinstance(child, dict):
            _ensure_uuids(child)


def load_structure(workspace_root: Path) -> Dict[str, Any]:
    path = structure_path(workspace_root)
    if not path.is_file():
        data = json.loads(json.dumps(DEFAULT_STRUCTURE))
        for act in data.get("acts", []):
            _ensure_uuids(act)
        return data
    try:
        with open(path, encoding="utf-8") as f:
            structure = json.load(f)
        for act in structure.get("acts", []):
            _ensure_uuids(act)
        return structure
    except (json.JSONDecodeError, OSError):
        return json.loads(json.dumps(DEFAULT_STRUCTURE))


def save_structure(workspace_root: Path, structure: Dict[str, Any]) -> None:
    path = structure_path(workspace_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(structure, f, indent=2, ensure_ascii=False)


def load_project_settings(workspace_root: Path) -> Dict[str, Any]:
    path = project_settings_path(workspace_root)
    defaults = {
        "global_pov": "Third Person Limited",
        "global_pov_character": "Character",
        "global_tense": "Present Tense",
    }
    if not path.is_file():
        return defaults
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        defaults.update(data)
        return defaults
    except (json.JSONDecodeError, OSError):
        return defaults


def save_project_settings(workspace_root: Path, settings: Dict[str, Any]) -> None:
    path = project_settings_path(workspace_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(settings, f, indent=2, ensure_ascii=False)


def find_scene_by_hierarchy(
    structure: Dict[str, Any], hierarchy: List[str]
) -> Optional[Dict[str, Any]]:
    """hierarchy: [act, chapter, scene]."""
    if len(hierarchy) < 3:
        return None
    act_name, ch_name, sc_name = hierarchy[0], hierarchy[1], hierarchy[2]
    for act in structure.get("acts", []):
        if act.get("name") != act_name:
            continue
        for ch in act.get("chapters", []):
            if ch.get("name") != ch_name:
                continue
            for sc in ch.get("scenes", []):
                if sc.get("name") == sc_name:
                    return sc
    return None


def walk_scenes(structure: Dict[str, Any]) -> List[tuple[List[str], Dict[str, Any]]]:
    """Yield (hierarchy_names, scene_node)."""
    out: List[tuple[List[str], Dict[str, Any]]] = []
    for act in structure.get("acts", []):
        an = act.get("name", "")
        for ch in act.get("chapters", []):
            cn = ch.get("name", "")
            for sc in ch.get("scenes", []):
                out.append(([an, cn, sc.get("name", "")], sc))
    return out
