import * as vscode from 'vscode';
import { getApiBaseUrl, getWorkspaceRoot } from '../config';
import { formatSelectionRangeLabel } from '../selectionRangeLabel';
import { resolveSelectionAttachment, type SelectionAttachment } from '../selectionContext';
import { openWorkshopChatPanel } from '../views/workshopChatPanel';

const MAX_SELECTION_CHARS = 80_000;

/**
 * Adds the active editor's selection(s) as Workshop context chips (like Cursor's selection context).
 */
export async function addEditorSelectionToWorkshop(context: vscode.ExtensionContext): Promise<void> {
  const root = getWorkspaceRoot();
  if (!root) {
    void vscode.window.showWarningMessage(
      vscode.l10n.t('Open a folder workspace to use the Workshop.')
    );
    return;
  }
  const ed = vscode.window.activeTextEditor;
  if (!ed) {
    void vscode.window.showWarningMessage(
      vscode.l10n.t('Open a text editor, then select the passage to send as Workshop context.')
    );
    return;
  }
  const baseUrl = getApiBaseUrl();
  let attachment;
  try {
    attachment = await resolveSelectionAttachment(root, ed.document.uri, baseUrl);
  } catch (e) {
    const m = e instanceof Error ? e.message : String(e);
    void vscode.window.showErrorMessage(
      vscode.l10n.t('Could not resolve AuthorKit context for this file: {0}', m)
    );
    return;
  }

  const items: Array<{
    text: string;
    attachment: SelectionAttachment;
    detailTitle: string;
  }> = [];
  let truncated = false;
  for (const sel of ed.selections) {
    let text = ed.document.getText(sel);
    if (!text.trim()) {
      continue;
    }
    if (text.length > MAX_SELECTION_CHARS) {
      text = text.slice(0, MAX_SELECTION_CHARS) + '\n\n[…]';
      truncated = true;
    }
    const rangeLabel = formatSelectionRangeLabel(ed.document, sel);
    const att: SelectionAttachment = { ...attachment, range_label: rangeLabel };
    items.push({
      text,
      attachment: att,
      detailTitle: `${attachment.label} · ${rangeLabel}`,
    });
  }
  if (truncated) {
    void vscode.window.showWarningMessage(
      vscode.l10n.t('At least one selection was truncated (max {0} characters).', String(MAX_SELECTION_CHARS))
    );
  }
  if (!items.length) {
    void vscode.window.showWarningMessage(vscode.l10n.t('Select some text in the editor first.'));
    return;
  }
  openWorkshopChatPanel(context, {
    applyContext: { mode: 'selection', selectionItems: items },
  });
}
