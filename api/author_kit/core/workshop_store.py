"""SQLite persistence for Workshop threads and messages (API process only)."""

from __future__ import annotations

import contextlib
import json
import sqlite3
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from author_kit.core.paths import workshop_db_path, workshop_thread_json_path, workshop_threads_dir


def _utc_now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def init_schema(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS workshop_threads (
            id TEXT PRIMARY KEY,
            workspace_root TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            rolling_summary TEXT,
            seed_type TEXT,
            seed_ref TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS workshop_messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            context_json TEXT,
            FOREIGN KEY (thread_id) REFERENCES workshop_threads(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_workshop_messages_thread
            ON workshop_messages(thread_id, id);
        """
    )
    conn.commit()
    _migrate_workshop_messages_context(conn)


def _migrate_workshop_messages_context(conn: sqlite3.Connection) -> None:
    rows = conn.execute("PRAGMA table_info(workshop_messages)").fetchall()
    col_names = {r[1] for r in rows}
    if "context_json" not in col_names:
        conn.execute("ALTER TABLE workshop_messages ADD COLUMN context_json TEXT")
        conn.commit()


def get_connection(workspace_root: Path) -> sqlite3.Connection:
    db = workshop_db_path(workspace_root)
    conn = _connect(db)
    init_schema(conn)
    return conn


@dataclass
class ThreadRow:
    id: str
    workspace_root: str
    title: str
    rolling_summary: str | None
    seed_type: str | None
    seed_ref: str | None
    created_at: str
    updated_at: str


def create_thread(
    workspace_root: Path,
    *,
    title: str = "",
    seed_type: str | None = None,
    seed_ref: str | None = None,
) -> str:
    tid = str(uuid.uuid4())
    ws = str(workspace_root.resolve())
    now = _utc_now_iso()
    conn = get_connection(workspace_root)
    try:
        conn.execute(
            """
            INSERT INTO workshop_threads
            (id, workspace_root, title, rolling_summary, seed_type, seed_ref, created_at, updated_at)
            VALUES (?, ?, ?, NULL, ?, ?, ?, ?)
            """,
            (tid, ws, title or "Workshop", seed_type, seed_ref, now, now),
        )
        conn.commit()
    finally:
        conn.close()
    write_thread_metadata_file(
        workspace_root, tid, title=title or "Workshop", seed_type=seed_type, seed_ref=seed_ref
    )
    return tid


def delete_thread(workspace_root: Path, thread_id: str) -> bool:
    conn = get_connection(workspace_root)
    try:
        cur = conn.execute(
            "DELETE FROM workshop_threads WHERE id = ? AND workspace_root = ?",
            (thread_id, str(workspace_root.resolve())),
        )
        conn.commit()
        deleted = cur.rowcount > 0
    finally:
        conn.close()
    if deleted:
        p = workshop_thread_json_path(workspace_root, thread_id)
        if p.is_file():
            p.unlink()
    return deleted


def list_threads(workspace_root: Path) -> list[ThreadRow]:
    ws = str(workspace_root.resolve())
    conn = get_connection(workspace_root)
    try:
        rows = conn.execute(
            """
            SELECT id, workspace_root, title, rolling_summary, seed_type, seed_ref, created_at, updated_at
            FROM workshop_threads
            WHERE workspace_root = ?
            ORDER BY updated_at DESC
            """,
            (ws,),
        ).fetchall()
    finally:
        conn.close()
    return [
        ThreadRow(
            id=r["id"],
            workspace_root=r["workspace_root"],
            title=r["title"] or "",
            rolling_summary=r["rolling_summary"],
            seed_type=r["seed_type"],
            seed_ref=r["seed_ref"],
            created_at=r["created_at"],
            updated_at=r["updated_at"],
        )
        for r in rows
    ]


def get_thread(workspace_root: Path, thread_id: str) -> ThreadRow | None:
    ws = str(workspace_root.resolve())
    conn = get_connection(workspace_root)
    try:
        r = conn.execute(
            """
            SELECT id, workspace_root, title, rolling_summary, seed_type, seed_ref, created_at, updated_at
            FROM workshop_threads
            WHERE id = ? AND workspace_root = ?
            """,
            (thread_id, ws),
        ).fetchone()
    finally:
        conn.close()
    if r is None:
        return None
    return ThreadRow(
        id=r["id"],
        workspace_root=r["workspace_root"],
        title=r["title"] or "",
        rolling_summary=r["rolling_summary"],
        seed_type=r["seed_type"],
        seed_ref=r["seed_ref"],
        created_at=r["created_at"],
        updated_at=r["updated_at"],
    )


def list_messages(workspace_root: Path, thread_id: str) -> list[dict]:
    ws = str(workspace_root.resolve())
    conn = get_connection(workspace_root)
    try:
        ok = conn.execute(
            "SELECT 1 FROM workshop_threads WHERE id = ? AND workspace_root = ?",
            (thread_id, ws),
        ).fetchone()
        if not ok:
            return []
        rows = conn.execute(
            """
            SELECT role, content, context_json FROM workshop_messages
            WHERE thread_id = ?
            ORDER BY id ASC
            """,
            (thread_id,),
        ).fetchall()
    finally:
        conn.close()
    out: list[dict] = []
    for r in rows:
        row: dict = {"role": r["role"], "content": r["content"]}
        cj = r["context_json"]
        if cj:
            with contextlib.suppress(json.JSONDecodeError):
                row["context"] = json.loads(cj)
        out.append(row)
    return out


def append_message(
    workspace_root: Path,
    thread_id: str,
    role: str,
    content: str,
    *,
    context_json: str | None = None,
) -> None:
    ws = str(workspace_root.resolve())
    now = _utc_now_iso()
    conn = get_connection(workspace_root)
    try:
        conn.execute(
            """
            INSERT INTO workshop_messages (thread_id, role, content, created_at, context_json)
            VALUES (?, ?, ?, ?, ?)
            """,
            (thread_id, role, content, now, context_json),
        )
        conn.execute(
            """
            UPDATE workshop_threads SET updated_at = ? WHERE id = ? AND workspace_root = ?
            """,
            (now, thread_id, ws),
        )
        conn.commit()
    finally:
        conn.close()


def update_rolling_summary(workspace_root: Path, thread_id: str, summary: str | None) -> None:
    ws = str(workspace_root.resolve())
    now = _utc_now_iso()
    conn = get_connection(workspace_root)
    try:
        conn.execute(
            "UPDATE workshop_threads SET rolling_summary = ?, updated_at = ? WHERE id = ? AND workspace_root = ?",
            (summary, now, thread_id, ws),
        )
        conn.commit()
    finally:
        conn.close()


def update_thread_title(workspace_root: Path, thread_id: str, title: str) -> None:
    ws = str(workspace_root.resolve())
    now = _utc_now_iso()
    conn = get_connection(workspace_root)
    try:
        conn.execute(
            "UPDATE workshop_threads SET title = ?, updated_at = ? WHERE id = ? AND workspace_root = ?",
            (title, now, thread_id, ws),
        )
        conn.commit()
    finally:
        conn.close()


def rename_thread(workspace_root: Path, thread_id: str, title: str) -> bool:
    """Set thread title and sync `.authorkit/workshop/threads/<id>.json`. Returns False if thread missing."""
    title = title.strip()
    if not title:
        return False
    if get_thread(workspace_root, thread_id) is None:
        return False
    update_thread_title(workspace_root, thread_id, title)
    tr = get_thread(workspace_root, thread_id)
    if tr is None:
        return False
    write_thread_metadata_file(
        workspace_root,
        thread_id,
        title=tr.title,
        seed_type=tr.seed_type,
        seed_ref=tr.seed_ref,
    )
    return True


def write_thread_metadata_file(
    workspace_root: Path,
    thread_id: str,
    *,
    title: str,
    seed_type: str | None = None,
    seed_ref: str | None = None,
    extra: dict | None = None,
) -> None:
    workshop_threads_dir(workspace_root).mkdir(parents=True, exist_ok=True)
    path = workshop_thread_json_path(workspace_root, thread_id)
    tr = get_thread(workspace_root, thread_id)
    created_at = tr.created_at if tr else _utc_now_iso()
    updated_at = tr.updated_at if tr else _utc_now_iso()
    payload: dict[str, Any] = {
        "thread_id": thread_id,
        "title": title,
        "created_at": created_at,
        "updated_at": updated_at,
        "seed": {"type": seed_type or "none", "ref": seed_ref or ""},
    }
    if extra:
        payload.update(extra)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def touch_thread_json_after_message(workspace_root: Path, thread_id: str) -> None:
    tr = get_thread(workspace_root, thread_id)
    if not tr:
        return
    write_thread_metadata_file(
        workspace_root,
        thread_id,
        title=tr.title,
        seed_type=tr.seed_type,
        seed_ref=tr.seed_ref,
    )
