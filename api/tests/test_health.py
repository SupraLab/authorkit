def test_health(client):
    r = client.get("/health")
    assert r.status_code == 200
    assert r.json()["status"] == "ok"


def test_ready(client):
    r = client.get("/ready")
    assert r.status_code == 200
    data = r.json()
    assert "active_llm_profile" in data
    assert data["status"] in ("ready", "degraded")
