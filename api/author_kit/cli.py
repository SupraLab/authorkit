"""CLI entry for packaged / standalone runs (`author-kit-api`, `python -m author_kit`)."""

from __future__ import annotations

import os


def main() -> None:
    import uvicorn

    # Allow one-shot overrides without loading .env from CWD when unset
    os.environ.setdefault("AUTHORKIT_HOST", "127.0.0.1")
    os.environ.setdefault("AUTHORKIT_PORT", "8765")

    from author_kit.config import get_author_kit_settings

    cfg = get_author_kit_settings()
    uvicorn.run(
        "author_kit.main:app",
        host=cfg.host,
        port=cfg.port,
        factory=False,
        log_level="info",
    )


if __name__ == "__main__":
    main()
