import { describe, expect, it } from 'vitest';

import { expandLeadingTildePath } from './pathUtils';

describe('expandLeadingTildePath', () => {
  it('expands ~ alone', () => {
    expect(expandLeadingTildePath('~', '/Users/x')).toBe('/Users/x');
  });

  it('expands ~/path', () => {
    expect(expandLeadingTildePath('~/bin/api', '/Users/x')).toBe('/Users/x/bin/api');
  });

  it('leaves non-tilde paths', () => {
    expect(expandLeadingTildePath('/abs', '/Users/x')).toBe('/abs');
  });

  it('keeps ~ when home missing', () => {
    expect(expandLeadingTildePath('~/x', '')).toBe('~/x');
  });
});
