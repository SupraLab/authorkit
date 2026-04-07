from author_kit.core.llm_aggregator import LLMAPIAggregator


def test_workshop_chat(client, monkeypatch):
    monkeypatch.setattr(
        LLMAPIAggregator,
        "send_prompt_to_llm",
        lambda self, *a, **k: "workshop-ok",
    )
    r = client.post(
        "/v1/workshop/chat",
        json={"user_message": "Hello"},
    )
    assert r.status_code == 200
    assert r.json()["content"] == "workshop-ok"
