import * as vscode from 'vscode';

import {
  localApiHttpBase,
  normalizeLocalApiPort,
  resolveApiBaseUrl,
  workshopLlmOptionsFromStrings,
} from './configLogic';

const SECTION = 'authorkit';

export function getStartLocalApi(): boolean {
  return vscode.workspace.getConfiguration(SECTION).get<boolean>('startLocalApi') ?? false;
}

export function getLocalApiPort(): number {
  return normalizeLocalApiPort(vscode.workspace.getConfiguration(SECTION).get<number>('localApiPort'));
}

export function getLocalApiBinaryPath(): string {
  return vscode.workspace.getConfiguration(SECTION).get<string>('localApiBinaryPath')?.trim() ?? '';
}

/** Optional override for GitHub release tag (e.g. `v0.1.0`). Empty = use `bundledApiVersion` from package.json. */
export function getGithubApiReleaseTag(): string {
  return vscode.workspace.getConfiguration(SECTION).get<string>('githubApiReleaseTag')?.trim() ?? '';
}

/** Base URL when **Start local API** is on (`127.0.0.1` + configured port). */
export function localApiBaseUrl(): string {
  const cfg = vscode.workspace.getConfiguration(SECTION);
  return localApiHttpBase(normalizeLocalApiPort(cfg.get<number>('localApiPort')));
}

export function getApiBaseUrl(): string {
  const cfg = vscode.workspace.getConfiguration(SECTION);
  return resolveApiBaseUrl({
    startLocalApi: cfg.get<boolean>('startLocalApi') ?? false,
    localApiPort: cfg.get<number>('localApiPort'),
    apiBaseUrl: cfg.get<string>('apiBaseUrl'),
  });
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
  return workshopLlmOptionsFromStrings(
    cfg.get<string>('activeLlmProfile'),
    cfg.get<string>('workshopModelOverride')
  );
}
