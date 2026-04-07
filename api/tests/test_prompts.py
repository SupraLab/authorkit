def test_prompt_preview(client):
    r = client.post(
        "/v1/prompts/preview",
        json={
            "prompt_config": {"text": "You are a writer."},
            "user_input": "Go on",
        },
    )
    assert r.status_code == 200
    assert "You are a writer" in r.json()["preview"]
