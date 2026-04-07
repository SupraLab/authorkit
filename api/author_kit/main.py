"""AuthorKit FastAPI application."""

from __future__ import annotations

from contextlib import asynccontextmanager
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from author_kit.config import get_author_kit_settings
from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.core.settings_store import SettingsStore
from author_kit.routers import app_settings, chat, compendium, conversation, health, prompts, projects, rag, workshop


@asynccontextmanager
async def lifespan(app: FastAPI):
    cfg = get_author_kit_settings()
    settings_path = cfg.settings_path or Path.home() / ".authorkit" / "settings.json"
    store = SettingsStore(file_path=settings_path)
    app.state.settings_store = store
    app.state.aggregator = LLMAPIAggregator(store)
    yield


def create_app() -> FastAPI:
    cfg = get_author_kit_settings()
    app = FastAPI(
        title="AuthorKit API",
        description="Creative writing assistant HTTP API (fork from Writingway).",
        version="0.1.0",
        lifespan=lifespan,
    )
    app.add_middleware(
        CORSMiddleware,
        allow_origins=cfg.cors_origins,
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )
    app.include_router(health.router)
    app.include_router(app_settings.router)
    app.include_router(chat.router)
    app.include_router(prompts.router)
    app.include_router(projects.router)
    app.include_router(compendium.router)
    app.include_router(workshop.router)
    app.include_router(rag.router)
    app.include_router(conversation.router)
    return app


app = create_app()
