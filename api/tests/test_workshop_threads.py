"""Workshop thread CRUD and persisted stream."""

import json
from pathlib import Path

from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.core.paths import workshop_thread_json_path


def test_thread_crud_and_json_file(client, tmp_path: Path):
    root = str(tmp_path)
    r = client.post(
        "/v1/workshop/threads",
        json={
            "workspace_root": root,
            "title": "My thread",
            "seed_type": "scene",
            "seed_ref": "abc-uuid",
        },
    )
    assert r.status_code == 200
    body = r.json()
    tid = body["thread_id"]
    assert body["title"] == "My thread"

    jpath = workshop_thread_json_path(tmp_path, tid)
    assert jpath.is_file()
    meta = json.loads(jpath.read_text())
    assert meta["thread_id"] == tid
    assert meta["seed"]["type"] == "scene"

    lst = client.get("/v1/workshop/threads", params={"workspace_root": root})
    assert lst.status_code == 200
    assert len(lst.json()) == 1
    assert lst.json()[0]["thread_id"] == tid

    msg = client.get(
        f"/v1/workshop/threads/{tid}/messages",
        params={"workspace_root": root},
    )
    assert msg.status_code == 200
    assert msg.json()["messages"] == []

    d = client.delete(
        f"/v1/workshop/threads/{tid}",
        params={"workspace_root": root},
    )
    assert d.status_code == 200
    assert not jpath.is_file()


def test_stream_persists_messages(client, tmp_path: Path, monkeypatch):
    root = str(tmp_path)

    def _stream(self, *a, **k):
        yield "hello"
        yield " world"

    monkeypatch.setattr(LLMAPIAggregator, "stream_prompt_to_llm", _stream)
    monkeypatch.setattr(
        LLMAPIAggregator,
        "send_prompt_to_llm",
        lambda self, *a, **k: "summary",
    )

    cr = client.post(
        "/v1/workshop/threads",
        json={"workspace_root": root, "title": "T"},
    )
    tid = cr.json()["thread_id"]

    r = client.post(
        "/v1/workshop/chat/stream",
        json={
            "user_message": "Hi",
            "workspace_root": root,
            "thread_id": tid,
        },
    )
    assert r.status_code == 200
    assert "hello" in r.text and "world" in r.text

    msg = client.get(
        f"/v1/workshop/threads/{tid}/messages",
        params={"workspace_root": root},
    )
    assert msg.status_code == 200
    messages = msg.json()["messages"]
    assert len(messages) == 2
    assert messages[0]["role"] == "user"
    assert messages[0]["content"] == "Hi"
    assert messages[1]["role"] == "assistant"
    assert messages[1]["content"] == "hello world"


def test_rename_thread(client, tmp_path: Path):
    root = str(tmp_path)
    cr = client.post(
        "/v1/workshop/threads",
        json={"workspace_root": root, "title": "Old"},
    )
    tid = cr.json()["thread_id"]
    r = client.patch(
        f"/v1/workshop/threads/{tid}",
        params={"workspace_root": root},
        json={"title": "Ethan — backstory"},
    )
    assert r.status_code == 200
    assert r.json()["title"] == "Ethan — backstory"
    lst = client.get("/v1/workshop/threads", params={"workspace_root": root})
    assert lst.json()[0]["title"] == "Ethan — backstory"


def test_thread_not_found_messages(client, tmp_path: Path):
    root = str(tmp_path)
    r = client.get(
        "/v1/workshop/threads/00000000-0000-0000-0000-000000000001/messages",
        params={"workspace_root": root},
    )
    assert r.status_code == 404


def test_stream_includes_scene_uuid_context(client, tmp_path: Path, monkeypatch):
    import uuid

    from author_kit.core.scene_store import write_scene

    root = str(tmp_path)
    uid = str(uuid.uuid4())
    write_scene(tmp_path, uid, "# Beat\n\nDialogue here.")

    captured: dict = {}

    def _stream(self, final_prompt, *a, **k):
        captured["prompt"] = final_prompt
        yield "ok"

    monkeypatch.setattr(LLMAPIAggregator, "stream_prompt_to_llm", _stream)

    cr = client.post(
        "/v1/workshop/threads",
        json={"workspace_root": root, "title": "S"},
    )
    tid = cr.json()["thread_id"]

    r = client.post(
        "/v1/workshop/chat/stream",
        json={
            "user_message": "Tighten",
            "workspace_root": root,
            "thread_id": tid,
            "scene_uuids": [uid],
        },
    )
    assert r.status_code == 200
    assert "Dialogue here" in captured.get("prompt", "")
    assert uid in captured.get("prompt", "")


def test_stream_persists_user_message_context(client, tmp_path: Path, monkeypatch):
    """scene_uuids + compendium_excerpts are stored on the user row and returned by GET messages."""
    import uuid

    from author_kit.core.scene_store import write_scene

    uid = str(uuid.uuid4())
    write_scene(tmp_path, uid, "# Scene\n\nBody.")

    def _stream(self, *a, **k):
        yield "done"

    monkeypatch.setattr(LLMAPIAggregator, "stream_prompt_to_llm", _stream)
    monkeypatch.setattr(
        LLMAPIAggregator,
        "send_prompt_to_llm",
        lambda self, *a, **k: "summary",
    )

    root = str(tmp_path)
    cr = client.post(
        "/v1/workshop/threads",
        json={"workspace_root": root, "title": "Ctx"},
    )
    tid = cr.json()["thread_id"]

    r = client.post(
        "/v1/workshop/chat/stream",
        json={
            "user_message": "Hello",
            "workspace_root": root,
            "thread_id": tid,
            "scene_uuids": [uid],
            "compendium_excerpts": [{"category": "Characters", "name": "Ethan"}],
        },
    )
    assert r.status_code == 200
    # Consume the full SSE body so the generator runs to completion and persists messages.
    assert "done" in r.text and "[DONE]" in r.text

    msg = client.get(
        f"/v1/workshop/threads/{tid}/messages",
        params={"workspace_root": root},
    )
    assert msg.status_code == 200
    messages = msg.json()["messages"]
    assert len(messages) == 2
    u = messages[0]
    assert u["role"] == "user"
    assert u["content"] == "Hello"
    assert u.get("context") is not None
    assert u["context"]["scene_uuids"] == [uid]
    assert u["context"]["compendium_excerpts"] == [{"category": "Characters", "name": "Ethan"}]
    assert messages[1]["role"] == "assistant"
    assert messages[1].get("context") in (None, {})
