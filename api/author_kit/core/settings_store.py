"""LLM and app settings — file-backed, no global singleton required."""

from __future__ import annotations

import copy
import json
import logging
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)

DEFAULT_SETTINGS: dict[str, Any] = {
    "version": "1",
    "general": {
        "language": "en",
        "enable_debug_logging": False,
    },
    "llm_configs": {
        "OpenAI": {
            "provider": "OpenAI",
            "endpoint": "https://api.openai.com/v1",
            "model": "gpt-3.5-turbo",
            "api_key": "",
            "timeout": 30,
        },
        "Ollama": {
            "provider": "Ollama",
            "endpoint": "http://localhost:11434/v1",
            "api_key": "",
            "model": "llama3.2",
            "timeout": 240,
        },
        "Anthropic": {
            "provider": "Anthropic",
            "endpoint": "https://api.anthropic.com/v1/",
            "model": "claude-3-haiku-20240307",
            "api_key": "",
            "timeout": 60,
        },
        "Gemini": {
            "provider": "Gemini",
            "endpoint": "https://generativelanguage.googleapis.com/v1beta/",
            "model": "gemini-2.0-flash",
            "api_key": "",
            "timeout": 60,
        },
        "OpenRouter": {
            "provider": "OpenRouter",
            "endpoint": "https://openrouter.ai/api/v1/",
            "model": "",
            "api_key": "",
            "timeout": 60,
        },
        "TogetherAI": {
            "provider": "TogetherAI",
            "endpoint": "https://api.together.xyz/v1",
            "model": "",
            "api_key": "",
            "timeout": 60,
        },
        "LMStudio": {
            "provider": "LMStudio",
            "endpoint": "http://localhost:1234/v1",
            "model": "local-model",
            "api_key": "",
            "timeout": 30,
        },
        "Custom": {
            "provider": "Custom",
            "endpoint": "http://localhost:11434/v1/",
            "model": "custom-model",
            "api_key": "",
            "timeout": 60,
        },
    },
    "active_llm_config": "Ollama",
}


class SettingsStore:
    """Holds AuthorKit settings dict; optionally persists to disk."""

    def __init__(self, file_path: Path | None = None, data: dict[str, Any] | None = None):
        self.file_path = Path(file_path) if file_path else None
        self._settings: dict[str, Any] = copy.deepcopy(DEFAULT_SETTINGS)
        if data is not None:
            self._merge(data)
        elif self.file_path and self.file_path.exists():
            self._load()

    def _merge(self, data: dict[str, Any]) -> None:
        if "llm_configs" in data:
            self._settings["llm_configs"].update(data.get("llm_configs", {}))
        for key in ("version", "active_llm_config", "general"):
            if key in data:
                if key == "general" and isinstance(data["general"], dict):
                    self._settings.setdefault("general", {}).update(data["general"])
                elif key != "general":
                    self._settings[key] = data[key]

    def _load(self) -> None:
        if not self.file_path:
            return
        try:
            with open(self.file_path, encoding="utf-8") as f:
                data = json.load(f)
            self._settings = copy.deepcopy(DEFAULT_SETTINGS)
            self._merge(data)
            logger.info("Loaded settings from %s", self.file_path)
        except (json.JSONDecodeError, OSError) as e:
            logger.error("Failed to load settings: %s", e)

    def save(self) -> bool:
        if not self.file_path:
            return False
        try:
            self.file_path.parent.mkdir(parents=True, exist_ok=True)
            with open(self.file_path, "w", encoding="utf-8") as f:
                json.dump(self._settings, f, indent=2, ensure_ascii=False)
            return True
        except OSError as e:
            logger.error("Failed to save settings: %s", e)
            return False

    def get_llm_configs(self) -> dict[str, dict[str, Any]]:
        return copy.deepcopy(self._settings.get("llm_configs", {}))

    def get_active_llm_name(self) -> str:
        return self._settings.get("active_llm_config", "") or ""

    def get_active_llm_config(self) -> dict[str, Any] | None:
        name = self.get_active_llm_name()
        if name and name in self._settings.get("llm_configs", {}):
            return copy.deepcopy(self._settings["llm_configs"][name])
        return None

    def set_active_llm(self, name: str) -> None:
        if name in self._settings.get("llm_configs", {}):
            self._settings["active_llm_config"] = name

    def update_llm_config(self, name: str, config: dict[str, Any]) -> None:
        self._settings.setdefault("llm_configs", {})[name] = config

    def merge_from_dict(self, data: dict[str, Any]) -> None:
        """Merge settings from a client (e.g. VS Code extension); same rules as loading from disk."""
        self._merge(data)

    @property
    def raw(self) -> dict[str, Any]:
        return copy.deepcopy(self._settings)
