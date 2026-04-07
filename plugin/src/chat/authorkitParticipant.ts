import * as vscode from 'vscode';
import * as api from '../api/client';
import { getApiBaseUrl, getWorkshopLlmOptions, getWorkspaceRoot } from '../config';

const CHAT_API_HINT_KEY = 'authorkit.chat.workshopApiHintShown';

export function registerAuthorkitChatParticipant(
  context: vscode.ExtensionContext
): vscode.ChatParticipant {
  const participant = vscode.chat.createChatParticipant(
    'authorkit.author-kit',
    async (request, _context, response, token) => {
      const root = getWorkspaceRoot();
      if (!root) {
        response.markdown('Open a single folder workspace (**File → Open Folder**) to use AuthorKit chat.');
        return {};
      }
      const baseUrl = getApiBaseUrl();
      const llm = getWorkshopLlmOptions();
      const ac = new AbortController();
      token.onCancellationRequested(() => ac.abort());
      try {
        if (context.workspaceState.get(CHAT_API_HINT_KEY) !== true) {
          void context.workspaceState.update(CHAT_API_HINT_KEY, true);
          response.markdown(
            `*AuthorKit workshop at \`${baseUrl}\`. For the same flow without @authorKit, use **AuthorKit → Workshop** in the sidebar toolbar.*\n\n`
          );
        }
        response.progress('AuthorKit workshop…');
        for await (const chunk of api.workshopChatStream(
          baseUrl,
          root,
          request.prompt,
          ac.signal,
          llm
        )) {
          if (token.isCancellationRequested) {
            break;
          }
          response.markdown(chunk);
        }
      } catch (e) {
        if (e instanceof Error && e.name === 'AbortError') {
          response.markdown('*Cancelled.*');
        } else {
          const msg = e instanceof Error ? e.message : String(e);
          response.markdown(`**Error:** ${msg}`);
        }
      }
      return {};
    }
  );
  context.subscriptions.push(participant);
  return participant;
}
