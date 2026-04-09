"""Compendium JSON under `.authorkit/compendium.json`."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

from author_kit.core.paths import compendium_entry_markdown_path, compendium_path


def _ensure_categories_list(data: dict[str, Any]) -> dict[str, Any]:
    categories = data.get("categories", [])
    if isinstance(categories, dict):
        data["categories"] = [
            {"name": cat, "entries": entries} for cat, entries in categories.items()
        ]
    elif not isinstance(categories, list):
        data["categories"] = []
    return data


def load_compendium(workspace_root: Path) -> dict[str, Any]:
    path = compendium_path(workspace_root)
    if not path.is_file():
        return {"categories": []}
    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        return _ensure_categories_list(data)
    except (json.JSONDecodeError, OSError):
        return {"categories": []}


def save_compendium(workspace_root: Path, data: dict[str, Any]) -> None:
    path = compendium_path(workspace_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = _ensure_categories_list(dict(data))
    with open(path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)


def get_entry_text(
    data: dict[str, Any],
    category: str,
    entry: str,
    workspace_root: Path | None = None,
) -> str:
    for cat in data.get("categories", []):
        if cat.get("name") == category:
            for e in cat.get("entries", []):
                if e.get("name") == entry:
                    eid = e.get("id")
                    if eid and workspace_root is not None:
                        path = compendium_entry_markdown_path(workspace_root, category, str(eid))
                        if path.is_file():
                            return path.read_text(encoding="utf-8")
                    return e.get("content", "")
    return f"[No content for {entry} in category {category}]"


def format_compendium_excerpts(
    data: dict[str, Any],
    refs: list[dict[str, str]],
    workspace_root: Path | None = None,
) -> str:
    """refs: items with keys `category` and `name` (entry name)."""
    blocks: list[str] = []
    for ref in refs:
        cat = ref.get("category", "")
        name = ref.get("name", "")
        text = get_entry_text(data, cat, name, workspace_root)
        blocks.append(f"### {cat} / {name}\n{text}")
    return "\n\n".join(blocks)


def parse_references(message: str, data: dict[str, Any]) -> list[str]:
    refs: list[str] = []
    names: list[str] = []
    cats = data.get("categories", [])
    if isinstance(cats, dict):
        names = list(cats.keys())
    elif isinstance(cats, list):
        for cat in cats:
            for entry in cat.get("entries", []):
                names.append(entry.get("name", ""))
    for name in names:
        if name and re.search(r"\b" + re.escape(name) + r"\b", message, re.IGNORECASE):
            refs.append(name)
    return refs
