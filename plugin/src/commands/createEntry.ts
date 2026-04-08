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

/** Localized category label for prompts when the stored name is the English default. */
function compendiumCategoryDisplayForPrompt(stored: string): string {
  const ch = getCharactersCategoryName();
  const wo = getWorldCategoryName();
  if (stored === ch) {
    return stored === 'Characters' ? vscode.l10n.t('Characters') : stored;
  }
  if (stored === wo) {
    return stored === 'World' ? vscode.l10n.t('World') : stored;
  }
  return stored;
}

export async function createCompendiumEntry(
  context: vscode.ExtensionContext,
  categoryName: string
): Promise<void> {
  const name = await vscode.window.showInputBox({
    prompt: vscode.l10n.t('New {0} entry name', compendiumCategoryDisplayForPrompt(categoryName)),
    validateInput: (v) => (v?.trim() ? undefined : vscode.l10n.t('Enter a name')),
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
    void vscode.window.showWarningMessage(
      vscode.l10n.t('An entry named \u201c{0}\u201d already exists.', trimmed)
    );
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
    const sheetRel = `.authorkit/${slugHint(categoryName)}/${id}.md`;
    const openWs = vscode.l10n.t('Open Workshop');
    const notNow = vscode.l10n.t('Not now');
    const pick = await vscode.window.showInformationMessage(
      vscode.l10n.t(
        'Created \u201c{0}\u201d (sheet: {1}). Open Workshop with this entry as context?',
        trimmed,
        sheetRel
      ),
      openWs,
      notNow
    );
    if (pick === openWs) {
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
      vscode.l10n.t(
        'Created \u201c{0}\u201d in {1} (Markdown: {2}).',
        trimmed,
        categoryName,
        `.authorkit/${slugHint(categoryName)}/${id}.md`
      )
    );
  }
}

function slugHint(categoryName: string): string {
  return categoryName.replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'entry';
}
