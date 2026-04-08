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
      label: vscode.l10n.t('At the end'),
      description: vscode.l10n.t('Append after existing content'),
      mode: 'end',
    },
  ];
  if (allowCursor) {
    items.push({
      label: vscode.l10n.t('At cursor position'),
      description: vscode.l10n.t(
        'Insert at the cursor; replaces selected text when there is a selection.'
      ),
      mode: 'cursor',
    });
  }
  const picked = await vscode.window.showQuickPick(items, {
    placeHolder: vscode.l10n.t('Where to insert the workshop reply?'),
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
    await editor.edit((eb) => {
      for (const sel of editor.selections) {
        if (sel.isEmpty) {
          eb.insert(sel.active, text);
        } else {
          eb.replace(sel, text);
        }
      }
    });
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
    void vscode.window.showWarningMessage(vscode.l10n.t('No workshop reply to insert.'));
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
    void vscode.window.showInformationMessage(
      vscode.l10n.t('Workshop reply inserted into the scene.')
    );
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(vscode.l10n.t('Could not insert into scene: {0}', m));
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
    void vscode.window.showWarningMessage(vscode.l10n.t('No workshop reply to insert.'));
    return;
  }
  const baseUrl = getApiBaseUrl();
  let data: CompendiumData;
  try {
    data = (await api.getCompendium(baseUrl, root)) as CompendiumData;
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(vscode.l10n.t('Could not load compendium: {0}', m));
    return;
  }
  const cat = data.categories?.find((c) => c.name === categoryName);
  const entry = cat?.entries?.find((e) => e.name === entryName);
  if (!entry) {
    void vscode.window.showErrorMessage(
      vscode.l10n.t('Entry \u201c{0}\u201d not found in \u201c{1}\u201d.', entryName, categoryName)
    );
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
    void vscode.window.showInformationMessage(
      vscode.l10n.t('Workshop reply inserted into \u201c{0}\u201d.', entryName)
    );
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(vscode.l10n.t('Could not insert into entry: {0}', m));
  }
}
