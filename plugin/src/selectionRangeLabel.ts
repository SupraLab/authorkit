import * as vscode from 'vscode';

/**
 * Compact, unique label for a selection: line range + document character offsets (inclusive).
 */
export function formatSelectionRangeLabel(doc: vscode.TextDocument, sel: vscode.Selection): string {
  const o0 = doc.offsetAt(sel.start);
  const o1 = doc.offsetAt(sel.end);
  const lo = Math.min(o0, o1);
  const hi = Math.max(o0, o1);
  const l0 = sel.start.line + 1;
  const l1 = sel.end.line + 1;
  const lines =
    l0 === l1
      ? vscode.l10n.t('Ln {0}', String(l0))
      : vscode.l10n.t('Ln {0}–{1}', String(l0), String(l1));
  const chars = vscode.l10n.t('ch.{0}–{1}', String(lo), String(hi));
  return `${lines} · ${chars}`;
}
