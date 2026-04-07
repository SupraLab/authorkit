import * as fs from 'fs/promises';
import * as path from 'path';
import * as vscode from 'vscode';

import * as api from './api/client';
import type { CompendiumData } from './api/client';
import { compendiumEntryMarkdownPath } from './compendiumPaths';
import { getApiBaseUrl, requireWorkspaceRoot } from './config';
import { sceneUri } from './scenes';

type InsertMode = 'end' | 'cursor';

async function pickInsertMode(allowCursor: boolean): Promise<InsertMode | undefined> {
  const items: Array<{ label: string; description: string; mode: InsertMode }> = [
    {
      label: 'At the end',
      description: 'Append after existing content',
      mode: 'end',
    },
  ];
  if (allowCursor) {
    items.push({
      label: 'At cursor position',
      description: 'Insert at the cursor in the editor',
      mode: 'cursor',
    });
  }
  const picked = await vscode.window.showQuickPick(items, {
    placeHolder: 'Where to insert the workshop reply?',
  });
  return picked?.mode;
}

async function insertAtUri(uri: vscode.Uri, text: string, mode: InsertMode): Promise<void> {
  let doc = vscode.workspace.textDocuments.find((d) => d.uri.toString() === uri.toString());
  if (!doc) {
    doc = await vscode.workspace.openTextDocument(uri);
  }
  const editor = await vscode.window.showTextDocument(doc, { preview: false });
  const d = editor.document;
  if (mode === 'end') {
    const lastLine = Math.max(0, d.lineCount - 1);
    const atEnd = d.lineAt(lastLine).range.end;
    const prefix = d.getText().length > 0 ? '\n\n' : '';
    await editor.edit((eb) => eb.insert(atEnd, prefix + text));
  } else {
    const pos = editor.selection.active;
    await editor.edit((eb) => eb.insert(pos, text));
  }
}

/** Ensures the scene markdown file exists (empty stub if missing). */
async function ensureSceneFile(root: string, sceneUuid: string): Promise<void> {
  const uri = sceneUri(root, sceneUuid);
  try {
    await fs.access(uri.fsPath);
  } catch {
    const baseUrl = getApiBaseUrl();
    await api.putSceneContent(baseUrl, root, sceneUuid, `# Scene\n\n`);
  }
}

export async function insertWorkshopReplyIntoScene(sceneUuid: string, reply: string): Promise<void> {
  const root = requireWorkspaceRoot();
  const trimmed = reply.trim();
  if (!trimmed) {
    void vscode.window.showWarningMessage('No workshop reply to insert.');
    return;
  }
  const mode = await pickInsertMode(true);
  if (!mode) {
    return;
  }
  try {
    await ensureSceneFile(root, sceneUuid);
    const uri = sceneUri(root, sceneUuid);
    await insertAtUri(uri, trimmed, mode);
    void vscode.window.showInformationMessage('Workshop reply inserted into the scene.');
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(`Could not insert into scene: ${m}`);
  }
}

export async function insertWorkshopReplyIntoCompendium(
  categoryName: string,
  entryName: string,
  reply: string
): Promise<void> {
  const root = requireWorkspaceRoot();
  const trimmed = reply.trim();
  if (!trimmed) {
    void vscode.window.showWarningMessage('No workshop reply to insert.');
    return;
  }
  const baseUrl = getApiBaseUrl();
  let data: CompendiumData;
  try {
    data = (await api.getCompendium(baseUrl, root)) as CompendiumData;
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(`Could not load compendium: ${m}`);
    return;
  }
  const cat = data.categories?.find((c) => c.name === categoryName);
  const entry = cat?.entries?.find((e) => e.name === entryName);
  if (!entry) {
    void vscode.window.showErrorMessage(`Entry “${entryName}” not found in “${categoryName}”.`);
    return;
  }
  const eid = entry.id?.trim();
  try {
    if (eid) {
      const fsPath = compendiumEntryMarkdownPath(root, categoryName, eid);
      try {
        await fs.access(fsPath);
      } catch {
        await fs.mkdir(path.dirname(fsPath), { recursive: true });
        await fs.writeFile(fsPath, `# ${entryName}\n\n${entry.content || ''}`, 'utf-8');
      }
      const mode = await pickInsertMode(true);
      if (!mode) {
        return;
      }
      const uri = vscode.Uri.file(fsPath);
      await insertAtUri(uri, trimmed, mode);
    } else {
      const mode = await pickInsertMode(false);
      if (!mode) {
        return;
      }
      entry.content = `${entry.content || ''}${entry.content ? '\n\n' : ''}${trimmed}`;
      await api.putCompendium(baseUrl, root, data);
    }
    void vscode.window.showInformationMessage(`Workshop reply inserted into “${entryName}”.`);
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(`Could not insert into entry: ${m}`);
  }
}
