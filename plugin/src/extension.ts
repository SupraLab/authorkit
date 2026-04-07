import * as vscode from 'vscode';
import * as api from './api/client';
import { registerAuthorkitChatParticipant } from './chat/authorkitParticipant';
import { createCompendiumEntry } from './commands/createEntry';
import { openCompendiumChatFromTreeItem } from './commands/openCompendiumChat';
import { initWorkspace } from './commands/initWorkspace';
import {
  addAct,
  addChapter,
  addScene,
  deleteStructureNode,
  moveStructureDown,
  moveStructureUp,
} from './commands/structureManage';
import { renameStructureNode } from './commands/renameStructure';
import { getApiBaseUrl, getCharactersCategoryName, getWorldCategoryName } from './config';
import { CompendiumProvider, type EntryItem } from './providers/compendiumProvider';
import { OutlineProvider, type OutlineTreeItem } from './providers/outlineProvider';
import { openSceneDocument } from './scenes';
import { openEntryDetailPanel } from './views/entryDetail';
import { openLlmSettingsPanel } from './views/llmSettingsPanel';
import { openWorkshopChatPanel, registerWorkshopView } from './views/workshopChatPanel';
import { CTX_HAS_PROJECT, updateAuthorkitProjectContext } from './workspaceContext';
import { isValidUuid } from './uuidUtil';

const AUTHORKIT_CFG = 'authorkit';

export function activate(context: vscode.ExtensionContext): void {
  void vscode.commands.executeCommand('setContext', CTX_HAS_PROJECT, false);
  const outlineProvider = new OutlineProvider();
  const charactersProvider = new CompendiumProvider(() => getCharactersCategoryName());
  const worldProvider = new CompendiumProvider(() => getWorldCategoryName());

  const outlineTreeView = vscode.window.createTreeView('authorkit.outline', {
    treeDataProvider: outlineProvider,
    showCollapseAll: true,
  });
  context.subscriptions.push(
    outlineTreeView,
    vscode.window.registerTreeDataProvider('authorkit.characters', charactersProvider),
    vscode.window.registerTreeDataProvider('authorkit.world', worldProvider)
  );

  const refreshTrees = (): void => {
    outlineProvider.refresh();
    charactersProvider.refresh();
    worldProvider.refresh();
  };

  const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  statusBar.command = 'authorkit.testConnection';

  async function updateConnectionStatus(): Promise<void> {
    const base = getApiBaseUrl();
    statusBar.tooltip = `API ${base}\nClick to test connection`;
    try {
      await api.health(base);
      statusBar.text = '$(pass) API';
      statusBar.backgroundColor = undefined;
    } catch {
      statusBar.text = '$(error) API';
      statusBar.backgroundColor = new vscode.ThemeColor('statusBarItem.errorBackground');
    }
    statusBar.show();
  }

  context.subscriptions.push(
    vscode.commands.registerCommand('authorkit.refresh', () => {
      refreshTrees();
      void updateAuthorkitProjectContext();
    }),
    vscode.commands.registerCommand(
      'authorkit.renameStructureNode',
      async (item?: OutlineTreeItem) => {
        try {
          await renameStructureNode(item ?? outlineTreeView.selection[0], outlineProvider);
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          void vscode.window.showErrorMessage(msg);
        }
      }
    ),
    vscode.commands.registerCommand('authorkit.addAct', async (item?: OutlineTreeItem) => {
      try {
        await addAct(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.addChapter', async (item?: OutlineTreeItem) => {
      try {
        await addChapter(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.addScene', async (item?: OutlineTreeItem) => {
      try {
        await addScene(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.deleteStructureNode', async (item?: OutlineTreeItem) => {
      try {
        await deleteStructureNode(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.moveStructureUp', async (item?: OutlineTreeItem) => {
      try {
        await moveStructureUp(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.moveStructureDown', async (item?: OutlineTreeItem) => {
      try {
        await moveStructureDown(item ?? outlineTreeView.selection[0], outlineProvider);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.initWorkspace', async () => {
      try {
        await initWorkspace(refreshTrees);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.openLlmSettings', async () => {
      try {
        await openLlmSettingsPanel(context);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.createCharacter', async () => {
      try {
        await createCompendiumEntry(context, getCharactersCategoryName());
        charactersProvider.refresh();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.createWorldEntry', async () => {
      try {
        await createCompendiumEntry(context, getWorldCategoryName());
        worldProvider.refresh();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.openScene', async (sceneUuid?: string) => {
      const id = typeof sceneUuid === 'string' ? sceneUuid : undefined;
      if (!id?.trim()) {
        void vscode.window.showErrorMessage('No scene UUID.');
        return;
      }
      if (!isValidUuid(id)) {
        void vscode.window.showErrorMessage(
          'This scene does not have a valid UUID in the project structure.'
        );
        return;
      }
      try {
        await openSceneDocument(id);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand(
      'authorkit.openEntryDetail',
      async (categoryName?: string, entryName?: string) => {
        if (typeof categoryName !== 'string' || typeof entryName !== 'string') {
          return;
        }
        try {
          await openEntryDetailPanel(context, categoryName, entryName);
        } catch (e) {
          const msg = e instanceof Error ? e.message : String(e);
          void vscode.window.showErrorMessage(msg);
        }
      }
    ),
    vscode.commands.registerCommand('authorkit.openWorkshop', async () => {
      try {
        openWorkshopChatPanel(context);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.openCompendiumChat', async (item?: EntryItem) => {
      try {
        await openCompendiumChatFromTreeItem(context, item);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(msg);
      }
    }),
    vscode.commands.registerCommand('authorkit.testConnection', async () => {
      const base = getApiBaseUrl();
      try {
        const h = await api.health(base);
        let detail = '';
        try {
          const r = await api.ready(base);
          detail = ` Ready: ${JSON.stringify(r)}`;
        } catch {
          detail = ' (/ready failed)';
        }
        void vscode.window.showInformationMessage(`API OK — ${base} (${h.status ?? 'ok'})${detail}`);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(`API unreachable (${base}): ${msg}`);
      }
      await updateConnectionStatus();
    }),
    statusBar,
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration(AUTHORKIT_CFG)) {
        refreshTrees();
        void updateConnectionStatus();
      }
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(() => {
      refreshTrees();
      void updateConnectionStatus();
      void updateAuthorkitProjectContext();
    })
  );

  registerAuthorkitChatParticipant(context);
  registerWorkshopView(context);

  void updateAuthorkitProjectContext();
  void updateConnectionStatus();
  const interval = setInterval(() => {
    void updateConnectionStatus();
  }, 30_000);
  context.subscriptions.push(new vscode.Disposable(() => clearInterval(interval)));
}

export function deactivate(): void {}
