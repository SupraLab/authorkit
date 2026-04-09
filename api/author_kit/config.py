"""Application settings (AUTHORKIT_* environment variables)."""

from functools import lru_cache
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class AuthorKitSettings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="AUTHORKIT_",
        env_file=".env",
        extra="ignore",
    )

    app_name: str = "AuthorKit API"
    host: str = "127.0.0.1"
    port: int = 8765
    cors_origins: list[str] = Field(default_factory=lambda: ["*"])
    settings_path: Path | None = Field(
        default=None,
        description="Path to JSON file with llm_configs (defaults to ~/.authorkit/settings.json)",
    )


@lru_cache
def get_author_kit_settings() -> AuthorKitSettings:
    return AuthorKitSettings()
