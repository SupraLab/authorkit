import * as crypto from 'crypto';
import * as fs from 'fs/promises';
import * as path from 'path';
import * as vscode from 'vscode';
import * as api from '../api/client';
import type { CompendiumData } from '../api/client';
import { openChatWithAuthorKitPrompt, type WorkshopApplyContext } from '../chatWorkflow';
import { compendiumEntryMarkdownPath } from '../compendiumPaths';
import {
  getApiBaseUrl,
  getCharactersCategoryName,
  getWorldCategoryName,
  requireWorkspaceRoot,
} from '../config';

export async function createCompendiumEntry(
  context: vscode.ExtensionContext,
  categoryName: string
): Promise<void> {
  const name = await vscode.window.showInputBox({
    prompt: `New ${categoryName} entry name`,
    validateInput: (v) => (v?.trim() ? undefined : 'Enter a name'),
  });
  if (!name?.trim()) {
    return;
  }
  const trimmed = name.trim();
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const data = (await api.getCompendium(baseUrl, root)) as CompendiumData;
  if (!data.categories) {
    data.categories = [];
  }
  let cat = data.categories.find((c) => c.name === categoryName);
  if (!cat) {
    cat = { name: categoryName, entries: [] };
    data.categories.push(cat);
  }
  if (cat.entries.some((e) => e.name === trimmed)) {
    void vscode.window.showWarningMessage(`An entry named “${trimmed}” already exists.`);
    return;
  }

  const id = crypto.randomUUID();
  const mdPath = compendiumEntryMarkdownPath(root, categoryName, id);
  await fs.mkdir(path.dirname(mdPath), { recursive: true });
  const initialMd = `# ${trimmed}\n\n`;
  await fs.writeFile(mdPath, initialMd, 'utf-8');

  cat.entries.push({ name: trimmed, content: '', id });
  await api.putCompendium(baseUrl, root, data);

  const chars = getCharactersCategoryName();
  const world = getWorldCategoryName();
  if (categoryName === chars || categoryName === world) {
    const pick = await vscode.window.showInformationMessage(
      `Created “${trimmed}” (sheet: .authorkit/${slugHint(categoryName)}/${id}.md). Open Workshop with this entry as context?`,
      'Open Workshop',
      'Not now'
    );
    if (pick === 'Open Workshop') {
      const sheetRel = path
        .relative(root, compendiumEntryMarkdownPath(root, categoryName, id))
        .replace(/\\/g, '/');
      const apply: WorkshopApplyContext =
        categoryName === chars
          ? { mode: 'character', entryName: trimmed, sheetRel }
          : { mode: 'world', entryName: trimmed, sheetRel };
      await openChatWithAuthorKitPrompt(context, '', apply);
    }
  } else {
    void vscode.window.showInformationMessage(
      `Created “${trimmed}” in ${categoryName} (Markdown: .authorkit/${slugHint(categoryName)}/${id}.md).`
    );
  }
}

function slugHint(categoryName: string): string {
  return categoryName.replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'entry';
}
