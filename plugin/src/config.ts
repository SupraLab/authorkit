import * as vscode from 'vscode';

const SECTION = 'authorkit';

export function getApiBaseUrl(): string {
  const v = vscode.workspace.getConfiguration(SECTION).get<string>('apiBaseUrl');
  return (v || 'http://127.0.0.1:8765').replace(/\/$/, '');
}

export function getCharactersCategoryName(): string {
  return vscode.workspace.getConfiguration(SECTION).get<string>('charactersCategoryName') || 'Characters';
}

export function getWorldCategoryName(): string {
  return vscode.workspace.getConfiguration(SECTION).get<string>('worldCategoryName') || 'World';
}

/** Single-folder workspace root, or undefined. */
export function getWorkspaceRoot(): string | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders?.length) {
    return undefined;
  }
  if (folders.length > 1) {
    void vscode.window.showWarningMessage(
      vscode.l10n.t(
        'AuthorKit: multiple workspace folders are open; only the first folder is used.'
      )
    );
  }
  return folders[0].uri.fsPath;
}

export function requireWorkspaceRoot(): string {
  const root = getWorkspaceRoot();
  if (!root) {
    throw new Error(vscode.l10n.t('Open a folder (File > Open Folder) to use AuthorKit.'));
  }
  return root;
}

/**
 * Workshop chat: profile key in API `llm_configs` and optional model override.
 * Empty profile falls back to the API file's `active_llm_config`.
 */
export function getWorkshopLlmOptions(): { provider?: string; model?: string } {
  const cfg = vscode.workspace.getConfiguration(SECTION);
  const profile = cfg.get<string>('activeLlmProfile')?.trim();
  const model = cfg.get<string>('workshopModelOverride')?.trim();
  const o: { provider?: string; model?: string } = {};
  if (profile) {
    o.provider = profile;
  }
  if (model) {
    o.model = model;
  }
  return o;
}
