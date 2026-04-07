"""Chat routes with mocked LLM."""

from author_kit.core.llm_aggregator import LLMAPIAggregator


def test_chat_completion(client, mock_llm):
    r = client.post(
        "/v1/chat",
        json={
            "messages": [{"role": "user", "content": "Hi"}],
        },
    )
    assert r.status_code == 200
    assert r.json()["content"] == "mocked-response"


def test_chat_stream(client, monkeypatch):
    monkeypatch.setattr(
        LLMAPIAggregator,
        "stream_prompt_to_llm",
        lambda self, *a, **k: iter(["a", "b"]),
    )
    r = client.post(
        "/v1/chat/stream",
        json={"messages": [{"role": "user", "content": "Hi"}]},
    )
    assert r.status_code == 200
    body = r.text
    assert "a" in body and "[DONE]" in body
