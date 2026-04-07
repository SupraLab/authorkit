# AuthorKit API

FastAPI service for the **AuthorKit** writing assistant. The **VS Code workspace** is the novel project: prose and metadata live under **`.authorkit/`**. The API process also reads **global LLM settings** from a file on the machine where the API runs (default **`~/.authorkit/settings.json`**).

**Author:** Julien Moulin — [julien@supralab.fr](mailto:julien@supralab.fr)  
**License:** MIT — see **`LICENSE`** in this folder.

## Run locally

```bash
cd api
python -m venv .venv
source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install -e ".[dev]"
uvicorn author_kit.main:app --reload --host 127.0.0.1 --port 8765
```

- **Swagger UI:** http://127.0.0.1:8765/docs  
- **OpenAPI JSON:** http://127.0.0.1:8765/openapi.json  

## Persistence & on-disk model

### Per workspace (`workspace_root` query parameter or body field)

All paths below are relative to the opened novel folder.

| Path / file | Purpose |
|-------------|---------|
| `.authorkit/structure.json` | Acts → chapters → scenes (UUIDs, names). |
| `.authorkit/compendium.json` | Categories and entries (names; optional `id` + inline `content`). |
| `.authorkit/scenes/<uuid>.md` | Scene body text. |
| `.authorkit/<category-slug>/<entry-id>.md` | Compendium entry body when the entry has an **`id`** (slug derived from category name). |
| `.authorkit/prompts.json`, `project_settings.json` | Prompts bundle and project settings. |
| `.authorkit/rag/` | RAG index files (when built). |
| **`.authorkit/workshop/workshop.db`** | **SQLite 3** database: workshop **threads**, **messages** (including optional **`context_json`** on user rows: scene UUIDs + compendium excerpt refs), **rolling summary** text for long threads. Created/updated by the API when using **`thread_id`** on workshop routes. |
| `.authorkit/workshop/threads/<thread_id>.json` | Small **metadata** file per thread (title, seed, timestamps) kept in sync with the DB. |

Workshop **history for the LLM** uses stored `role` + `content` only; persisted **context** is for clients (UI tags, insert-target hints), not re-sent as opaque JSON inside chat turns.

### On the API host (not inside the novel folder)

| Location | Purpose |
|----------|---------|
| `AUTHORKIT_SETTINGS_PATH` or **`~/.authorkit/settings.json`** | **Global** LLM profiles (`llm_configs`), active profile, optional app fields. Read/written via **`GET/PUT /v1/app/settings`**. |

## Configuration (environment)

| Env | Purpose |
|-----|---------|
| `AUTHORKIT_SETTINGS_PATH` | Override path to the global settings JSON. |
| `AUTHORKIT_CORS_ORIGINS` | CORS allowlist for browser clients (see `author_kit.config`). |
| `OPENAI_API_KEY` | Used when RAG or embeddings need OpenAI. |
| `AUTHORKIT_OPENAI_EMBEDDING_MODEL` | Optional (default `text-embedding-3-small`). |

## Main endpoints (overview)

| Method | Path | Notes |
|--------|------|-------|
| GET | `/health` | Liveness. |
| GET | `/ready` | Settings file presence + active LLM profile name. |
| GET/PUT | `/v1/app/settings` | Global LLM / app JSON (extension **LLM Settings** panel). |
| POST | `/v1/chat`, `/v1/chat/stream` | Raw chat; messages are **`role` + `content`** only toward the model. |
| POST | `/v1/prompts/preview`, `/v1/generate/prose` | Assembled prompts / prose generation. |
| GET/PUT | `/v1/projects/structure`, `/v1/projects/settings`, `/v1/projects/prompts-file` | Query `workspace_root`. |
| GET/PUT | `/v1/projects/scenes/{uuid}` | Scene Markdown. |
| GET/PUT | `/v1/compendium` | Full compendium JSON document. |
| POST | `/v1/workshop/chat`, `/v1/workshop/chat/stream` | Workshop: optional **`thread_id`**, **`scene_uuids`**, **`compendium_excerpts`**, **`use_rag`**, **`extra_context`**, **`system_prompt`**, provider/model overrides. Stream returns SSE chunks then **`[DONE]`**. |
| GET | `/v1/workshop/threads` | List threads for `workspace_root`. |
| POST | `/v1/workshop/threads` | Create thread (optional seed). |
| GET | `/v1/workshop/threads/{id}/messages` | Full message list; user messages may include **`context`**. |
| PATCH/DELETE | `/v1/workshop/threads/{id}` | Rename / delete thread (cascades messages in SQLite). |
| POST | `/v1/rag/index`, `/v1/rag/query` | `embedding_backend`: `tiktoken` or `openai`. |
| GET | `/v1/rag/status` | Index presence. |
| POST | `/v1/conversation/summarize` | LLM summary of a message list. |

## Tests

```bash
pytest tests/ -q
```
