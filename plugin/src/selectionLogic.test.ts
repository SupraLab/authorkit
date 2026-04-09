import { describe, expect, it } from 'vitest';

import { selectionIntrinsicPartsFromOffsets } from './selectionLogic';

describe('selectionIntrinsicPartsFromOffsets', () => {
  it('orders char range when selection reversed', () => {
    const p = selectionIntrinsicPartsFromOffsets(0, 0, 100, 10);
    expect(p.charLo).toBe(10);
    expect(p.charHi).toBe(100);
    expect(p.lineOneBasedStart).toBe(1);
    expect(p.lineOneBasedEnd).toBe(1);
  });

  it('handles multi-line', () => {
    const p = selectionIntrinsicPartsFromOffsets(0, 2, 5, 20);
    expect(p.lineOneBasedStart).toBe(1);
    expect(p.lineOneBasedEnd).toBe(3);
  });
});
