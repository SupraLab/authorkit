import * as vscode from 'vscode';
import { openWorkshopChatPanel, type WorkshopApplyContext } from './views/workshopChatPanel';

export type { WorkshopApplyContext } from './views/workshopChatPanel';

/**
 * Focuses the Workshop panel and optionally pre-fills the message / context selectors.
 */
export async function openChatWithAuthorKitPrompt(
  context: vscode.ExtensionContext,
  initialPrompt: string,
  applyContext?: WorkshopApplyContext
): Promise<void> {
  openWorkshopChatPanel(context, { initialPrompt, applyContext });
}
