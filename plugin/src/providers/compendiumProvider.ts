import * as vscode from 'vscode';
import * as api from '../api/client';
import type { CompendiumData } from '../api/client';
import { getApiBaseUrl, getWorkspaceRoot } from '../config';

export type EntryItem =
  | { kind: 'loading' | 'empty' | 'error'; id: string; label: string; message?: string }
  | {
      kind: 'entry';
      id: string;
      label: string;
      categoryName: string;
      entryName: string;
      /** Stable id for `.authorkit/<slug>/<id>.md` when present on the API entry. */
      entryId?: string;
    };

export class CompendiumProvider implements vscode.TreeDataProvider<EntryItem> {
  constructor(private readonly getCategoryName: () => string) {}

  private _onDidChange = new vscode.EventEmitter<EntryItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChange.event;

  refresh(): void {
    this._onDidChange.fire(undefined);
  }

  getTreeItem(element: EntryItem): vscode.TreeItem {
    if (element.kind === 'loading' || element.kind === 'empty' || element.kind === 'error') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);
      ti.description = element.message;
      return ti;
    }
    if (element.kind === 'entry') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);
      ti.contextValue = 'authorkit.compentry';
      ti.command = {
        command: 'authorkit.openEntryDetail',
        title: 'Open',
        arguments: [element.categoryName, element.entryName],
      };
      return ti;
    }
    return new vscode.TreeItem('?', vscode.TreeItemCollapsibleState.None);
  }

  async getChildren(_element?: EntryItem): Promise<EntryItem[]> {
    const root = getWorkspaceRoot();
    if (!root) {
      return [{ kind: 'empty', id: 'no-ws', label: 'Open a folder first.' }];
    }
    const baseUrl = getApiBaseUrl();
    try {
      const data = (await api.getCompendium(baseUrl, root)) as CompendiumData;
      const categoryName = this.getCategoryName();
      const cat = data.categories?.find((c) => c.name === categoryName);
      if (!cat) {
        return [
          {
            kind: 'empty',
            id: 'no-cat',
            label: `No “${categoryName}” category yet. Use “New …”.`,
          },
        ];
      }
      const entries = cat.entries || [];
      if (!entries.length) {
        return [{ kind: 'empty', id: 'no-ent', label: 'No entries yet.' }];
      }
      return entries.map((e, i) => ({
        kind: 'entry' as const,
        id: e.id ? `ent-${e.id}` : `ent-${categoryName}-${i}-${e.name}`,
        label: e.name,
        categoryName,
        entryName: e.name,
        entryId: e.id,
      }));
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      return [{ kind: 'error', id: 'err', label: 'Failed to load compendium', message: msg }];
    }
  }
}
