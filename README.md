<p align="center">
  <img src="authorkit.png" alt="AuthorKit" width="650" />
</p>

**AuthorKit** evolves the ideas of [**Writingway**](https://github.com/aomukai/Writingway) (creative writing assistant) into a **decoupled** stack: a standalone **HTTP API** ([`api/`](api/README.md)) and a **Visual Studio Code extension** ([`plugin/`](plugin/README.md)). The user opens a **novel project folder** in VS Code; the API reads and writes project files under **`.authorkit/`** inside that workspace (and keeps workshop chat history in a **SQLite** database there).

## Authors

- **Julien Moulin** — [julien@supralab.fr](mailto:julien@supralab.fr)

Upstream **Writingway** is © aomukai (MIT). See **`LICENSE`**.

## What it does

### API ([`api/`](api/README.md))

- **FastAPI** service: **`/v1/...`**, **`/health`**, **`/ready`**, OpenAPI at **`/docs`**.
- **Project on disk** (per `workspace_root`): **structure** (acts → chapters → scenes), **compendium** JSON, **scene** Markdown (`.authorkit/scenes/<uuid>.md`), optional **per-entry** Markdown for compendium rows that have an **`id`** (`.authorkit/<category-slug>/<id>.md`), **prompts** / **project settings** JSON.
- **Workshop:** augmented chat with optional **scene** and **compendium** context, **`use_rag`**, streaming (**SSE**). LLM calls go through a **multi-provider** stack (profiles in settings).
- **Workshop threads:** when `thread_id` is sent, the API loads history from **SQLite** (`.authorkit/workshop/workshop.db`), appends turns, stores **structured context** on user messages (`scene_uuids`, `compendium_excerpts`), maintains a **rolling summary** when the thread grows past a hot window, and writes small **JSON** anchors under `.authorkit/workshop/threads/<id>.json`. See the [**API README**](api/README.md) (persistence, endpoints).
- **Global app settings:** `GET/PUT /v1/app/settings` → file on the API host (default **`~/.authorkit/settings.json`**): LLM profiles, keys, timeouts.
- **Other:** raw **`/v1/chat`** (sync + stream), **prompt preview** / **prose generate**, **RAG** index/query, **conversation summarize**.

### VS Code extension ([`plugin/`](plugin/README.md))

- **Trees:** **Book Structure**, **Characters**, **World** — structure and compendium editing through the API.
- **Workshop** view: **thread** list (create / rename / delete / switch), **context chips** (scenes + compendium), **streaming** assistant replies, **persisted tags** on user messages after reload, **insert the assistant reply** into a scene or compendium sheet (**at end** or **at cursor**).
- **Chat:** optional **`@authorKit`** participant (same workshop pipeline); the **Workshop** panel is the main UI.
- **LLM Settings** webview, **status bar** API indicator, **Initialize Workspace**, commands listed in the [**extension README**](plugin/README.md).

## Data layout (high level)

| Location | Role |
|----------|------|
| **`.authorkit/`** in the novel workspace | Structure, compendium, scenes, prompts, RAG dir, **workshop** SQLite + thread JSON |
| **`~/.authorkit/settings.json`** (API host) | Global LLM profiles and app settings (used by the extension’s LLM panel) |

## Repository layout

| Folder | Purpose |
|--------|---------|
| [`api/`](api/README.md) | AuthorKit **HTTP service** (Python). |
| [`plugin/`](plugin/README.md) | **VS Code extension** (TypeScript). |
| `source/` | *(Optional)* Legacy Writingway **PyQt** app—not required for AuthorKit. |

## Quick start

1. **API:** follow [**api/README.md**](api/README.md) — `uvicorn` on `127.0.0.1:8765` by default.
2. **Extension:** follow [**plugin/README.md**](plugin/README.md) — `npm install`, `npm run compile`, F5 to debug.
3. In VS Code, **open a folder** (single-root workspace), run **Initialize Workspace** once, then use the AuthorKit views.

## License

- **Root** and **`api/`**: **MIT** — see **`LICENSE`** in each place. Retains **Writingway** © aomukai and **AuthorKit** (API / stack) © Julien Moulin.
- **`plugin/`** (VS Code extension): **MIT** — see **`plugin/LICENSE`**. Copyright **Julien Moulin** only (extension code is not derived from the legacy Writingway desktop app).
