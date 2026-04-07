import * as path from 'path';
import * as vscode from 'vscode';
import { compendiumEntryMarkdownPath } from '../compendiumPaths';
import {
  getCharactersCategoryName,
  getWorldCategoryName,
  getWorkspaceRoot,
} from '../config';
import type { EntryItem } from '../providers/compendiumProvider';
import { openChatWithAuthorKitPrompt, type WorkshopApplyContext } from '../chatWorkflow';

function sheetProjectRelativePath(
  categoryName: string,
  entryId: string | undefined,
  workspaceRoot: string
): string | undefined {
  if (!entryId?.trim()) {
    return undefined;
  }
  const abs = compendiumEntryMarkdownPath(workspaceRoot, categoryName, entryId);
  return path.relative(workspaceRoot, abs).replace(/\\/g, '/');
}

/**
 * Context menu on Characters / World: focus Workshop and set Scene/Character/World context.
 */
export async function openCompendiumChatFromTreeItem(
  context: vscode.ExtensionContext,
  item?: EntryItem
): Promise<void> {
  if (!item || item.kind !== 'entry') {
    return;
  }
  const root = getWorkspaceRoot();
  if (!root) {
    void vscode.window.showWarningMessage('Open a folder workspace to use the Workshop.');
    return;
  }
  const chars = getCharactersCategoryName();
  const world = getWorldCategoryName();
  const rel = sheetProjectRelativePath(item.categoryName, item.entryId, root);

  let apply: WorkshopApplyContext;
  if (item.categoryName === chars) {
    apply = { mode: 'character', entryName: item.entryName, sheetRel: rel };
  } else if (item.categoryName === world) {
    apply = { mode: 'world', entryName: item.entryName, sheetRel: rel };
  } else {
    void vscode.window.showInformationMessage(
      'Workshop context is only wired for Characters and World entries.'
    );
    return;
  }

  await openChatWithAuthorKitPrompt(context, '', apply);
}
