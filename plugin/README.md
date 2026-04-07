# AuthorKit (VS Code extension)

**AuthorKit** for VS Code connects a novel project folder to the **AuthorKit HTTP API** (run from **`api/`** in this repository). The extension drives **structure**, **compendium**, **workshop chat** (threads + context + insert), and optional **native Chat** via **`@authorKit`**.

**Author:** Julien Moulin — [julien@supralab.fr](mailto:julien@supralab.fr)  
**License:** MIT — see **`LICENSE`** in this folder (copyright on this extension: Julien Moulin only).

## What the extension provides

| Area | Behavior |
|------|----------|
| **Book Structure** | Tree: acts → chapters → scenes. Create/rename/reorder/remove; opens scene `.md` files. All changes go through the API (`PUT /v1/projects/structure`, scene CRUD). |
| **Characters / World** | Compendium trees (category names configurable). Add entries, open Markdown sheets when entries have an **`id`**, or legacy inline content. |
| **Workshop** | Dedicated view: pick **thread**, attach **chips** (scenes + compendium entries), send messages, **stream** replies. Tags on user bubbles are **persisted** (via API SQLite). Under each assistant message, **insert** the reply into a linked scene or sheet (**at end** or **at cursor**). |
| **Chat (`@authorKit`)** | Optional Chat participant using the same workshop **stream** and LLM settings; Workshop is the primary UI. |
| **LLM Settings** | Webview loads/saves **`GET/PUT /v1/app/settings`** (same file as **`~/.authorkit/settings.json`** on the API machine). |
| **Status bar** | Shows API reachability and base URL tooltip. |
| **Initialize Workspace** | Creates `.authorkit/` layout via the API for an empty folder. |

The API stores workshop **threads and messages** in **`.authorkit/workshop/workshop.db`** (SQLite) inside the workspace; the extension does not open that file directly—it uses **`/v1/workshop/...`** routes.

## Requirements

- **Node.js** (to build this package).
- A running **AuthorKit API** (e.g. `uvicorn` on the port you configure). Default: `http://127.0.0.1:8765`.
- **Single-folder workspace** — `File → Open Folder`. Only the **first** root folder is used if multiple are open.

## On-disk layout (in the novel folder)

- **`.authorkit/scenes/<uuid>.md`** — scene bodies.  
- **`.authorkit/compendium.json`** — categories and entries.  
- **`.authorkit/<category-slug>/<id>.md`** — compendium sheet when an entry has an **`id`**.  
- **`.authorkit/workshop/workshop.db`** — SQLite: workshop threads/messages (managed by the API).  
- **`.authorkit/workshop/threads/<id>.json`** — thread metadata mirrors (API).  

## Settings

| Key | Default | Purpose |
|-----|---------|---------|
| `authorkit.apiBaseUrl` | `http://127.0.0.1:8765` | API origin (no trailing slash). |
| `authorkit.charactersCategoryName` | `Characters` | Compendium category for the Characters view. |
| `authorkit.worldCategoryName` | `World` | Compendium category for the World view. |
| `authorkit.activeLlmProfile` | _(empty)_ | Key under API `llm_configs`, sent as workshop **`provider`**. Set via **LLM Settings**. |
| `authorkit.workshopModelOverride` | _(empty)_ | Optional model id for workshop / `@authorKit`. |

## Develop and debug

1. `npm install` and `npm run compile` in **`plugin/`**.
2. **Run → Start Debugging** (F5) — **Extension Development Host**.
3. Open a folder, run **Initialize Workspace** once if needed.
4. Use the **AuthorKit** activity bar; open **Workshop** for threaded chat and insert actions.

**Commands** (Command Palette — filter **AuthorKit**): **LLM Settings**, **Initialize Workspace**, refresh trees, **Character** / **World**, Open Scene, Open Entry, Test API Connection, structure commands (Add Act, Rename…, Move Up/Down, …). **Characters** / **World** title bar **`+`** adds an entry.

## Package (optional)

```bash
npm install -g @vscode/vsce
vsce package
```

Install the resulting **`.vsix`** with **Extensions: Install from VSIX…**.
