/**
 * Selection label building blocks — numbers only; `selectionRangeLabel.ts` adds `vscode.l10n`.
 */

export type SelectionIntrinsicParts = {
  lineOneBasedStart: number;
  lineOneBasedEnd: number;
  charLo: number;
  charHi: number;
};

export function selectionIntrinsicPartsFromOffsets(
  startLine: number,
  endLine: number,
  startOffset: number,
  endOffset: number
): SelectionIntrinsicParts {
  const lo = Math.min(startOffset, endOffset);
  const hi = Math.max(startOffset, endOffset);
  return {
    lineOneBasedStart: startLine + 1,
    lineOneBasedEnd: endLine + 1,
    charLo: lo,
    charHi: hi,
  };
}
