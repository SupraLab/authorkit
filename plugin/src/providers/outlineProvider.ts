import * as vscode from 'vscode';
import * as api from '../api/client';
import type { StructureData } from '../api/client';
import { getApiBaseUrl, getWorkspaceRoot } from '../config';

export type OutlineTreeItem =
  | { kind: 'loading' | 'empty' | 'error'; id: string; label: string; message?: string }
  | {
      kind: 'act';
      id: string;
      label: string;
      actIndex: number;
    }
  | {
      kind: 'chapter';
      id: string;
      label: string;
      actIndex: number;
      chapterIndex: number;
    }
  | {
      kind: 'scene';
      id: string;
      label: string;
      uuid: string;
      actIndex: number;
      chapterIndex: number;
      sceneIndex: number;
    };

export class OutlineProvider implements vscode.TreeDataProvider<OutlineTreeItem> {
  private _onDidChange = new vscode.EventEmitter<OutlineTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChange.event;

  refresh(): void {
    this._onDidChange.fire(undefined);
  }

  getTreeItem(element: OutlineTreeItem): vscode.TreeItem {
    if (element.kind === 'loading' || element.kind === 'empty' || element.kind === 'error') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);
      ti.description = element.message;
      return ti;
    }
    if (element.kind === 'act') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.Expanded);
      ti.contextValue = 'authorkit.act';
      return ti;
    }
    if (element.kind === 'chapter') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.Expanded);
      ti.contextValue = 'authorkit.chapter';
      return ti;
    }
    if (element.kind === 'scene') {
      const ti = new vscode.TreeItem(element.label, vscode.TreeItemCollapsibleState.None);
      ti.contextValue = 'authorkit.scene';
      ti.command = {
        command: 'authorkit.openScene',
        title: vscode.l10n.t('Open scene'),
        arguments: [element.uuid],
      };
      return ti;
    }
    return new vscode.TreeItem('?', vscode.TreeItemCollapsibleState.None);
  }

  async getChildren(element?: OutlineTreeItem): Promise<OutlineTreeItem[]> {
    const root = getWorkspaceRoot();
    if (!root) {
      return [
        {
          kind: 'empty',
          id: 'no-ws',
          label: vscode.l10n.t('Open a folder to see the book structure.'),
        },
      ];
    }
    const baseUrl = getApiBaseUrl();

    if (!element) {
      try {
        const structure = (await api.getStructure(baseUrl, root)) as StructureData;
        const acts = structure.acts as Array<{
          name?: string;
          chapters?: Array<{
            name?: string;
            scenes?: Array<{ name?: string; uuid?: string }>;
          }>;
        }>;
        if (!acts?.length) {
          return [{ kind: 'empty', id: 'no-acts', label: vscode.l10n.t('No acts in structure.') }];
        }
        return acts.map((act, actIndex) => ({
          kind: 'act' as const,
          id: `act-${actIndex}`,
          label: act.name || vscode.l10n.t('Act {0}', String(actIndex + 1)),
          actIndex,
        }));
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        return [
          {
            kind: 'error',
            id: 'err',
            label: vscode.l10n.t('Failed to load book structure'),
            message: msg,
          },
        ];
      }
    }

    if (element.kind === 'loading' || element.kind === 'empty' || element.kind === 'error') {
      return [];
    }

    const structure = (await api.getStructure(baseUrl, root)) as StructureData;
    const acts = structure.acts as Array<{
      chapters?: Array<{
        name?: string;
        scenes?: Array<{ name?: string; uuid?: string }>;
      }>;
    }>;

    if (element.kind === 'act') {
      const chapters = acts[element.actIndex]?.chapters || [];
      return chapters.map((ch, chapterIndex) => ({
        kind: 'chapter' as const,
        id: `act-${element.actIndex}-ch-${chapterIndex}`,
        label: ch.name || vscode.l10n.t('Chapter {0}', String(chapterIndex + 1)),
        actIndex: element.actIndex,
        chapterIndex,
      }));
    }

    if (element.kind === 'chapter') {
      const scenes = acts[element.actIndex]?.chapters?.[element.chapterIndex]?.scenes || [];
      return scenes.map((sc, sceneIndex) => {
        const uuid = sc.uuid || `missing-uuid-${element.actIndex}-${element.chapterIndex}-${sceneIndex}`;
        return {
          kind: 'scene' as const,
          id: `scene-${uuid}`,
          label: sc.name || vscode.l10n.t('Scene {0}', String(sceneIndex + 1)),
          uuid,
          actIndex: element.actIndex,
          chapterIndex: element.chapterIndex,
          sceneIndex,
        };
      });
    }

    return [];
  }

  getParent(_element: OutlineTreeItem): vscode.ProviderResult<OutlineTreeItem> {
    return undefined;
  }
}
