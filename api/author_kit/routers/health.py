"""Health and readiness."""

from fastapi import APIRouter, Request

router = APIRouter(tags=["health"])


@router.get("/health")
def health() -> dict:
    return {"status": "ok"}


@router.get("/ready")
def ready(request: Request) -> dict:
    store = request.app.state.settings_store
    path = store.file_path
    settings_ok = path is not None and path.is_file()
    return {
        "status": "ready" if settings_ok else "degraded",
        "settings_path": str(path) if path else None,
        "settings_file_present": settings_ok,
        "active_llm_profile": store.get_active_llm_name(),
    }
