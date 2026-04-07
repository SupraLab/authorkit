"""Global app settings API."""

from fastapi.testclient import TestClient

from author_kit.main import create_app


def test_app_settings_get():
    app = create_app()
    with TestClient(app) as client:
        r = client.get("/v1/app/settings")
        assert r.status_code == 200
        data = r.json()
        assert "llm_configs" in data
        assert "active_llm_config" in data
        assert isinstance(data["llm_configs"], dict)


def test_app_settings_put_merge():
    app = create_app()
    with TestClient(app) as client:
        cur = client.get("/v1/app/settings").json()
        cur["active_llm_config"] = cur.get("active_llm_config") or "Ollama"
        r = client.put("/v1/app/settings", json=cur)
        assert r.status_code == 200
        again = client.get("/v1/app/settings").json()
        assert again["active_llm_config"] == cur["active_llm_config"]
