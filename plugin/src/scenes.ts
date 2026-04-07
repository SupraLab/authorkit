import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs/promises';

import * as api from './api/client';
import { getApiBaseUrl, requireWorkspaceRoot } from './config';

/** Path to `.authorkit/scenes/<uuid>.md` */
export function sceneUri(workspaceRoot: string, sceneUuid: string): vscode.Uri {
  return vscode.Uri.file(path.join(workspaceRoot, '.authorkit', 'scenes', `${sceneUuid}.md`));
}

/** Best-effort delete `.authorkit/scenes/<uuid>.md` when removing a scene from the structure. */
export async function deleteSceneMarkdownFile(workspaceRoot: string, sceneUuid: string): Promise<void> {
  const uri = sceneUri(workspaceRoot, sceneUuid);
  try {
    await fs.unlink(uri.fsPath);
  } catch (e) {
    const err = e as NodeJS.ErrnoException;
    if (err.code !== 'ENOENT') {
      throw e;
    }
  }
}

export async function openSceneDocument(sceneUuid: string): Promise<void> {
  const root = requireWorkspaceRoot();
  const uri = sceneUri(root, sceneUuid);
  try {
    await fs.access(uri.fsPath);
  } catch {
    const baseUrl = getApiBaseUrl();
    await api.putSceneContent(baseUrl, root, sceneUuid, `# Scene\n\n`);
  }
  const doc = await vscode.workspace.openTextDocument(uri);
  await vscode.window.showTextDocument(doc, { preview: false });
}
