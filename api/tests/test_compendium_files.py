"""Compendium entries can store prose in `.authorkit/<category>/<uuid>.md`."""

from pathlib import Path

from author_kit.core import compendium_store


def test_get_entry_text_prefers_markdown_file(tmp_path: Path):
    uid = "550e8400-e29b-41d4-a716-446655440000"
    md = tmp_path / ".authorkit" / "characters" / f"{uid}.md"
    md.parent.mkdir(parents=True)
    md.write_text("# Hero\n\nFrom file.", encoding="utf-8")
    data = {
        "categories": [
            {
                "name": "Characters",
                "entries": [
                    {"name": "Hero", "content": "from json", "id": uid},
                ],
            }
        ]
    }
    text = compendium_store.get_entry_text(data, "Characters", "Hero", tmp_path)
    assert "From file." in text
    assert "from json" not in text


def test_get_entry_text_fallback_json(tmp_path: Path):
    data = {
        "categories": [
            {
                "name": "Characters",
                "entries": [{"name": "Hero", "content": "only json"}],
            }
        ]
    }
    text = compendium_store.get_entry_text(data, "Characters", "Hero", tmp_path)
    assert text == "only json"
