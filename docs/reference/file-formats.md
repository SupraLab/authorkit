# File formats

All JSON is UTF-8 and pretty-printed. Unknown fields are ignored on read; fields added later use
`#[serde(default)]` so older files keep loading.

## `authorkit.toml`

```toml
[book]
title = "Mon roman"
author = "Jane Doe"
language = "fr"
status = "brouillon"
premise = "…"

[styles]
rules = "…"
voice_notes = "…"
avoid = "…"
```

## `manuscript/index.json`

```json
{
  "acts": [
    {
      "id": "uuid",
      "title": "Acte I",
      "chapters": [
        { "id": "uuid", "title": "Chapitre 1", "scene_ids": ["uuid", "uuid"] }
      ]
    }
  ]
}
```

The index is the only source of ordering.

## Scene — `manuscript/scenes/<slug>-<id8>.md`

JSON frontmatter between `---` lines, then the Markdown body:

```markdown
---
{
  "id": "6f1c…",
  "title": "Scène d'ouverture",
  "links": ["note-uuid"],
  "synopsis": "…",
  "status": "draft"
}
---

Body in **Markdown**.
```

`status`: `to_write`, `draft`, `to_revise`, `done`. A leading BOM is tolerated. A missing or empty
`id` makes the file unreadable (reported by diagnostics, skipped at load).

## Note — `notes/<kind>/<slug>-<id8>.md`

```markdown
---
{
  "id": "…",
  "name": "Aston",
  "kind": "character",
  "links": []
}
---

# Apparence
…
```

`kind`: `character`, `setting`, `lore`.

## Chat

`chat/<id>.json` (`StoredConversation`, `format_version: 2`):

| Field | Notes |
|-------|-------|
| `id`, `title`, `title_generated` | Title is local until `title_generated` |
| `context_id` | `scene:<id>` / `note:<id>` |
| `updated_at` | `YYYY-MM-DD HH:MM` local time |
| `messages[]` | `StoredMessage { id, role, content, status, created_at, payload }` |
| `draft`, `composer_mode`, `attachment` | Composer state survives restarts |

`status`: `pending`, `streaming`, `done`, `cancelled`, `error`.

`payload` is tagged by `kind`:

| `kind` | Fields |
|--------|--------|
| `chat` | — |
| `user_quote` | `quote`, `label`, `note`, `tool` (`"chat"`, `"brush:<id>"`, `"continue:<mode>"`) |
| `edit_proposal` | `label`, `mode`, `range_start`, `range_end`, `doc_id`, `doc_kind`, `original_slice`, `anchor_before`, `anchor_after`, `proposal_status`, `conflict`, `error`, `job_id`, `regen` |

`composer_mode` is tagged by `kind`: `chat`, `continue { mode }`, `brush { id }`.

`chat/index.json`: `{ "active_id", "active_by_context": { "<context>": "<conv id>" }, "conversations": [ids] }`.

Legacy conversations (bare `ChatMessage` arrays, `scene_id` instead of `context_id`) are migrated
on load.

## Trash entry — `trash/<id>.json`

`{ "id", "created_at" (RFC 3339), "label", "kind": "scene" | "chapter" | "act" | "note", … }`
with kind-specific payload (metadata, body, original position `index_pos`, nested scenes).

## Version snapshot — `versions/<kind>/<doc-id>/<uuid>.json`

`{ "id", "created_at", "doc_kind", "doc_id", "title", "reason", "body", "meta_json" }`.

## Review — `reviews/<scene-id>.json`

`{ "format_version": 1, "items": [ReviewItem], "raw", "scene_id", "text_hash", "created_at" }`.
