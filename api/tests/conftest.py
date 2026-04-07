"""Pytest fixtures."""

import pytest
from fastapi.testclient import TestClient

from author_kit.core.llm_aggregator import LLMAPIAggregator
from author_kit.main import create_app


@pytest.fixture
def client():
    app = create_app()
    with TestClient(app) as c:
        yield c


@pytest.fixture
def mock_llm(monkeypatch):
    monkeypatch.setattr(
        LLMAPIAggregator,
        "send_prompt_to_llm",
        lambda self, *a, **k: "mocked-response",
    )

    def _stream(self, *a, **k):
        yield "m"
        yield "ock"

    monkeypatch.setattr(LLMAPIAggregator, "stream_prompt_to_llm", _stream)
