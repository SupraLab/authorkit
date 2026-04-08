import * as vscode from 'vscode';
import * as api from '../api/client';
import type { StructureData } from '../api/client';
import { getApiBaseUrl, requireWorkspaceRoot } from '../config';
import type { OutlineProvider, OutlineTreeItem } from '../providers/outlineProvider';

type ActJson = {
  name?: string;
  chapters?: Array<{
    name?: string;
    scenes?: Array<{ name?: string; uuid?: string }>;
  }>;
};

export async function renameStructureNode(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  if (!item || item.kind === 'loading' || item.kind === 'empty' || item.kind === 'error') {
    void vscode.window.showInformationMessage(
      vscode.l10n.t('Select an act, chapter, or scene in Book Structure first.')
    );
    return;
  }

  type Node = Extract<OutlineTreeItem, { kind: 'act' | 'chapter' | 'scene' }>;
  const el = item as Node;

  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = (await api.getStructure(baseUrl, root)) as StructureData;
  const acts = structure.acts as ActJson[] | undefined;
  if (!acts?.length) {
    void vscode.window.showErrorMessage(vscode.l10n.t('No structure loaded.'));
    return;
  }

  let currentTitle = '';
  if (el.kind === 'act') {
    currentTitle = acts[el.actIndex]?.name || '';
  } else if (el.kind === 'chapter') {
    currentTitle = acts[el.actIndex]?.chapters?.[el.chapterIndex]?.name || '';
  } else {
    currentTitle =
      acts[el.actIndex]?.chapters?.[el.chapterIndex]?.scenes?.[el.sceneIndex]?.name || '';
  }

  const titleRename =
    el.kind === 'act'
      ? vscode.l10n.t('Rename act')
      : el.kind === 'chapter'
        ? vscode.l10n.t('Rename chapter')
        : vscode.l10n.t('Rename scene');
  const promptRename =
    el.kind === 'act'
      ? vscode.l10n.t('New title for this act')
      : el.kind === 'chapter'
        ? vscode.l10n.t('New title for this chapter')
        : vscode.l10n.t('New title for this scene');
  const next = await vscode.window.showInputBox({
    title: titleRename,
    value: currentTitle,
    prompt: promptRename,
    validateInput: (v) => (v?.trim() ? undefined : vscode.l10n.t('Enter a title')),
  });
  if (next === undefined) {
    return;
  }
  const trimmed = next.trim();

  const act = acts[el.actIndex];
  if (!act) {
    void vscode.window.showErrorMessage(vscode.l10n.t('Act not found in structure.'));
    return;
  }

  if (el.kind === 'act') {
    act.name = trimmed;
  } else if (el.kind === 'chapter') {
    if (!act.chapters) {
      act.chapters = [];
    }
    const ch = act.chapters[el.chapterIndex];
    if (!ch) {
      void vscode.window.showErrorMessage(vscode.l10n.t('Chapter not found in structure.'));
      return;
    }
    ch.name = trimmed;
  } else {
    const ch = act.chapters?.[el.chapterIndex];
    if (!ch?.scenes) {
      void vscode.window.showErrorMessage(vscode.l10n.t('Scene not found in structure.'));
      return;
    }
    const sc = ch.scenes[el.sceneIndex];
    if (!sc) {
      void vscode.window.showErrorMessage(vscode.l10n.t('Scene not found in structure.'));
      return;
    }
    sc.name = trimmed;
  }

  try {
    await api.putStructure(baseUrl, root, structure);
    outlineProvider.refresh();
    void vscode.window.showInformationMessage(vscode.l10n.t('Renamed to \u201c{0}\u201d.', trimmed));
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(vscode.l10n.t('Could not save structure: {0}', msg));
  }
}
