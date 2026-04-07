"""Workspace-relative paths for AuthorKit metadata."""

import re
from pathlib import Path

AUTHORKIT_DIR = ".authorkit"


def category_slug(category: str) -> str:
    """Folder name under `.authorkit/` for compendium entries with a file `id`."""
    s = re.sub(r"[^a-zA-Z0-9]+", "-", category).strip("-").lower()
    return s or "entry"


def compendium_entry_markdown_path(workspace_root: Path, category: str, entry_id: str) -> Path:
    """One Markdown file per compendium entry when `id` is set (mirrors scene files under `scenes/`)."""
    return authorkit_root(workspace_root) / category_slug(category) / f"{entry_id}.md"


def authorkit_root(workspace_root: Path) -> Path:
    return workspace_root / AUTHORKIT_DIR


def structure_path(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "structure.json"


def compendium_path(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "compendium.json"


def prompts_path(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "prompts.json"


def project_settings_path(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "project_settings.json"


def rag_dir(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "rag"


def scenes_dir(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "scenes"


def workshop_dir(workspace_root: Path) -> Path:
    return authorkit_root(workspace_root) / "workshop"


def workshop_db_path(workspace_root: Path) -> Path:
    return workshop_dir(workspace_root) / "workshop.db"


def workshop_threads_dir(workspace_root: Path) -> Path:
    return workshop_dir(workspace_root) / "threads"


def workshop_thread_json_path(workspace_root: Path, thread_id: str) -> Path:
    return workshop_threads_dir(workspace_root) / f"{thread_id}.json"
