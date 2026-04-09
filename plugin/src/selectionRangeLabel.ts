import * as vscode from 'vscode';

import { selectionIntrinsicPartsFromOffsets } from './selectionLogic';

/**
 * Compact, unique label for a selection: line range + document character offsets (inclusive).
 */
export function formatSelectionRangeLabel(doc: vscode.TextDocument, sel: vscode.Selection): string {
  const o0 = doc.offsetAt(sel.start);
  const o1 = doc.offsetAt(sel.end);
  const p = selectionIntrinsicPartsFromOffsets(sel.start.line, sel.end.line, o0, o1);
  const lines =
    p.lineOneBasedStart === p.lineOneBasedEnd
      ? vscode.l10n.t('Ln {0}', String(p.lineOneBasedStart))
      : vscode.l10n.t('Ln {0}–{1}', String(p.lineOneBasedStart), String(p.lineOneBasedEnd));
  const chars = vscode.l10n.t('ch.{0}–{1}', String(p.charLo), String(p.charHi));
  return `${lines} · ${chars}`;
}
