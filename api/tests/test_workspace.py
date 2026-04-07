"""Workspace JSON under .authorkit/."""

import json
from pathlib import Path


def test_structure_roundtrip(client, tmp_path: Path):
    root = str(tmp_path)
    r = client.get("/v1/projects/structure", params={"workspace_root": root})
    assert r.status_code == 200
    data = r.json()
    assert "acts" in data

    data["acts"][0]["name"] = "Act X"
    r2 = client.put(
        "/v1/projects/structure",
        json={"workspace_root": root, "structure": data},
    )
    assert r2.status_code == 200
    p = tmp_path / ".authorkit" / "structure.json"
    assert p.is_file()
    saved = json.loads(p.read_text())
    assert saved["acts"][0]["name"] == "Act X"


def test_compendium_roundtrip(client, tmp_path: Path):
    root = str(tmp_path)
    payload = {
        "categories": [{"name": "People", "entries": [{"name": "Hero", "content": "A"}]}]
    }
    r = client.put(
        "/v1/compendium",
        json={"workspace_root": root, "data": payload},
    )
    assert r.status_code == 200
    r2 = client.get("/v1/compendium", params={"workspace_root": root})
    assert r2.json()["categories"][0]["name"] == "People"


def test_scene_roundtrip(client, tmp_path: Path):
    import uuid

    root = str(tmp_path)
    uid = str(uuid.uuid4())
    r = client.put(
        f"/v1/projects/scenes/{uid}",
        json={"workspace_root": root, "content": "# Scene\n\nHello"},
    )
    assert r.status_code == 200
    g = client.get(
        f"/v1/projects/scenes/{uid}",
        params={"workspace_root": root},
    )
    assert g.status_code == 200
    assert "Hello" in g.json()["content"]


def test_rag_index_query(client, tmp_path: Path):
    root = str(tmp_path)
    r = client.post(
        "/v1/rag/index",
        json={
            "workspace_root": root,
            "chunks": ["alpha bravo", "charlie delta"],
        },
    )
    assert r.status_code == 200
    q = client.post(
        "/v1/rag/query",
        json={"workspace_root": root, "query": "bravo", "k": 2},
    )
    assert q.status_code == 200
    assert len(q.json()["chunks"]) >= 1
