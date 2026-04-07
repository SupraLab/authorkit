import * as fs from 'fs/promises';
import * as path from 'path';
import * as vscode from 'vscode';
import * as api from '../api/client';
import type { CompendiumData } from '../api/client';
import { compendiumEntryMarkdownPath } from '../compendiumPaths';
import { getApiBaseUrl, requireWorkspaceRoot } from '../config';

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

export async function openEntryDetailPanel(
  context: vscode.ExtensionContext,
  categoryName: string,
  entryName: string
): Promise<void> {
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const data = (await api.getCompendium(baseUrl, root)) as CompendiumData;
  const cat = data.categories?.find((c) => c.name === categoryName);
  const entry = cat?.entries?.find((e) => e.name === entryName);
  if (!entry) {
    void vscode.window.showErrorMessage(`Entry “${entryName}” not found in ${categoryName}.`);
    return;
  }

  const eid = entry.id?.trim();
  if (eid) {
    const fsPath = compendiumEntryMarkdownPath(root, categoryName, eid);
    try {
      await fs.access(fsPath);
    } catch {
      await fs.mkdir(path.dirname(fsPath), { recursive: true });
      await fs.writeFile(fsPath, `# ${entryName}\n\n${entry.content || ''}`, 'utf-8');
    }
    const uri = vscode.Uri.file(fsPath);
    const doc = await vscode.workspace.openTextDocument(uri);
    await vscode.window.showTextDocument(doc, { preview: false });
    return;
  }

  const panel = vscode.window.createWebviewPanel(
    'authorkitEntryDetail',
    `${categoryName}: ${entryName}`,
    vscode.ViewColumn.One,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  panel.webview.html = getWebviewHtml(categoryName, entryName, entry.content);

  panel.webview.onDidReceiveMessage(
    async (msg: { type?: string; content?: string }) => {
      if (msg.type !== 'save' || typeof msg.content !== 'string') {
        return;
      }
      try {
        const fresh = (await api.getCompendium(baseUrl, root)) as CompendiumData;
        const c = fresh.categories?.find((x) => x.name === categoryName);
        const e = c?.entries?.find((x) => x.name === entryName);
        if (!e) {
          void vscode.window.showErrorMessage('Entry disappeared; refresh the tree.');
          return;
        }
        e.content = msg.content;
        await api.putCompendium(baseUrl, root, fresh);
        void vscode.window.showInformationMessage('Saved compendium entry.');
      } catch (err) {
        const m = err instanceof Error ? err.message : String(err);
        void vscode.window.showErrorMessage(`Save failed: ${m}`);
      }
    },
    undefined,
    context.subscriptions
  );
}

function getWebviewHtml(categoryName: string, entryName: string, content: string): string {
  const esc = escapeHtml(content);
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    body { font-family: var(--vscode-font-family); color: var(--vscode-foreground); padding: 1rem; }
    h1 { font-size: 1.25rem; margin: 0 0 0.75rem 0; }
    .meta { opacity: 0.8; font-size: 0.85rem; margin-bottom: 1rem; }
    textarea {
      width: 100%; min-height: 320px; box-sizing: border-box;
      font-family: var(--vscode-editor-font-family); font-size: var(--vscode-editor-font-size);
      background: var(--vscode-input-background); color: var(--vscode-input-foreground);
      border: 1px solid var(--vscode-input-border, #555); padding: 0.5rem;
    }
    button {
      margin-top: 0.75rem; padding: 0.4rem 0.9rem;
      background: var(--vscode-button-background); color: var(--vscode-button-foreground);
      border: none; cursor: pointer;
    }
    button:hover { background: var(--vscode-button-hoverBackground); }
  </style>
</head>
<body>
  <h1>${escapeHtml(entryName)}</h1>
  <div class="meta">${escapeHtml(categoryName)}</div>
  <textarea id="body">${esc}</textarea>
  <div>
    <button type="button" id="save">Save to compendium</button>
  </div>
  <script>
    (function() {
      const vscode = acquireVsCodeApi();
      document.getElementById('save').addEventListener('click', function() {
        var content = document.getElementById('body').value;
        vscode.postMessage({ type: 'save', content: content });
      });
    })();
  </script>
</body>
</html>`;
}
