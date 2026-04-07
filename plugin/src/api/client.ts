/**
 * HTTP client for AuthorKit API - all project routes pass workspace_root.
 */

export interface CompendiumExcerptRef {
  category: string;
  name: string;
}

/** Mirrors API `WorkshopMessageContext`; persisted on user turns. */
export interface WorkshopMessageContext {
  scene_uuids?: string[];
  compendium_excerpts?: CompendiumExcerptRef[];
}

export interface ChatMessage {
  role: string;
  content: string;
  /** Present on user messages when scenes/compendium were attached. */
  context?: WorkshopMessageContext | null;
}

export interface CompendiumEntry {
  name: string;
  /** Inline body; ignored when `id` is set and the Markdown file exists (API reads the file). */
  content: string;
  /** Stable id: source of truth is `.authorkit/<categorySlug>/<id>.md`. */
  id?: string;
}

export interface CompendiumData {
  categories: Array<{
    name: string;
    entries: CompendiumEntry[];
  }>;
}

export interface StructureData {
  acts: unknown[];
}

/** Global API / LLM settings (`~/.authorkit/settings.json`). */
export interface LlmProfileConfig {
  provider: string;
  endpoint: string;
  model: string;
  api_key: string;
  timeout?: number;
}

export interface AppSettingsData {
  version?: string;
  general?: Record<string, unknown>;
  llm_configs: Record<string, LlmProfileConfig>;
  active_llm_config: string;
}

export async function getAppSettings(baseUrl: string): Promise<AppSettingsData> {
  return jsonFetch(baseUrl, '/v1/app/settings');
}

export async function putAppSettings(baseUrl: string, data: AppSettingsData): Promise<void> {
  await jsonFetch(baseUrl, '/v1/app/settings', {
    method: 'PUT',
    body: JSON.stringify(data),
  });
}

async function jsonFetch<T>(
  baseUrl: string,
  path: string,
  init?: RequestInit
): Promise<T> {
  const url = `${baseUrl}${path}`;
  const res = await fetch(url, {
    ...init,
    headers: {
      Accept: 'application/json',
      'Content-Type': 'application/json',
      ...init?.headers,
    },
  });
  if (!res.ok) {
    const t = await res.text();
    throw new Error(`${res.status} ${res.statusText}: ${t.slice(0, 500)}`);
  }
  return res.json() as Promise<T>;
}

function qWorkspace(workspaceRoot: string): string {
  return `workspace_root=${encodeURIComponent(workspaceRoot)}`;
}

export async function health(baseUrl: string): Promise<{ status: string }> {
  return jsonFetch(baseUrl, '/health');
}

export async function ready(baseUrl: string): Promise<Record<string, unknown>> {
  return jsonFetch(baseUrl, '/ready');
}

export async function getStructure(
  baseUrl: string,
  workspaceRoot: string
): Promise<StructureData> {
  return jsonFetch(baseUrl, `/v1/projects/structure?${qWorkspace(workspaceRoot)}`);
}

export async function putStructure(
  baseUrl: string,
  workspaceRoot: string,
  structure: StructureData
): Promise<void> {
  await jsonFetch(baseUrl, '/v1/projects/structure', {
    method: 'PUT',
    body: JSON.stringify({ workspace_root: workspaceRoot, structure }),
  });
}

export async function getCompendium(
  baseUrl: string,
  workspaceRoot: string
): Promise<CompendiumData> {
  return jsonFetch(baseUrl, `/v1/compendium?${qWorkspace(workspaceRoot)}`);
}

export async function putCompendium(
  baseUrl: string,
  workspaceRoot: string,
  data: CompendiumData
): Promise<void> {
  await jsonFetch(baseUrl, '/v1/compendium', {
    method: 'PUT',
    body: JSON.stringify({ workspace_root: workspaceRoot, data }),
  });
}

export async function putSceneContent(
  baseUrl: string,
  workspaceRoot: string,
  sceneUuid: string,
  content: string
): Promise<void> {
  await jsonFetch(baseUrl, `/v1/projects/scenes/${encodeURIComponent(sceneUuid)}`, {
    method: 'PUT',
    body: JSON.stringify({ workspace_root: workspaceRoot, content }),
  });
}

export interface WorkshopThreadSummary {
  thread_id: string;
  title: string;
  created_at: string;
  updated_at: string;
  seed_type?: string | null;
  seed_ref?: string | null;
}

export interface WorkshopStreamOptions {
  /** Profile key in \`llm_configs\` (sent as \`provider\` to the API). */
  provider?: string;
  /** Optional model id override. */
  model?: string;
  /** Server-side thread; loads/saves history in API SQLite. */
  threadId?: string;
  sceneUuids?: string[];
  compendiumExcerpts?: CompendiumExcerptRef[];
}

export async function listWorkshopThreads(
  baseUrl: string,
  workspaceRoot: string
): Promise<WorkshopThreadSummary[]> {
  return jsonFetch(baseUrl, `/v1/workshop/threads?${qWorkspace(workspaceRoot)}`);
}

export async function createWorkshopThread(
  baseUrl: string,
  workspaceRoot: string,
  opts?: { title?: string; seed_type?: string; seed_ref?: string }
): Promise<WorkshopThreadSummary> {
  return jsonFetch(baseUrl, '/v1/workshop/threads', {
    method: 'POST',
    body: JSON.stringify({
      workspace_root: workspaceRoot,
      title: opts?.title,
      seed_type: opts?.seed_type,
      seed_ref: opts?.seed_ref,
    }),
  });
}

export async function getWorkshopMessages(
  baseUrl: string,
  workspaceRoot: string,
  threadId: string
): Promise<ChatMessage[]> {
  const r = await jsonFetch<{ messages: ChatMessage[] }>(
    baseUrl,
    `/v1/workshop/threads/${encodeURIComponent(threadId)}/messages?${qWorkspace(workspaceRoot)}`
  );
  return r.messages;
}

export async function deleteWorkshopThread(
  baseUrl: string,
  workspaceRoot: string,
  threadId: string
): Promise<void> {
  await jsonFetch(baseUrl, `/v1/workshop/threads/${encodeURIComponent(threadId)}?${qWorkspace(workspaceRoot)}`, {
    method: 'DELETE',
  });
}

export async function renameWorkshopThread(
  baseUrl: string,
  workspaceRoot: string,
  threadId: string,
  title: string
): Promise<WorkshopThreadSummary> {
  return jsonFetch(
    baseUrl,
    `/v1/workshop/threads/${encodeURIComponent(threadId)}?${qWorkspace(workspaceRoot)}`,
    {
      method: 'PATCH',
      body: JSON.stringify({ title }),
    }
  );
}

/**
 * Stream workshop SSE: lines like `data: {"text":"..."}` and `data: [DONE]`
 */
export async function* workshopChatStream(
  baseUrl: string,
  workspaceRoot: string,
  userMessage: string,
  signal: AbortSignal,
  streamOpts?: WorkshopStreamOptions
): AsyncGenerator<string> {
  const body: Record<string, unknown> = {
    user_message: userMessage,
    workspace_root: workspaceRoot,
    use_rag: false,
  };
  if (streamOpts?.provider?.trim()) {
    body.provider = streamOpts.provider.trim();
  }
  if (streamOpts?.model?.trim()) {
    body.model = streamOpts.model.trim();
  }
  if (streamOpts?.threadId?.trim()) {
    body.thread_id = streamOpts.threadId.trim();
  }
  if (streamOpts?.sceneUuids?.length) {
    body.scene_uuids = streamOpts.sceneUuids;
  }
  if (streamOpts?.compendiumExcerpts?.length) {
    body.compendium_excerpts = streamOpts.compendiumExcerpts;
  }
  const res = await fetch(`${baseUrl}/v1/workshop/chat/stream`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
    signal,
  });
  if (!res.ok) {
    const t = await res.text();
    throw new Error(t.slice(0, 500));
  }
  if (!res.body) {
    throw new Error('No response body');
  }
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    buffer += decoder.decode(value, { stream: true });
    const parts = buffer.split('\n');
    buffer = parts.pop() ?? '';
    for (const line of parts) {
      if (!line.startsWith('data:')) {
        continue;
      }
      const payload = line.slice(5).trim();
      if (payload === '[DONE]') {
        return;
      }
      try {
        const j = JSON.parse(payload) as { text?: string; error?: string };
        if (j.error) {
          throw new Error(j.error);
        }
        if (j.text) {
          yield j.text;
        }
      } catch (e) {
        if (e instanceof SyntaxError) {
          continue;
        }
        throw e;
      }
    }
  }
}
