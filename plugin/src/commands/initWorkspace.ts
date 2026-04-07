import * as fs from 'fs/promises';
import * as path from 'path';
import * as vscode from 'vscode';
import * as api from '../api/client';
import type { StructureData } from '../api/client';
import { getApiBaseUrl, getCharactersCategoryName, getWorldCategoryName, requireWorkspaceRoot } from '../config';
import { REL_AUTHORKIT_STRUCTURE, updateAuthorkitProjectContext } from '../workspaceContext';

const COMPENDIUM_REL = path.join('.authorkit', 'compendium.json');

/**
 * Create `.authorkit/` on disk via the API: default outline + empty compendium categories.
 * If a project already exists, asks before overwriting outline/compendium files.
 */
export async function initWorkspace(refreshTrees: () => void): Promise<void> {
  const root = requireWorkspaceRoot();
  const structurePath = path.join(root, REL_AUTHORKIT_STRUCTURE);
  const compendiumPath = path.join(root, COMPENDIUM_REL);

  let hasStructure = false;
  try {
    await fs.access(structurePath);
    hasStructure = true;
  } catch {
    /* absent */
  }

  if (hasStructure) {
    const pick = await vscode.window.showWarningMessage(
      'This folder already has an AuthorKit project (.authorkit/structure.json). Replace the outline and compendium with fresh defaults?',
      { modal: true },
      'Replace',
      'Cancel'
    );
    if (pick !== 'Replace') {
      return;
    }
    try {
      await fs.unlink(structurePath);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      void vscode.window.showErrorMessage(`Could not remove structure file: ${msg}`);
      return;
    }
    try {
      await fs.unlink(compendiumPath);
    } catch {
      /* optional file */
    }
  }

  const baseUrl = getApiBaseUrl();
  const chars = getCharactersCategoryName();
  const world = getWorldCategoryName();

  try {
    const structure = (await api.getStructure(baseUrl, root)) as StructureData;
    await api.putStructure(baseUrl, root, structure);
    await api.putCompendium(baseUrl, root, {
      categories: [
        { name: chars, entries: [] },
        { name: world, entries: [] },
      ],
    });
    void vscode.window.showInformationMessage(
      'AuthorKit project files are ready in this folder (.authorkit).'
    );
    refreshTrees();
    await updateAuthorkitProjectContext();
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(
      `Initialize failed (is the API running at ${baseUrl}?): ${msg}`
    );
  }
}
