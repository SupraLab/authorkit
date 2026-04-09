import { describe, expect, it } from 'vitest';

import { isValidUuid } from './uuidUtil';

describe('isValidUuid', () => {
  it('accepts RFC 4122 lowercase', () => {
    expect(isValidUuid('550e8400-e29b-41d4-a716-446655440000')).toBe(true);
  });

  it('accepts uppercase', () => {
    expect(isValidUuid('550E8400-E29B-41D4-A716-446655440000')).toBe(true);
  });

  it('rejects invalid variant nibble', () => {
    expect(isValidUuid('550e8400-e29b-41d4-c716-446655440000')).toBe(false);
  });

  it('rejects garbage', () => {
    expect(isValidUuid('not-a-uuid')).toBe(false);
    expect(isValidUuid('')).toBe(false);
  });

  it('trims whitespace', () => {
    expect(isValidUuid('  550e8400-e29b-41d4-a716-446655440000  ')).toBe(true);
  });
});
