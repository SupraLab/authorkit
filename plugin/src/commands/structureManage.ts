import * as crypto from 'crypto';
import * as vscode from 'vscode';
import * as api from '../api/client';
import type { StructureData } from '../api/client';
import { getApiBaseUrl, requireWorkspaceRoot } from '../config';
import type { OutlineProvider, OutlineTreeItem } from '../providers/outlineProvider';
import { deleteSceneMarkdownFile } from '../scenes';
import { isValidUuid } from '../uuidUtil';

type SceneNode = { name: string; uuid: string };
type ChapterNode = {
  name: string;
  summary?: string;
  has_summary?: boolean;
  uuid: string;
  scenes: SceneNode[];
};
type ActNode = {
  name: string;
  summary?: string;
  has_summary?: boolean;
  uuid: string;
  chapters: ChapterNode[];
};

function newUuid(): string {
  return crypto.randomUUID();
}

function newScene(name: string): SceneNode {
  return { name, uuid: newUuid() };
}

function newChapter(name: string): ChapterNode {
  return {
    name,
    summary: '',
    has_summary: false,
    uuid: newUuid(),
    scenes: [],
  };
}

function newAct(name: string): ActNode {
  return {
    name,
    summary: '',
    has_summary: false,
    uuid: newUuid(),
    chapters: [],
  };
}

type Node3 = Extract<OutlineTreeItem, { kind: 'act' | 'chapter' | 'scene' }>;

async function loadStructureFull(root: string, baseUrl: string): Promise<StructureData> {
  return (await api.getStructure(baseUrl, root)) as StructureData;
}

async function saveStructureFull(
  root: string,
  baseUrl: string,
  structure: StructureData
): Promise<void> {
  await api.putStructure(baseUrl, root, structure);
}

function getActs(structure: StructureData): ActNode[] {
  const acts = structure.acts as ActNode[] | undefined;
  return acts ?? [];
}

export async function addAct(
  _item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  void _item;
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = await loadStructureFull(root, baseUrl);
  const acts = getActs(structure);
  const name =
    (await vscode.window.showInputBox({
      title: 'New act',
      value: `Act ${acts.length + 1}`,
      prompt: 'Title for the new act',
      validateInput: (v) => (v?.trim() ? undefined : 'Enter a title'),
    }))?.trim() ?? '';
  if (!name) {
    return;
  }
  acts.push(newAct(name));
  structure.acts = acts;
  await saveStructureFull(root, baseUrl, structure);
  outlineProvider.refresh();
  void vscode.window.showInformationMessage(`Added act “${name}”.`);
}

export async function addChapter(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  const el = asNode3(item);
  if (!el || (el.kind !== 'act' && el.kind !== 'chapter')) {
    void vscode.window.showInformationMessage('Select an act or chapter in Book Structure first.');
    return;
  }
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = await loadStructureFull(root, baseUrl);
  const acts = getActs(structure);
  const act = acts[el.actIndex];
  if (!act) {
    void vscode.window.showErrorMessage('Act not found.');
    return;
  }
  if (!act.chapters) {
    act.chapters = [];
  }
  const ch =
    (await vscode.window.showInputBox({
      title: 'New chapter',
      value: `Chapter ${act.chapters.length + 1}`,
      prompt: 'Title for the new chapter',
      validateInput: (v) => (v?.trim() ? undefined : 'Enter a title'),
    }))?.trim() ?? '';
  if (!ch) {
    return;
  }
  const chapter = newChapter(ch);
  if (el.kind === 'act') {
    act.chapters.push(chapter);
  } else {
    act.chapters.splice(el.chapterIndex + 1, 0, chapter);
  }
  structure.acts = acts;
  await saveStructureFull(root, baseUrl, structure);
  outlineProvider.refresh();
  void vscode.window.showInformationMessage(`Added chapter “${ch}”.`);
}

export async function addScene(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  const el = asNode3(item);
  if (!el || (el.kind !== 'chapter' && el.kind !== 'scene')) {
    void vscode.window.showInformationMessage('Select a chapter or scene in Book Structure first.');
    return;
  }
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = await loadStructureFull(root, baseUrl);
  const acts = getActs(structure);
  const act = acts[el.actIndex];
  const chapter = act?.chapters?.[el.chapterIndex];
  if (!act || !chapter) {
    void vscode.window.showErrorMessage('Chapter not found.');
    return;
  }
  if (!chapter.scenes) {
    chapter.scenes = [];
  }
  const sn =
    (await vscode.window.showInputBox({
      title: 'New scene',
      value: `Scene ${chapter.scenes.length + 1}`,
      prompt: 'Title for the new scene',
      validateInput: (v) => (v?.trim() ? undefined : 'Enter a title'),
    }))?.trim() ?? '';
  if (!sn) {
    return;
  }
  const scene = newScene(sn);
  if (el.kind === 'chapter') {
    chapter.scenes.push(scene);
  } else {
    chapter.scenes.splice(el.sceneIndex + 1, 0, scene);
  }
  structure.acts = acts;
  await saveStructureFull(root, baseUrl, structure);
  outlineProvider.refresh();
  void vscode.window.showInformationMessage(`Added scene “${sn}”.`);
}

export async function deleteStructureNode(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  const el = asNode3(item);
  if (!el) {
    void vscode.window.showInformationMessage('Select an act, chapter, or scene in Book Structure first.');
    return;
  }
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = await loadStructureFull(root, baseUrl);
  const acts = getActs(structure);

  const label =
    el.kind === 'act'
      ? acts[el.actIndex]?.name || 'this act'
      : el.kind === 'chapter'
        ? acts[el.actIndex]?.chapters?.[el.chapterIndex]?.name || 'this chapter'
        : acts[el.actIndex]?.chapters?.[el.chapterIndex]?.scenes?.[el.sceneIndex]?.name ||
          'this scene';

  const extra =
    el.kind === 'scene'
      ? ' The scene Markdown file will be deleted if it exists.'
      : el.kind === 'chapter' || el.kind === 'act'
        ? ' Scene files under this part of the tree will be deleted from disk when possible.'
        : '';
  const pick = await vscode.window.showWarningMessage(
    `Remove “${label}” from the book structure?${extra}`,
    { modal: true },
    'Remove'
  );
  if (pick !== 'Remove') {
    return;
  }

  if (el.kind === 'act') {
    if (acts.length <= 1) {
      void vscode.window.showErrorMessage('Cannot remove the last act.');
      return;
    }
    const removed = acts[el.actIndex];
    if (removed?.chapters) {
      for (const ch of removed.chapters) {
        await deleteSceneFilesForChapter(root, ch);
      }
    }
    acts.splice(el.actIndex, 1);
  } else if (el.kind === 'chapter') {
    const act = acts[el.actIndex];
    if (!act?.chapters) {
      return;
    }
    const removed = act.chapters[el.chapterIndex];
    if (removed) {
      await deleteSceneFilesForChapter(root, removed);
    }
    act.chapters.splice(el.chapterIndex, 1);
  } else {
    const act = acts[el.actIndex];
    const ch = act?.chapters?.[el.chapterIndex];
    const sc = ch?.scenes?.[el.sceneIndex];
    const uid = sc?.uuid;
    if (ch?.scenes) {
      ch.scenes.splice(el.sceneIndex, 1);
    }
    if (uid && isValidUuid(uid)) {
      await deleteSceneMarkdownFile(root, uid);
    }
  }

  structure.acts = acts;
  await saveStructureFull(root, baseUrl, structure);
  outlineProvider.refresh();
  void vscode.window.showInformationMessage('Structure updated.');
}

export async function moveStructureUp(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  await moveStructure(item, outlineProvider, 'up');
}

export async function moveStructureDown(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider
): Promise<void> {
  await moveStructure(item, outlineProvider, 'down');
}

async function moveStructure(
  item: OutlineTreeItem | undefined,
  outlineProvider: OutlineProvider,
  dir: 'up' | 'down'
): Promise<void> {
  const el = asNode3(item);
  if (!el) {
    void vscode.window.showInformationMessage('Select an act, chapter, or scene in Book Structure first.');
    return;
  }
  const root = requireWorkspaceRoot();
  const baseUrl = getApiBaseUrl();
  const structure = await loadStructureFull(root, baseUrl);
  const acts = getActs(structure);
  const delta = dir === 'up' ? -1 : 1;

  if (el.kind === 'act') {
    const j = el.actIndex + delta;
    if (j < 0 || j >= acts.length) {
      void vscode.window.showInformationMessage(
        dir === 'up' ? 'Already at the top.' : 'Already at the bottom.'
      );
      return;
    }
    const t = acts[el.actIndex];
    acts[el.actIndex] = acts[j];
    acts[j] = t;
  } else if (el.kind === 'chapter') {
    const act = acts[el.actIndex];
    const chs = act?.chapters;
    if (!chs?.length) {
      return;
    }
    const j = el.chapterIndex + delta;
    if (j < 0 || j >= chs.length) {
      void vscode.window.showInformationMessage(
        dir === 'up' ? 'Already at the top.' : 'Already at the bottom.'
      );
      return;
    }
    const t = chs[el.chapterIndex];
    chs[el.chapterIndex] = chs[j];
    chs[j] = t;
  } else {
    const ch = acts[el.actIndex]?.chapters?.[el.chapterIndex];
    const scs = ch?.scenes;
    if (!scs?.length) {
      return;
    }
    const j = el.sceneIndex + delta;
    if (j < 0 || j >= scs.length) {
      void vscode.window.showInformationMessage(
        dir === 'up' ? 'Already at the top.' : 'Already at the bottom.'
      );
      return;
    }
    const t = scs[el.sceneIndex];
    scs[el.sceneIndex] = scs[j];
    scs[j] = t;
  }

  structure.acts = acts;
  await saveStructureFull(root, baseUrl, structure);
  outlineProvider.refresh();
}

async function deleteSceneFilesForChapter(root: string, chapter: ChapterNode): Promise<void> {
  for (const sc of chapter.scenes || []) {
    const uid = sc?.uuid;
    if (uid && isValidUuid(uid)) {
      await deleteSceneMarkdownFile(root, uid);
    }
  }
}

function asNode3(item: OutlineTreeItem | undefined): Node3 | undefined {
  if (!item || item.kind === 'loading' || item.kind === 'empty' || item.kind === 'error') {
    return undefined;
  }
  return item as Node3;
}
