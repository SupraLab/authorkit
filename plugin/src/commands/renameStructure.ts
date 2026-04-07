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
    void vscode.window.showInformationMessage('Select an act, chapter, or scene in Book Structure first.');
    return;
  }

  type Node = Extract<OutlineTreeItem, { kind: 'act' | 'chapter' | 'scene' }>;
  const el = item as Node;

  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = (await api.getStructure(baseUrl, root)) as StructureData;
  const acts = structure.acts as ActJson[] | undefined;
  if (!acts?.length) {
    void vscode.window.showErrorMessage('No structure loaded.');
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

  const kindLabel = el.kind === 'act' ? 'act' : el.kind === 'chapter' ? 'chapter' : 'scene';
  const next = await vscode.window.showInputBox({
    title: `Rename ${kindLabel}`,
    value: currentTitle,
    prompt: `New title for this ${kindLabel}`,
    validateInput: (v) => (v?.trim() ? undefined : 'Enter a title'),
  });
  if (next === undefined) {
    return;
  }
  const trimmed = next.trim();

  const act = acts[el.actIndex];
  if (!act) {
    void vscode.window.showErrorMessage('Act not found in structure.');
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
      void vscode.window.showErrorMessage('Chapter not found in structure.');
      return;
    }
    ch.name = trimmed;
  } else {
    const ch = act.chapters?.[el.chapterIndex];
    if (!ch?.scenes) {
      void vscode.window.showErrorMessage('Scene not found in structure.');
      return;
    }
    const sc = ch.scenes[el.sceneIndex];
    if (!sc) {
      void vscode.window.showErrorMessage('Scene not found in structure.');
      return;
    }
    sc.name = trimmed;
  }

  try {
    await api.putStructure(baseUrl, root, structure);
    outlineProvider.refresh();
    void vscode.window.showInformationMessage(`Renamed to “${trimmed}”.`);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(`Could not save structure: ${msg}`);
  }
}
