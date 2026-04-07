import * as fs from 'fs/promises';
import * as path from 'path';
import * as vscode from 'vscode';

/** Matches init: project exists on disk once structure has been saved. */
export const REL_AUTHORKIT_STRUCTURE = path.join('.authorkit', 'structure.json');

export const CTX_HAS_PROJECT = 'authorkit.hasProject';

/** Fast path: AuthorKit at workspace root. */
export async function workspaceHasAuthorkitStructure(root: string): Promise<boolean> {
  try {
    await fs.access(path.join(root, REL_AUTHORKIT_STRUCTURE));
    return true;
  } catch {
    return false;
  }
}

/**
 * True if any `.authorkit/structure.json` exists under the first workspace folder
 * (root or nested, e.g. monorepo with the novel in a subfolder).
 */
export async function updateAuthorkitProjectContext(): Promise<void> {
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) {
    await vscode.commands.executeCommand('setContext', CTX_HAS_PROJECT, false);
    return;
  }
  const root = folder.uri.fsPath;
  let has = await workspaceHasAuthorkitStructure(root);
  if (!has) {
    const found = await vscode.workspace.findFiles(
      new vscode.RelativePattern(folder, '**/.authorkit/structure.json'),
      '**/{node_modules,.git,out,dist,.vscode-test}/**',
      1
    );
    has = found.length > 0;
  }
  await vscode.commands.executeCommand('setContext', CTX_HAS_PROJECT, has);
}
