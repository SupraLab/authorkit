from author_kit.core.llm_aggregator import LLMAPIAggregator


def test_summarize(client, monkeypatch):
    monkeypatch.setattr(
        LLMAPIAggregator,
        "send_prompt_to_llm",
        lambda self, *a, **k: "short summary",
    )
    r = client.post(
        "/v1/conversation/summarize",
        json={
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": "Hello"},
            ],
            "max_tokens": 100,
        },
    )
    assert r.status_code == 200
    assert r.json()["summary"] == "short summary"
