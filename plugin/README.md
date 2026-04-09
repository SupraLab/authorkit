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

- **Node.js** (to build this package from source).
- **AuthorKit API** reachable at the URL the extension uses — either:
  - **Managed locally by the extension** (optional): enable **Start local API** so the extension downloads and runs the **standalone** binary from **GitHub Releases** (see below), or
  - **Started by you**: e.g. `uvicorn` / `python -m author_kit` / a PyInstaller build — default `http://127.0.0.1:8765` when **Start local API** is off.
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
| `authorkit.startLocalApi` | `false` | When **true**, the extension downloads (if needed) and runs the **standalone API** on `127.0.0.1` at **Local API port**. **API base URL** is ignored for HTTP calls in this mode. |
| `authorkit.localApiPort` | `8765` | TCP port for the locally managed API (`127.0.0.1`). |
| `authorkit.localApiBinaryPath` | _(empty)_ | Optional **absolute path** to a standalone `author-kit-api` executable (skips GitHub download). Use this for local PyInstaller builds or custom installs. |
| `authorkit.githubApiReleaseTag` | _(empty)_ | Optional GitHub **release tag** for the API zip (e.g. `v0.1.0`). Empty = use **`bundledApiVersion`** from `package.json`. |
| `authorkit.apiBaseUrl` | `http://127.0.0.1:8765` | API origin when **Start local API** is **off** (no trailing slash). |
| `authorkit.charactersCategoryName` | `Characters` | Compendium category for the Characters view. |
| `authorkit.worldCategoryName` | `World` | Compendium category for the World view. |
| `authorkit.activeLlmProfile` | _(empty)_ | Key under API `llm_configs`, sent as workshop **`provider`**. Set via **LLM Settings**. |
| `authorkit.workshopModelOverride` | _(empty)_ | Optional model id for workshop / `@authorKit`. |
| `authorkit.selectionCodeLens` | `true` | CodeLens on selections to add text to the Workshop. |

The extension **`package.json`** includes **`bundledApiVersion`** (e.g. `0.1.0`). That semver (with a `v` prefix on GitHub) selects which **release asset** to download:  
`author-kit-api-<version>-<platform>.zip` from the repository’s **Releases** page.

## Install from GitHub Release

Official builds attach **`author-kit-<semver>.vsix`** to each **[GitHub Release](https://github.com/SupraLab/authorkit/releases)** (same tag as the API zips, e.g. `v0.1.0`). The `<semver>` matches **`version`** in this folder’s **`package.json`**.

1. Download **`author-kit-<semver>.vsix`** for the release you want.
2. VS Code: **Extensions** → **⋯** (Views and More Actions) → **Install from VSIX…**, select the file.
3. Connect the API: enable **Start local API** (downloads **`author-kit-api-<semver>-<platform>.zip`** from that release) or run the API yourself and set **API base URL**. See **Local API bundle** below.

CI builds this artifact in [`.github/workflows/release-api-binaries.yml`](../.github/workflows/release-api-binaries.yml) (`vsce package` on Ubuntu). You can produce the same file locally with **Package (optional)**.

## Local API bundle (GitHub)

When **Start local API** is enabled:

1. The extension resolves your OS/arch to a **platform** id (`linux-x64`, `win-amd64`, `darwin-arm64`, `darwin-x64`).
2. It downloads  
   `https://github.com/<org>/<repo>/releases/download/v<version>/author-kit-api-<version>-<platform>.zip`  
   (org/repo come from the extension’s `repository` field in `package.json`).
3. The zip is extracted under VS Code **global storage** (not inside the workspace), and an `installed.json` manifest records the version.
4. The process is started with **`AUTHORKIT_HOST=127.0.0.1`** and **`AUTHORKIT_PORT`** = **Local API port**. Logs appear in the **Output** channel **AuthorKit API**.

**Re-download AuthorKit API bundle** (Command Palette, or Book Structure toolbar when **Start local API** is on) forces a fresh download and, if the local API is enabled, restarts the process.

**Troubleshooting**

- **404 / download failed:** there may be no GitHub Release yet for that tag and platform, or the version does not match published assets. Build from [`api/`](../api/README.md) and set **Local API binary path**, or run the API manually and turn off **Start local API**.
- **macOS:** Gatekeeper may block an unsigned downloaded binary; you may need to allow it in **Privacy & Security** or remove quarantine (`xattr`) as documented by Apple — same as other downloaded CLI tools.
- **Proxy / offline:** use **Local API binary path** or run **`uvicorn`** yourself.

## Develop and debug

1. `npm install` in **`plugin/`**.
2. **Quality checks (same as CI):**
   ```bash
   npm run lint      # ESLint (TypeScript)
   npm run test      # Vitest unit tests
   npm run compile   # TypeScript → out/
   ```
3. **Run → Start Debugging** (F5) — **Extension Development Host**.
4. Open a folder, run **Initialize Workspace** once if needed.
5. Use the **AuthorKit** activity bar; open **Workshop** for threaded chat and insert actions.

Unit tests (**`*.test.ts`**, Vitest) run in Node **without** the VS Code host. Prefer **pure modules** (`configLogic`, `pathUtils`, `selectionLogic`, `compendiumPaths`, GitHub URL helpers, etc.): `config.ts` and UI code stay thin and only read `vscode.workspace` / `TextDocument` before delegating to those functions.

Code that **must** call `vscode.*` (TreeDataProvider, webviews, `spawn` + health) is harder to unit-test; options are (1) **integration tests** with [`@vscode/test-electron`](https://github.com/microsoft/vscode-test) / the official test CLI, or (2) **injecting** fakes in constructors (larger refactors). Neither is wired in this repo yet.

**Commands** (Command Palette — filter **AuthorKit**): **LLM Settings**, **Initialize Workspace**, refresh trees, **Character** / **World**, Open Scene, Open Entry, **Test API Connection**, **Re-download AuthorKit API bundle**, structure commands (Add Act, Rename…, Move Up/Down, …). **Characters** / **World** title bar **`+`** adds an entry.

## Package (optional)

From **`plugin/`** (same output as the release job):

```bash
npm ci   # or npm install
npx @vscode/vsce package
```

This writes **`author-kit-<version>.vsix`** (from **`name`** + **`version`** in `package.json`). Install it with **Extensions → Install from VSIX…**, or rely on the **`.vsix`** attached to the repo’s GitHub Release for that tag.
