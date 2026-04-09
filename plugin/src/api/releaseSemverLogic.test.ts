import { describe, expect, it } from 'vitest';

import { effectiveApiReleaseSemver } from './releaseSemverLogic';

describe('effectiveApiReleaseSemver', () => {
  it('uses bundled when tag empty', () => {
    expect(effectiveApiReleaseSemver('', '0.1.0')).toBe('0.1.0');
  });

  it('strips v from tag', () => {
    expect(effectiveApiReleaseSemver('v0.2.0', '0.1.0')).toBe('0.2.0');
  });

  it('trims tag', () => {
    expect(effectiveApiReleaseSemver('  v0.3.0  ', '0.1.0')).toBe('0.3.0');
  });
});
