import * as vscode from 'vscode';
import * as api from '../api/client';
import {
  getApiBaseUrl,
  getCharactersCategoryName,
  getWorldCategoryName,
  getWorkshopLlmOptions,
  getWorkspaceRoot,
} from '../config';
import { loadWorkshopBootstrap } from '../workshopBootstrap';
import { insertWorkshopReplyIntoCompendium, insertWorkshopReplyIntoScene } from '../workshopInsert';

export const WORKSHOP_VIEW_ID = 'authorkit.workshopView';
const FOCUS_WORKSHOP_CMD = 'workbench.view.extension.authorkit-workshop';

let workshopView: vscode.WebviewView | undefined;
let streamAbort: AbortController | undefined;

/** Queued until the webview is ready (focus + first resolve). */
let pendingOpen: { initialPrompt?: string; applyContext?: WorkshopApplyContext } | undefined;

export interface WorkshopApplyContext {
  mode: 'none' | 'scene' | 'character' | 'world' | 'selection';
  sceneUuid?: string;
  sceneLabel?: string;
  entryName?: string;
  sheetRel?: string;
  /** When mode === 'selection', snippets from the active editor (command / context menu). */
  selectionItems?: Array<{
    text: string;
    attachment: api.SelectionAttachment;
    detailTitle?: string;
  }>;
}

export interface OpenWorkshopOptions {
  initialPrompt?: string;
  applyContext?: WorkshopApplyContext;
}

/** Multi-select context sent to the API as scene_uuids + compendium_excerpts. */
export type WorkshopContextChip =
  | { kind: 'scene'; uuid: string; label: string }
  | { kind: 'character'; name: string }
  | { kind: 'world'; name: string }
  | {
      kind: 'selection';
      text: string;
      attachment: api.SelectionAttachment;
      detailTitle?: string;
    };

function threadStateKey(workspaceRoot: string): string {
  return `authorkit.workshop.activeThread.${workspaceRoot}`;
}

function getNonce(): string {
  let t = '';
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    t += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return t;
}

/** One tag segment for the composer chip strip (deduped). */
function contextChipTagLine(c: WorkshopContextChip): string {
  if (c.kind === 'scene') {
    const lab = c.label?.trim() || c.uuid.slice(0, 8);
    return vscode.l10n.t('scene: {0}', lab);
  }
  if (c.kind === 'character') {
    return vscode.l10n.t('character: {0}', c.name);
  }
  if (c.kind === 'world') {
    return vscode.l10n.t('world: {0}', c.name);
  }
  const a = c.attachment;
  const rng = a.range_label?.trim() ? ` · ${a.range_label.trim()}` : '';
  if (a.kind === 'scene') {
    return vscode.l10n.t('scene: {0}', a.label) + rng;
  }
  if (a.kind === 'compendium') {
    const charsCat = getCharactersCategoryName();
    const worldCat = getWorldCategoryName();
    const cat = a.compendium_category || '';
    const nm = a.compendium_name || '';
    if (cat === charsCat) {
      return vscode.l10n.t('character: {0}', nm) + rng;
    }
    if (cat === worldCat) {
      return vscode.l10n.t('world: {0}', nm) + rng;
    }
    return vscode.l10n.t('compendium: {0} — {1}', cat, nm) + rng;
  }
  return vscode.l10n.t('selection: {0}', a.label) + rng;
}

/** Short line for the chat bubble next to "You", e.g. "scene: Beat 1 · character: Ethan". */
function chipTagLineForUi(chips: WorkshopContextChip[]): string {
  if (!chips.length) {
    return '';
  }
  const seen = new Set<string>();
  const parts: string[] = [];
  for (const c of chips) {
    const line = contextChipTagLine(c);
    if (seen.has(line)) {
      continue;
    }
    seen.add(line);
    parts.push(line);
  }
  return parts.join(' · ');
}

function buildWorkshopWebviewStrings(): Record<string, string> {
  return {
    thread: vscode.l10n.t('Thread'),
    ariaActiveThread: vscode.l10n.t('Active thread'),
    newThread: vscode.l10n.t('New thread'),
    renameThread: vscode.l10n.t('Rename thread'),
    deleteThread: vscode.l10n.t('Delete thread'),
    context: vscode.l10n.t('Context'),
    addScene: vscode.l10n.t('Add scene'),
    addCharacter: vscode.l10n.t('Add character'),
    addWorldEntry: vscode.l10n.t('Add world entry'),
    reloadLists: vscode.l10n.t('Reload lists'),
    contextTagsAria: vscode.l10n.t('Context tags'),
    messagePlaceholder: vscode.l10n.t('Message… (⌃/⌘+Enter)'),
    sendTitle: vscode.l10n.t('Send'),
    sendAria: vscode.l10n.t('Send'),
    stopTitle: vscode.l10n.t('Stop'),
    stopAria: vscode.l10n.t('Stop'),
    noScenesInStructure: vscode.l10n.t('No scenes in structure'),
    noCharacters: vscode.l10n.t('No characters'),
    noWorldEntries: vscode.l10n.t('No world entries'),
    removeChipAria: vscode.l10n.t('Remove'),
    you: vscode.l10n.t('You'),
    workshop: vscode.l10n.t('Workshop'),
    error: vscode.l10n.t('Error'),
    insertScenePrefix: vscode.l10n.t('Insert into scene: '),
    insertIntoPrefix: vscode.l10n.t('Insert into: '),
    contextErrorPrefix: vscode.l10n.t('Context: '),
    sceneTag: vscode.l10n.t('scene: '),
    charTag: vscode.l10n.t('character: '),
    worldTag: vscode.l10n.t('world: '),
    compendiumPrefix: vscode.l10n.t('compendium: '),
    selectionTag: vscode.l10n.t('selection: '),
    /** Display label when category matches workspace Characters (data may store English default). */
    categoryCharacters: vscode.l10n.t('Characters'),
    categoryWorld: vscode.l10n.t('World'),
  };
}

function escapeForHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
}

function mapChipsToStreamOptions(
  chips: WorkshopContextChip[]
): Pick<
  api.WorkshopStreamOptions,
  | 'sceneUuids'
  | 'compendiumExcerpts'
  | 'extraContext'
  | 'selectionLabels'
  | 'selectionAttachments'
> {
  const sceneUuids: string[] = [];
  const compendiumExcerpts: api.CompendiumExcerptRef[] = [];
  const selectionBlocks: string[] = [];
  const attachmentDedup: api.SelectionAttachment[] = [];
  const seenAtt = new Set<string>();
  const pushAtt = (a: api.SelectionAttachment): void => {
    const r = a.range_label ?? '';
    const k =
      a.kind === 'scene' && a.scene_uuid
        ? `s:${a.scene_uuid}:${r}`
        : a.kind === 'compendium' && a.compendium_category && a.compendium_name
          ? `c:${a.compendium_category}:${a.compendium_name}:${r}`
          : `f:${a.label}:${r}`;
    if (seenAtt.has(k)) {
      return;
    }
    seenAtt.add(k);
    attachmentDedup.push(a);
  };

  const charsCat = getCharactersCategoryName();
  const worldCat = getWorldCategoryName();
  for (const c of chips) {
    if (c.kind === 'scene') {
      sceneUuids.push(c.uuid);
    } else if (c.kind === 'character') {
      compendiumExcerpts.push({ category: charsCat, name: c.name });
    } else if (c.kind === 'world') {
      compendiumExcerpts.push({ category: worldCat, name: c.name });
    } else if (c.kind === 'selection') {
      const head = c.attachment.range_label?.trim()
        ? `### ${c.attachment.label} (${c.attachment.range_label.trim()})\n\n`
        : `### ${c.attachment.label}\n\n`;
      selectionBlocks.push(head + c.text);
      pushAtt(c.attachment);
    }
  }
  const extraContext =
    selectionBlocks.length > 0 ? selectionBlocks.join('\n\n---\n\n') : undefined;
  const selectionLabels = attachmentDedup.length
    ? attachmentDedup.map((a) =>
        a.range_label?.trim() ? `${a.label} · ${a.range_label.trim()}` : a.label
      )
    : undefined;
  return {
    sceneUuids: sceneUuids.length ? sceneUuids : undefined,
    compendiumExcerpts: compendiumExcerpts.length ? compendiumExcerpts : undefined,
    extraContext,
    selectionLabels,
    selectionAttachments: attachmentDedup.length ? attachmentDedup : undefined,
  };
}

function flushPendingToView(): void {
  if (!workshopView || !pendingOpen) {
    return;
  }
  const w = workshopView.webview;
  const p = pendingOpen;
  pendingOpen = undefined;
  if (p.initialPrompt !== undefined && p.initialPrompt !== '') {
    void w.postMessage({ type: 'setInput', text: p.initialPrompt });
  }
  if (p.applyContext) {
    void w.postMessage({ type: 'applyContext', ...p.applyContext });
  }
}

/** Reload threads + messages into the webview (shared by panel open + view provider). */
async function pushWorkshopStateToWebview(
  context: vscode.ExtensionContext,
  webview: vscode.Webview
): Promise<void> {
  const root = getWorkspaceRoot();
  if (!root) {
    void webview.postMessage({
      type: 'workshopState',
      threads: [],
      activeThreadId: '',
      messages: [],
    });
    return;
  }
  const baseUrl = getApiBaseUrl();
  const key = threadStateKey(root);
  let threads = await api.listWorkshopThreads(baseUrl, root);
  if (threads.length === 0) {
    const firstTitle = await vscode.window.showInputBox({
      title: vscode.l10n.t('New thread'),
      prompt: vscode.l10n.t('Name your first conversation.'),
      placeHolder: vscode.l10n.t('e.g. Main workshop'),
      validateInput: (s) => {
        if (!s?.trim()) {
          return vscode.l10n.t('Name cannot be empty.');
        }
        return undefined;
      },
    });
    if (firstTitle === undefined) {
      void webview.postMessage({
        type: 'workshopState',
        threads: [],
        activeThreadId: '',
        messages: [],
      });
      return;
    }
    const t = await api.createWorkshopThread(baseUrl, root, { title: firstTitle.trim() });
    await context.workspaceState.update(key, t.thread_id);
    threads = await api.listWorkshopThreads(baseUrl, root);
  }
  let active = context.workspaceState.get<string>(key);
  if (!active || !threads.some((x) => x.thread_id === active)) {
    active = threads[0].thread_id;
    await context.workspaceState.update(key, active);
  }
  const messages = await api.getWorkshopMessages(baseUrl, root, active);
  void webview.postMessage({
    type: 'workshopState',
    threads,
    activeThreadId: active,
    messages,
  });
}

async function pushBootstrapToWebview(webview: vscode.Webview): Promise<void> {
  const root = getWorkspaceRoot();
  if (!root) {
    void webview.postMessage({
      type: 'bootstrap',
      payload: {
        scenes: [],
        characters: [],
        world: [],
        charactersCategory: getCharactersCategoryName(),
        worldCategory: getWorldCategoryName(),
      },
      error: vscode.l10n.t('Open a folder workspace to load project context.'),
    });
    return;
  }
  try {
    const payload = await loadWorkshopBootstrap(getApiBaseUrl(), root);
    void webview.postMessage({
      type: 'bootstrap',
      payload: {
        ...payload,
        charactersCategory: getCharactersCategoryName(),
        worldCategory: getWorldCategoryName(),
      },
    });
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void webview.postMessage({
      type: 'bootstrap',
      payload: {
        scenes: [],
        characters: [],
        world: [],
        charactersCategory: getCharactersCategoryName(),
        worldCategory: getWorldCategoryName(),
      },
      error: m,
    });
  }
}

type WorkshopFromWebview =
  | { type: 'send'; text: string; chips: WorkshopContextChip[] }
  | { type: 'stop' }
  | { type: 'refreshBootstrap' }
  | { type: 'selectThread'; threadId: string }
  | { type: 'newThread' }
  | { type: 'renameThread'; threadId: string }
  | { type: 'deleteThread'; threadId: string }
  | {
      type: 'workshopResponseAction';
      action: 'insertScene' | 'insertCompendium';
      assistantText: string;
      sceneUuid?: string;
      compendiumCategory?: string;
      compendiumName?: string;
    };

class WorkshopViewProvider implements vscode.WebviewViewProvider {
  constructor(private readonly context: vscode.ExtensionContext) {}

  resolveWebviewView(webviewView: vscode.WebviewView): void | Thenable<void> {
    workshopView = webviewView;
    const { webview } = webviewView;
    webview.options = { enableScripts: true };
    const nonce = getNonce();
    const docLang = (vscode.env.language || 'en').split(/[-_]/)[0] || 'en';
    webview.html = getWorkshopHtml(webview, nonce, buildWorkshopWebviewStrings(), docLang);

    webview.onDidReceiveMessage(
      (msg: WorkshopFromWebview) => {
        void this.onMessage(webview, msg);
      },
      undefined,
      this.context.subscriptions
    );

    webviewView.onDidDispose(
      () => {
        if (workshopView === webviewView) {
          workshopView = undefined;
        }
      },
      null,
      this.context.subscriptions
    );

    return pushBootstrapToWebview(webview)
      .then(() => pushWorkshopStateToWebview(this.context, webview))
      .then(() => flushPendingToView());
  }

  private async pushWorkshopState(webview: vscode.Webview): Promise<void> {
    await pushWorkshopStateToWebview(this.context, webview);
  }

  private async onMessage(webview: vscode.Webview, msg: WorkshopFromWebview): Promise<void> {
    if (msg.type === 'workshopResponseAction') {
      const text = typeof msg.assistantText === 'string' ? msg.assistantText : '';
      if (msg.action === 'insertScene' && msg.sceneUuid) {
        void insertWorkshopReplyIntoScene(msg.sceneUuid, text);
      } else if (msg.action === 'insertCompendium' && msg.compendiumCategory && msg.compendiumName) {
        void insertWorkshopReplyIntoCompendium(msg.compendiumCategory, msg.compendiumName, text);
      }
      return;
    }
    if (msg.type === 'refreshBootstrap') {
      await pushBootstrapToWebview(webview);
      await this.pushWorkshopState(webview);
      return;
    }
    if (msg.type === 'stop') {
      streamAbort?.abort();
      return;
    }
    if (msg.type === 'selectThread' && msg.threadId) {
      const root = getWorkspaceRoot();
      if (root) {
        await this.context.workspaceState.update(threadStateKey(root), msg.threadId);
      }
      await this.pushWorkshopState(webview);
      return;
    }
    if (msg.type === 'newThread') {
      const root = getWorkspaceRoot();
      if (!root) {
        return;
      }
      const title = await vscode.window.showInputBox({
        title: vscode.l10n.t('New thread'),
        prompt: vscode.l10n.t('Name this conversation.'),
        placeHolder: vscode.l10n.t('e.g. Character — Ethan'),
        validateInput: (s) => {
          if (!s?.trim()) {
            return vscode.l10n.t('Name cannot be empty.');
          }
          return undefined;
        },
      });
      if (title === undefined) {
        return;
      }
      const baseUrl = getApiBaseUrl();
      try {
        const t = await api.createWorkshopThread(baseUrl, root, { title: title.trim() });
        await this.context.workspaceState.update(threadStateKey(root), t.thread_id);
        await this.pushWorkshopState(webview);
      } catch (e) {
        const err = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(
          vscode.l10n.t('AuthorKit: could not create thread — {0}', err)
        );
      }
      return;
    }
    if (msg.type === 'renameThread' && msg.threadId) {
      const root = getWorkspaceRoot();
      if (!root) {
        return;
      }
      const baseUrl = getApiBaseUrl();
      const threads = await api.listWorkshopThreads(baseUrl, root);
      const current = threads.find((x) => x.thread_id === msg.threadId);
      const picked = await vscode.window.showInputBox({
        title: vscode.l10n.t('Rename thread'),
        prompt: vscode.l10n.t('Display name in the thread list.'),
        value: current?.title ?? '',
        validateInput: (s) => {
          if (!s?.trim()) {
            return vscode.l10n.t('Name cannot be empty.');
          }
          return undefined;
        },
      });
      if (picked === undefined) {
        return;
      }
      try {
        await api.renameWorkshopThread(baseUrl, root, msg.threadId, picked.trim());
        await this.pushWorkshopState(webview);
      } catch (e) {
        const err = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(
          vscode.l10n.t('AuthorKit: could not rename thread — {0}', err)
        );
      }
      return;
    }
    if (msg.type === 'deleteThread' && msg.threadId) {
      const root = getWorkspaceRoot();
      if (!root) {
        return;
      }
      const baseUrl = getApiBaseUrl();
      const threads = await api.listWorkshopThreads(baseUrl, root);
      const thread = threads.find((x) => x.thread_id === msg.threadId);
      const label = thread?.title?.trim() || msg.threadId.slice(0, 8) + '…';
      const delBtn = vscode.l10n.t('Delete');
      const confirm = await vscode.window.showWarningMessage(
        vscode.l10n.t('Delete thread \u201c{0}\u201d?', label),
        {
          modal: true,
          detail: vscode.l10n.t(
            'All messages in this thread will be removed. This cannot be undone.'
          ),
        },
        delBtn
      );
      if (confirm !== delBtn) {
        return;
      }
      try {
        await api.deleteWorkshopThread(baseUrl, root, msg.threadId);
      } catch (e) {
        const err = e instanceof Error ? e.message : String(e);
        void vscode.window.showErrorMessage(
          vscode.l10n.t('AuthorKit: could not delete thread — {0}', err)
        );
        return;
      }
      const key = threadStateKey(root);
      const cur = this.context.workspaceState.get<string>(key);
      if (cur === msg.threadId) {
        const remaining = await api.listWorkshopThreads(baseUrl, root);
        const next = remaining[0]?.thread_id ?? '';
        await this.context.workspaceState.update(key, next);
      }
      await this.pushWorkshopState(webview);
      return;
    }
    if (msg.type === 'send' && typeof msg.text === 'string') {
      const chips = Array.isArray(msg.chips) ? msg.chips : [];
      await this.runSend(webview, msg.text.trim(), chips);
    }
  }

  private async runSend(
    webview: vscode.Webview,
    raw: string,
    chips: WorkshopContextChip[]
  ): Promise<void> {
    if (!raw.trim()) {
      return;
    }
    const root = getWorkspaceRoot();
    if (!root) {
      void webview.postMessage({
        type: 'error',
        text: vscode.l10n.t('Open a folder workspace to use the workshop.'),
      });
      return;
    }
    const active = this.context.workspaceState.get<string>(threadStateKey(root));
    if (!active) {
      void webview.postMessage({
        type: 'error',
        text: vscode.l10n.t('No thread selected. Create a thread first.'),
      });
      return;
    }

    streamAbort?.abort();
    streamAbort = new AbortController();
    const signal = streamAbort.signal;
    const baseUrl = getApiBaseUrl();
    const llm: api.WorkshopStreamOptions = {
      ...getWorkshopLlmOptions(),
      threadId: active,
      userLanguage: vscode.env.language,
      ...mapChipsToStreamOptions(chips),
    };

    const tagLine = chipTagLineForUi(chips);
    void webview.postMessage({
      type: 'user',
      text: raw,
      ...(tagLine ? { tagLine } : {}),
    });
    void webview.postMessage({ type: 'assistantStart' });

    try {
      for await (const chunk of api.workshopChatStream(baseUrl, root, raw, signal, llm)) {
        if (signal.aborted) {
          break;
        }
        void webview.postMessage({ type: 'assistantChunk', text: chunk });
      }
      if (!signal.aborted) {
        void webview.postMessage({ type: 'assistantEnd' });
        await this.pushWorkshopState(webview);
      }
    } catch (e) {
      if (e instanceof Error && e.name === 'AbortError') {
        void webview.postMessage({ type: 'assistantEnd' });
        return;
      }
      const err = e instanceof Error ? e.message : String(e);
      void webview.postMessage({ type: 'error', text: err });
    }
  }
}

export function registerWorkshopView(context: vscode.ExtensionContext): void {
  const provider = new WorkshopViewProvider(context);
  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider(WORKSHOP_VIEW_ID, provider, {
      webviewOptions: { retainContextWhenHidden: true },
    })
  );
}

export function openWorkshopChatPanel(context: vscode.ExtensionContext, options?: OpenWorkshopOptions): void {
  pendingOpen = {
    initialPrompt: options?.initialPrompt?.trim(),
    applyContext: options?.applyContext,
  };
  void vscode.commands.executeCommand(FOCUS_WORKSHOP_CMD);
  setTimeout(() => {
    void (async () => {
      if (workshopView) {
        await pushBootstrapToWebview(workshopView.webview);
        await pushWorkshopStateToWebview(context, workshopView.webview);
        flushPendingToView();
      }
    })();
  }, 120);
}

function getWorkshopHtml(
  webview: vscode.Webview,
  nonce: string,
  str: Record<string, string>,
  documentLang: string
): string {
  const csp = [
    `default-src 'none'`,
    `style-src ${webview.cspSource} 'unsafe-inline'`,
    `script-src 'nonce-${nonce}'`,
  ].join('; ');
  const strJson = JSON.stringify(str).replace(/</g, '\\u003c');

  return `<!DOCTYPE html>
<html lang="${escapeForHtml(documentLang)}">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="Content-Security-Policy" content="${csp}" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    * { box-sizing: border-box; }
    :root {
      --wk-pad-x: 12px;
    }
    [hidden] { display: none !important; }
    html, body {
      height: 100%;
      width: 100%;
      margin: 0;
      padding: 0;
      overflow-x: hidden;
    }
    body {
      display: flex;
      flex-direction: column;
      font-family: var(--vscode-font-family);
      font-size: var(--vscode-font-size);
      color: var(--vscode-foreground);
      background: var(--vscode-editor-background);
      min-height: 100%;
    }
    /* Chat scroll — inner horizontal padding inside the scroll container */
    #thread {
      flex: 1;
      min-height: 0;
      overflow-y: auto;
      overflow-x: hidden;
      padding: 10px var(--wk-pad-x) 8px;
      display: flex;
      flex-direction: column;
      gap: 12px;
      align-items: stretch;
      scroll-behavior: smooth;
    }
    .bubble {
      font-size: var(--vscode-font-size);
      line-height: 1.55;
      white-space: pre-wrap;
      word-break: break-word;
      position: relative;
    }
    .bubble-meta {
      font-size: 0.62rem;
      font-weight: 600;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      opacity: 0.5;
      margin-bottom: 6px;
    }
    .bubble-body {
      padding: 10px 12px;
      border-radius: 12px;
    }
    .bubble.user {
      align-self: flex-end;
      max-width: min(92%, 28rem);
    }
    .bubble.user .bubble-body {
      background: var(--vscode-input-background);
      border: 1px solid var(--vscode-input-border, rgba(128,128,128,.45));
      border-bottom-right-radius: 4px;
      box-shadow: 0 1px 2px rgba(0,0,0,.12);
    }
    .bubble.assistant {
      align-self: stretch;
      width: 100%;
      max-width: 100%;
    }
    .bubble.assistant .bubble-body {
      background: var(--vscode-sideBar-background);
      border: 1px solid var(--vscode-widget-border, rgba(128,128,128,.35));
      border-bottom-left-radius: 4px;
    }
    .bubble.error .bubble-body {
      border-color: var(--vscode-inputValidation-errorBorder, #c00);
      color: var(--vscode-errorForeground);
    }
    .bubble-tags {
      font-weight: 500;
      letter-spacing: 0.02em;
      text-transform: none;
      opacity: 0.72;
      font-size: 0.58rem;
    }
    .bubble-actions {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin-top: 8px;
      padding-top: 8px;
      border-top: 1px solid var(--vscode-widget-border, rgba(128,128,128,.25));
    }
    .bubble-action {
      font-family: var(--vscode-font-family);
      font-size: 0.72rem;
      padding: 4px 8px;
      border-radius: 6px;
      border: 1px solid var(--vscode-button-secondaryBorder, var(--vscode-widget-border));
      background: var(--vscode-button-secondaryBackground);
      color: var(--vscode-button-secondaryForeground);
      cursor: pointer;
    }
    .bubble-action:hover {
      background: var(--vscode-toolbar-hoverBackground);
    }
    /* Thread strip — inner padding left/right matches chat + composer */
    .thread-strip {
      flex-shrink: 0;
      display: flex;
      align-items: center;
      gap: 6px;
      flex-wrap: wrap;
      padding: 8px var(--wk-pad-x);
      background: var(--vscode-sideBar-background);
      border-bottom: 1px solid var(--vscode-widget-border, rgba(128,128,128,.35));
    }
    .thread-strip label {
      font-size: 0.62rem;
      font-weight: 600;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      opacity: 0.55;
    }
    #threadSelect {
      flex: 1;
      min-width: 100px;
      font-size: var(--vscode-font-size);
      font-family: var(--vscode-font-family);
      line-height: 1.35;
      appearance: none;
      -webkit-appearance: none;
      -moz-appearance: none;
      background-color: var(--vscode-dropdown-background);
      color: var(--vscode-dropdown-foreground);
      border: 1px solid var(--vscode-dropdown-border, var(--vscode-widget-border));
      border-radius: 4px;
      padding: 4px 8px;
      cursor: pointer;
    }
    #threadSelect:focus {
      outline: 1px solid var(--vscode-focusBorder);
      outline-offset: -1px;
    }
    #threadSelect option {
      background-color: var(--vscode-dropdown-background);
      color: var(--vscode-dropdown-foreground);
    }
    .thread-icon-btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 28px;
      height: 28px;
      padding: 0;
      flex-shrink: 0;
      border: none;
      border-radius: 5px;
      background: transparent;
      color: var(--vscode-icon-foreground);
      cursor: pointer;
    }
    .thread-icon-btn:hover {
      background: var(--vscode-toolbar-hoverBackground);
      color: var(--vscode-icon-foreground);
    }
    .thread-icon-btn:focus-visible {
      outline: 1px solid var(--vscode-focusBorder);
      outline-offset: 1px;
    }
    .thread-icon-btn.danger:hover {
      background: var(--vscode-inputValidation-errorBackground, rgba(255,80,80,.12));
      color: var(--vscode-errorForeground);
    }
    .composer-dock {
      flex-shrink: 0;
      padding: 0;
      background: var(--vscode-sideBar-background);
      border-top: 1px solid var(--vscode-widget-border, rgba(128,128,128,.35));
    }
    /* Full-width composer: no side border/inset (was reading as “inner margin”) */
    .composer-card {
      background: var(--vscode-editor-background);
      border: none;
      border-top: 1px solid var(--vscode-widget-border, rgba(128,128,128,.25));
      border-radius: 0;
      padding: 8px var(--wk-pad-x) 10px;
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    .ctx-toolbar {
      display: flex;
      align-items: center;
      gap: 8px;
      flex-wrap: wrap;
    }
    .ctx-toolbar-label {
      font-size: 0.62rem;
      font-weight: 600;
      letter-spacing: 0.06em;
      text-transform: uppercase;
      opacity: 0.5;
    }
    .ctx-menus {
      display: flex;
      align-items: center;
      gap: 6px;
    }
    .ctx-drop {
      position: relative;
    }
    .ctx-drop > summary {
      list-style: none;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 32px;
      height: 32px;
      border-radius: 8px;
      border: 1px solid var(--vscode-widget-border, rgba(128,128,128,.45));
      background: var(--vscode-input-background);
      color: var(--vscode-foreground);
    }
    .ctx-drop > summary::-webkit-details-marker { display: none; }
    .ctx-drop > summary:hover {
      border-color: var(--vscode-focusBorder, #007fd4);
      background: var(--vscode-list-hoverBackground);
    }
    .ctx-drop[open] > summary {
      border-color: var(--vscode-focusBorder, #007fd4);
    }
    .ctx-list {
      position: absolute;
      bottom: calc(100% + 6px);
      left: 0;
      margin: 0;
      padding: 4px 0;
      list-style: none;
      min-width: 220px;
      max-width: min(320px, 92vw);
      max-height: 200px;
      overflow-y: auto;
      z-index: 50;
      background: var(--vscode-dropdown-background);
      color: var(--vscode-dropdown-foreground);
      border: 1px solid var(--vscode-widget-border, #555);
      border-radius: 8px;
      box-shadow: 0 4px 16px rgba(0,0,0,.25);
    }
    .ctx-list-item {
      display: block;
      width: 100%;
      text-align: left;
      padding: 6px 12px;
      font-size: var(--vscode-font-size);
      font-family: var(--vscode-font-family);
      background: transparent;
      color: inherit;
      border: none;
      cursor: pointer;
    }
    .ctx-list-item:hover {
      background: var(--vscode-list-hoverBackground);
    }
    .ctx-list-empty {
      padding: 8px 12px;
      font-size: 0.75rem;
      opacity: 0.6;
    }
    button.ctx-refresh {
      margin-left: auto;
      width: 32px;
      height: 32px;
      border-radius: 8px;
      border: 1px solid var(--vscode-widget-border);
      background: var(--vscode-button-secondaryBackground);
      color: var(--vscode-button-secondaryForeground);
      cursor: pointer;
    }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 5px;
      min-height: 4px;
      align-items: center;
    }
    .chip {
      font-size: 0.7rem;
      padding: 3px 8px 3px 6px;
      border-radius: 999px;
      background: var(--vscode-badge-background);
      color: var(--vscode-badge-foreground);
      display: inline-flex;
      align-items: center;
      gap: 4px;
      max-width: 100%;
    }
    .chip-ic {
      display: inline-flex;
      flex-shrink: 0;
      opacity: 0.95;
    }
    .chip button {
      border: none;
      background: transparent;
      color: inherit;
      cursor: pointer;
      padding: 0 2px;
      font-size: 0.9rem;
      line-height: 1;
      opacity: 0.85;
    }
    .input-row {
      display: flex;
      gap: 6px;
      align-items: stretch;
    }
    #input {
      flex: 1;
      min-width: 0;
      min-height: 44px;
      max-height: 160px;
      resize: vertical;
      padding: 7px 9px;
      font-family: var(--vscode-editor-font-family);
      font-size: var(--vscode-editor-font-size);
      background: var(--vscode-input-background);
      color: var(--vscode-input-foreground);
      border: 1px solid var(--vscode-input-border, #555);
      border-radius: 6px;
    }
    #input:focus {
      outline: 1px solid var(--vscode-focusBorder, #007fd4);
      outline-offset: -1px;
    }
    /* Single slot: send OR stop (icon-only, theme-aware) */
    .actions {
      position: relative;
      width: 34px;
      min-width: 34px;
      flex-shrink: 0;
      align-self: stretch;
    }
    .actions button.icon-action {
      position: absolute;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      width: 30px;
      height: 30px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      border: none;
      border-radius: 6px;
      background: transparent;
      color: var(--vscode-icon-foreground);
    }
    .actions button.icon-action:hover {
      background: var(--vscode-toolbar-hoverBackground);
      color: var(--vscode-textLink-foreground);
    }
    #stop {
      color: var(--vscode-errorForeground);
    }
    #stop:hover {
      background: var(--vscode-inputValidation-errorBackground, rgba(255,80,80,.15));
      color: var(--vscode-errorForeground);
    }
  </style>
</head>
<body>
  <script nonce="${nonce}">const STR = ${strJson};</script>
  <header class="thread-strip">
    <label for="threadSelect">${escapeForHtml(str.thread)}</label>
    <select id="threadSelect" aria-label="${escapeForHtml(str.ariaActiveThread)}"></select>
    <button type="button" class="thread-icon-btn" id="newThread" title="${escapeForHtml(str.newThread)}" aria-label="${escapeForHtml(str.newThread)}">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
    </button>
    <button type="button" class="thread-icon-btn" id="renameThread" title="${escapeForHtml(str.renameThread)}" aria-label="${escapeForHtml(str.renameThread)}">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
    </button>
    <button type="button" class="thread-icon-btn danger" id="delThread" title="${escapeForHtml(str.deleteThread)}" aria-label="${escapeForHtml(str.deleteThread)}">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14z"/><path d="M10 11v6M14 11v6"/></svg>
    </button>
  </header>
  <div id="thread" role="log" aria-live="polite" aria-relevant="additions"></div>
  <footer class="composer-dock">
    <div class="composer-card">
      <div class="ctx-toolbar">
        <span class="ctx-toolbar-label">${escapeForHtml(str.context)}</span>
        <div class="ctx-menus">
          <details class="ctx-drop" id="menuScene">
            <summary class="ctx-icon-btn" title="${escapeForHtml(str.addScene)}">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h3l1.5-2h2L14 10h4"/></svg>
            </summary>
            <ul class="ctx-list" id="listScene"></ul>
          </details>
          <details class="ctx-drop" id="menuChar">
            <summary class="ctx-icon-btn" title="${escapeForHtml(str.addCharacter)}">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="8" r="3.5"/><path d="M5 20v-1a5 5 0 015-5h4a5 5 0 015 5v1"/></svg>
            </summary>
            <ul class="ctx-list" id="listChar"></ul>
          </details>
          <details class="ctx-drop" id="menuWorld">
            <summary class="ctx-icon-btn" title="${escapeForHtml(str.addWorldEntry)}">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a14 14 0 010 20M12 2a14 14 0 000 20"/></svg>
            </summary>
            <ul class="ctx-list" id="listWorld"></ul>
          </details>
        </div>
        <button type="button" class="ctx-refresh" id="refresh" title="${escapeForHtml(str.reloadLists)}">↻</button>
      </div>
      <div class="chips" id="chips" aria-label="${escapeForHtml(str.contextTagsAria)}"></div>
      <div class="input-row">
        <textarea id="input" placeholder="${escapeForHtml(str.messagePlaceholder)}" rows="2"></textarea>
        <div class="actions">
          <button type="button" class="icon-action" id="send" title="${escapeForHtml(str.sendTitle)}" aria-label="${escapeForHtml(str.sendAria)}">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M2 21l21-9L2 3v7l15 2-15 2v7z"/></svg>
          </button>
          <button type="button" class="icon-action" id="stop" hidden title="${escapeForHtml(str.stopTitle)}" aria-label="${escapeForHtml(str.stopAria)}">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="1"/></svg>
          </button>
        </div>
      </div>
    </div>
  </footer>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    const threadEl = document.getElementById('thread');
    const input = document.getElementById('input');
    const chipsEl = document.getElementById('chips');
    const threadSelect = document.getElementById('threadSelect');
    const sendBtn = document.getElementById('send');
    const stopBtn = document.getElementById('stop');
    const refreshBtn = document.getElementById('refresh');
    const newThreadBtn = document.getElementById('newThread');
    const renameThreadBtn = document.getElementById('renameThread');
    const delThreadBtn = document.getElementById('delThread');
    const listScene = document.getElementById('listScene');
    const listChar = document.getElementById('listChar');
    const listWorld = document.getElementById('listWorld');

    sendBtn.hidden = false;
    stopBtn.hidden = true;

    const ICON_SCENE = '<svg class="chip-ic" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h3l1.5-2h2L14 10h4"/></svg>';
    const ICON_CHAR = '<svg class="chip-ic" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="8" r="3"/><path d="M5 20v-1a5 5 0 015-5h4a5 5 0 015 5v1"/></svg>';
    const ICON_WORLD = '<svg class="chip-ic" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/></svg>';
    const ICON_SELECTION = '<svg class="chip-ic" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 7h10M4 12h16M4 17h8"/></svg>';

    let bootstrap = { scenes: [], characters: [], world: [], charactersCategory: 'Characters', worldCategory: 'World' };
    /** @type {Array<{kind:string, uuid?:string, label?:string, name?:string}>} */
    let chips = [];
    let assistantEl = null;

    function chipKey(c) {
      if (c.kind === 'scene') return 'scene:' + c.uuid;
      if (c.kind === 'selection') {
        var a = c.attachment || {};
        return (
          'selection:' +
          (a.scene_uuid || a.compendium_category || '') +
          ':' +
          (a.range_label || '') +
          ':' +
          String(c.text || '').slice(0, 32)
        );
      }
      return c.kind + ':' + c.name;
    }

    function closeAllCtxMenus() {
      ['menuScene', 'menuChar', 'menuWorld'].forEach(function (id) {
        const d = document.getElementById(id);
        if (d) d.open = false;
      });
    }

    ['menuScene', 'menuChar', 'menuWorld'].forEach(function (id) {
      const el = document.getElementById(id);
      if (!el) return;
      el.addEventListener('toggle', function () {
        if (el.open) {
          ['menuScene', 'menuChar', 'menuWorld'].forEach(function (oid) {
            if (oid !== id) {
              const o = document.getElementById(oid);
              if (o) o.open = false;
            }
          });
        }
      });
    });

    function addChipFromPicker(c) {
      const k = chipKey(c);
      if (chips.some(function (x) { return chipKey(x) === k; })) return;
      chips.push(c);
      renderChips();
    }

    function buildContextMenus() {
      listScene.innerHTML = '';
      listChar.innerHTML = '';
      listWorld.innerHTML = '';
      if (!bootstrap.scenes.length) {
        listScene.innerHTML = '<li class="ctx-list-empty">' + STR.noScenesInStructure + '</li>';
      } else {
        bootstrap.scenes.forEach(function (s) {
          const li = document.createElement('li');
          const b = document.createElement('button');
          b.type = 'button';
          b.className = 'ctx-list-item';
          b.textContent = s.label + ' · ' + s.uuid.slice(0, 8) + '…';
          b.addEventListener('click', function () {
            addChipFromPicker({ kind: 'scene', uuid: s.uuid, label: s.label });
            closeAllCtxMenus();
          });
          li.appendChild(b);
          listScene.appendChild(li);
        });
      }
      if (!bootstrap.characters.length) {
        listChar.innerHTML = '<li class="ctx-list-empty">' + STR.noCharacters + '</li>';
      } else {
        bootstrap.characters.forEach(function (c) {
          const li = document.createElement('li');
          const b = document.createElement('button');
          b.type = 'button';
          b.className = 'ctx-list-item';
          b.textContent = c.name;
          b.addEventListener('click', function () {
            addChipFromPicker({ kind: 'character', name: c.name });
            closeAllCtxMenus();
          });
          li.appendChild(b);
          listChar.appendChild(li);
        });
      }
      if (!bootstrap.world.length) {
        listWorld.innerHTML = '<li class="ctx-list-empty">' + STR.noWorldEntries + '</li>';
      } else {
        bootstrap.world.forEach(function (c) {
          const li = document.createElement('li');
          const b = document.createElement('button');
          b.type = 'button';
          b.className = 'ctx-list-item';
          b.textContent = c.name;
          b.addEventListener('click', function () {
            addChipFromPicker({ kind: 'world', name: c.name });
            closeAllCtxMenus();
          });
          li.appendChild(b);
          listWorld.appendChild(li);
        });
      }
    }

    function renderChips() {
      chipsEl.innerHTML = '';
      chips.forEach(function (c, idx) {
        const span = document.createElement('span');
        span.className = 'chip';
        var lab;
        var ic;
        if (c.kind === 'scene') {
          lab = c.label || c.uuid;
          ic = ICON_SCENE;
        } else if (c.kind === 'character') {
          lab = c.name;
          ic = ICON_CHAR;
        } else if (c.kind === 'world') {
          lab = c.name;
          ic = ICON_WORLD;
        } else {
          var att = c.attachment || {};
          lab = (att.label || '') + (att.range_label ? ' · ' + att.range_label : '');
          ic = ICON_SELECTION;
        }
        if (c.kind === 'selection' && c.detailTitle) {
          span.title = c.detailTitle;
        }
        span.insertAdjacentHTML('afterbegin', ic);
        span.appendChild(document.createTextNode(' ' + lab));
        const x = document.createElement('button');
        x.type = 'button';
        x.setAttribute('aria-label', STR.removeChipAria);
        x.textContent = '×';
        x.addEventListener('click', function () {
          chips = chips.filter(function (_, i) { return i !== idx; });
          renderChips();
        });
        span.appendChild(x);
        chipsEl.appendChild(span);
      });
    }

    function sceneLabelFromBootstrap(uuid) {
      if (!uuid) return '';
      var list = (bootstrap && bootstrap.scenes) || [];
      for (var i = 0; i < list.length; i++) {
        if (list[i].uuid === uuid) {
          return (list[i].label && String(list[i].label).trim()) || uuid.slice(0, 8);
        }
      }
      return uuid.slice(0, 8);
    }

    /** Localized category for UI when data uses default English names; custom names stay as-is. */
    function localizedCompendiumCategoryLabel(cat) {
      if (!cat) return '';
      var ch = bootstrap && bootstrap.charactersCategory;
      var wo = bootstrap && bootstrap.worldCategory;
      if (ch && cat === ch) {
        return cat === 'Characters' ? STR.categoryCharacters : cat;
      }
      if (wo && cat === wo) {
        return cat === 'World' ? STR.categoryWorld : cat;
      }
      return cat;
    }

    function tagLineFromApiContext(ctx) {
      if (!ctx) return '';
      var parts = [];
      (ctx.scene_uuids || []).forEach(function (u) {
        if (u) parts.push(STR.sceneTag + sceneLabelFromBootstrap(String(u)));
      });
      (ctx.compendium_excerpts || []).forEach(function (ex) {
        if (!ex) return;
        var cat = ex.category || '';
        var nm = ex.name || '';
        var ch = bootstrap && bootstrap.charactersCategory;
        var wo = bootstrap && bootstrap.worldCategory;
        if (ch && cat === ch) parts.push(STR.charTag + nm);
        else if (wo && cat === wo) parts.push(STR.worldTag + nm);
        else if (cat && nm) {
          parts.push(STR.compendiumPrefix + localizedCompendiumCategoryLabel(cat) + ' / ' + nm);
        }
      });
      (ctx.selection_attachments || []).forEach(function (a) {
        if (!a || !a.label) return;
        var rng = a.range_label ? ' · ' + a.range_label : '';
        if (a.kind === 'scene') parts.push(STR.sceneTag + a.label + rng);
        else if (a.kind === 'compendium') {
          var ccat = a.compendium_category || '';
          var cnm = a.compendium_name || '';
          var ch = bootstrap && bootstrap.charactersCategory;
          var wo = bootstrap && bootstrap.worldCategory;
          if (ch && ccat === ch) parts.push(STR.charTag + cnm + rng);
          else if (wo && ccat === wo) parts.push(STR.worldTag + cnm + rng);
          else if (ccat && cnm) {
            parts.push(STR.compendiumPrefix + localizedCompendiumCategoryLabel(ccat) + ' / ' + cnm + rng);
          }
        } else {
          parts.push(STR.selectionTag + a.label + rng);
        }
      });
      if (!(ctx.selection_attachments || []).length) {
        (ctx.selection_labels || []).forEach(function (lbl) {
          if (lbl) parts.push(STR.selectionTag + lbl);
        });
      }
      return parts.join(' · ');
    }

    function appendAssistantActions(shell, prevUser, assistantText) {
      var ctx = prevUser && prevUser.role === 'user' ? prevUser.context : null;
      if (!ctx) return;
      var reply = assistantText != null ? String(assistantText) : '';
      var actions = document.createElement('div');
      actions.className = 'bubble-actions';
      var seenScene = {};
      var seenComp = {};

      function addSceneBtn(uid) {
        if (!uid || seenScene[uid]) return;
        seenScene[uid] = 1;
        var b = document.createElement('button');
        b.type = 'button';
        b.className = 'bubble-action';
        b.textContent = STR.insertScenePrefix + sceneLabelFromBootstrap(String(uid));
        b.addEventListener('click', function () {
          vscode.postMessage({
            type: 'workshopResponseAction',
            action: 'insertScene',
            sceneUuid: String(uid),
            assistantText: reply,
          });
        });
        actions.appendChild(b);
      }
      function addCompBtn(cat, name) {
        if (!cat || !name) return;
        var k = cat + '\0' + name;
        if (seenComp[k]) return;
        seenComp[k] = 1;
        var b = document.createElement('button');
        b.type = 'button';
        b.className = 'bubble-action';
        b.textContent =
          STR.insertIntoPrefix + localizedCompendiumCategoryLabel(cat) + ' — ' + name;
        b.addEventListener('click', function () {
          vscode.postMessage({
            type: 'workshopResponseAction',
            action: 'insertCompendium',
            compendiumCategory: String(cat),
            compendiumName: String(name),
            assistantText: reply,
          });
        });
        actions.appendChild(b);
      }

      (ctx.scene_uuids || []).forEach(function (uid) {
        addSceneBtn(uid ? String(uid) : '');
      });
      (ctx.compendium_excerpts || []).forEach(function (ex) {
        if (ex && ex.category && ex.name) addCompBtn(ex.category, ex.name);
      });
      (ctx.selection_attachments || []).forEach(function (a) {
        if (!a) return;
        if (a.kind === 'scene' && a.scene_uuid) addSceneBtn(String(a.scene_uuid));
        if (a.kind === 'compendium' && a.compendium_category && a.compendium_name) {
          addCompBtn(String(a.compendium_category), String(a.compendium_name));
        }
      });

      if (!actions.children.length) return;
      shell.appendChild(actions);
    }

    function renderThreadHistory(messages) {
      threadEl.innerHTML = '';
      var arr = messages || [];
      arr.forEach(function (m, i) {
        const wrap = document.createElement('div');
        wrap.className = 'bubble ' + (m.role === 'user' ? 'user' : 'assistant');
        const meta = document.createElement('div');
        meta.className = 'bubble-meta';
        if (m.role === 'user') {
          const you = document.createElement('span');
          you.textContent = STR.you;
          meta.appendChild(you);
          var tl = tagLineFromApiContext(m.context);
          if (tl) {
            const tg = document.createElement('span');
            tg.className = 'bubble-tags';
            tg.textContent = ' (' + tl + ')';
            meta.appendChild(tg);
          }
        } else {
          meta.textContent = STR.workshop;
        }
        const shell = document.createElement('div');
        shell.className = 'bubble-body';
        const body = document.createElement('div');
        body.className = m.role === 'assistant' ? 'body inner' : 'inner';
        body.textContent = m.content || '';
        shell.appendChild(body);
        if (m.role === 'assistant' && i > 0) {
          appendAssistantActions(shell, arr[i - 1], m.content || '');
        }
        wrap.appendChild(meta);
        wrap.appendChild(shell);
        threadEl.appendChild(wrap);
      });
      threadEl.scrollTop = threadEl.scrollHeight;
    }

    function populateThreadSelect(threads, activeId) {
      threadSelect.innerHTML = '';
      (threads || []).forEach(function (t) {
        const o = document.createElement('option');
        o.value = t.thread_id;
        const lab = t.title || STR.thread;
        o.textContent = lab.length > 52 ? lab.slice(0, 50) + '…' : lab;
        o.title = lab;
        threadSelect.appendChild(o);
      });
      for (let i = 0; i < threadSelect.options.length; i++) {
        if (threadSelect.options[i].value === activeId) {
          threadSelect.selectedIndex = i;
          break;
        }
      }
    }

    refreshBtn.addEventListener('click', function () { vscode.postMessage({ type: 'refreshBootstrap' }); });
    newThreadBtn.addEventListener('click', function () { vscode.postMessage({ type: 'newThread' }); });
    renameThreadBtn.addEventListener('click', function () {
      const id = threadSelect.value;
      if (id) vscode.postMessage({ type: 'renameThread', threadId: id });
    });
    delThreadBtn.addEventListener('click', function () {
      const id = threadSelect.value;
      if (id) vscode.postMessage({ type: 'deleteThread', threadId: id });
    });
    threadSelect.addEventListener('change', function () {
      const id = threadSelect.value;
      if (id) vscode.postMessage({ type: 'selectThread', threadId: id });
    });

    function appendUser(text, tagLine) {
      const wrap = document.createElement('div');
      wrap.className = 'bubble user';
      const meta = document.createElement('div');
      meta.className = 'bubble-meta';
      const you = document.createElement('span');
      you.textContent = STR.you;
      meta.appendChild(you);
      if (tagLine && String(tagLine).trim()) {
        const tg = document.createElement('span');
        tg.className = 'bubble-tags';
        tg.textContent = ' (' + String(tagLine).trim() + ')';
        meta.appendChild(tg);
      }
      const shell = document.createElement('div');
      shell.className = 'bubble-body';
      const body = document.createElement('div');
      body.className = 'inner';
      body.textContent = text;
      shell.appendChild(body);
      wrap.appendChild(meta);
      wrap.appendChild(shell);
      threadEl.appendChild(wrap);
      threadEl.scrollTop = threadEl.scrollHeight;
    }

    function startAssistant() {
      assistantEl = document.createElement('div');
      assistantEl.className = 'bubble assistant';
      const meta = document.createElement('div');
      meta.className = 'bubble-meta';
      meta.textContent = STR.workshop;
      const shell = document.createElement('div');
      shell.className = 'bubble-body';
      const body = document.createElement('div');
      body.className = 'body inner';
      shell.appendChild(body);
      assistantEl.appendChild(meta);
      assistantEl.appendChild(shell);
      threadEl.appendChild(assistantEl);
      threadEl.scrollTop = threadEl.scrollHeight;
    }

    function appendChunk(t) {
      const body = assistantEl && assistantEl.querySelector('.body.inner');
      if (body) body.textContent += t;
      threadEl.scrollTop = threadEl.scrollHeight;
    }

    function showError(t) {
      const d = document.createElement('div');
      d.className = 'bubble error assistant';
      const meta = document.createElement('div');
      meta.className = 'bubble-meta';
      meta.textContent = STR.error;
      const shell = document.createElement('div');
      shell.className = 'bubble-body';
      const body = document.createElement('div');
      body.className = 'inner';
      body.textContent = t;
      shell.appendChild(body);
      d.appendChild(meta);
      d.appendChild(shell);
      threadEl.appendChild(d);
      threadEl.scrollTop = threadEl.scrollHeight;
    }

    function setBusy(b) {
      sendBtn.hidden = b;
      stopBtn.hidden = !b;
    }

    function doSend() {
      const t = input.value;
      if (!t.trim()) return;
      vscode.postMessage({ type: 'send', text: t, chips: chips.slice() });
      input.value = '';
    }

    sendBtn.addEventListener('click', doSend);
    input.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        doSend();
      }
    });
    stopBtn.addEventListener('click', function () { vscode.postMessage({ type: 'stop' }); });

    function applyContextPayload(p) {
      if (!p || !p.mode || p.mode === 'none') return;
      if (p.mode === 'scene' && p.sceneUuid) {
        const c = { kind: 'scene', uuid: p.sceneUuid, label: p.sceneLabel || '' };
        if (!chips.some(function (x) { return chipKey(x) === chipKey(c); })) chips.push(c);
      } else if (p.mode === 'character' && p.entryName) {
        const c = { kind: 'character', name: p.entryName };
        if (!chips.some(function (x) { return chipKey(x) === chipKey(c); })) chips.push(c);
      } else if (p.mode === 'world' && p.entryName) {
        const c = { kind: 'world', name: p.entryName };
        if (!chips.some(function (x) { return chipKey(x) === chipKey(c); })) chips.push(c);
      } else if (p.mode === 'selection' && Array.isArray(p.selectionItems) && p.selectionItems.length) {
        p.selectionItems.forEach(function (item) {
          if (!item || !String(item.text || '').trim() || !item.attachment) return;
          const c = {
            kind: 'selection',
            text: String(item.text),
            attachment: item.attachment,
            detailTitle: item.detailTitle || '',
          };
          if (!chips.some(function (x) { return chipKey(x) === chipKey(c); })) chips.push(c);
        });
      }
      renderChips();
    }

    window.addEventListener('message', function (event) {
      const m = event.data;
      if (!m || typeof m.type !== 'string') return;
      if (m.type === 'bootstrap' && m.payload) {
        bootstrap = m.payload;
        if (m.error) showError(STR.contextErrorPrefix + m.error);
        buildContextMenus();
        return;
      }
      if (m.type === 'workshopState') {
        populateThreadSelect(m.threads, m.activeThreadId);
        renderThreadHistory(m.messages);
        return;
      }
      if (m.type === 'setInput' && typeof m.text === 'string') {
        input.value = m.text;
        input.focus();
        return;
      }
      if (m.type === 'applyContext') {
        applyContextPayload(m);
        return;
      }
      if (m.type === 'user') {
        appendUser(m.text || '', m.tagLine);
        setBusy(true);
        return;
      }
      if (m.type === 'assistantStart') {
        startAssistant();
        return;
      }
      if (m.type === 'assistantChunk' && typeof m.text === 'string') {
        appendChunk(m.text);
        return;
      }
      if (m.type === 'assistantEnd') {
        assistantEl = null;
        setBusy(false);
        // Selection excerpts apply to one turn only; picker chips (scene / compendium) remain.
        chips = chips.filter(function (c) { return c.kind !== 'selection'; });
        renderChips();
        return;
      }
      if (m.type === 'error' && typeof m.text === 'string') {
        showError(m.text);
        assistantEl = null;
        setBusy(false);
      }
    });
    buildContextMenus();
  </script>
</body>
</html>`;
}
