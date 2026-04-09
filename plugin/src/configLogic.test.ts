import { describe, expect, it } from 'vitest';

import {
  DEFAULT_API_BASE_URL,
  localApiHttpBase,
  normalizeApiBaseUrl,
  normalizeLocalApiPort,
  resolveApiBaseUrl,
  workshopLlmOptionsFromStrings,
} from './configLogic';

describe('normalizeApiBaseUrl', () => {
  it('strips trailing slash', () => {
    expect(normalizeApiBaseUrl('http://127.0.0.1:8765/')).toBe('http://127.0.0.1:8765');
  });

  it('uses default when undefined', () => {
    expect(normalizeApiBaseUrl(undefined)).toBe(DEFAULT_API_BASE_URL);
  });
});

describe('normalizeLocalApiPort', () => {
  it('keeps valid number', () => {
    expect(normalizeLocalApiPort(9000)).toBe(9000);
  });

  it('defaults on bad input', () => {
    expect(normalizeLocalApiPort(undefined)).toBe(8765);
    expect(normalizeLocalApiPort(NaN)).toBe(8765);
  });
});

describe('localApiHttpBase', () => {
  it('builds 127.0.0.1 URL', () => {
    expect(localApiHttpBase(8765)).toBe('http://127.0.0.1:8765');
  });
});

describe('resolveApiBaseUrl', () => {
  it('uses local when startLocalApi', () => {
    expect(
      resolveApiBaseUrl({
        startLocalApi: true,
        localApiPort: 9000,
        apiBaseUrl: 'http://example.com',
      })
    ).toBe('http://127.0.0.1:9000');
  });

  it('uses apiBaseUrl when not local', () => {
    expect(
      resolveApiBaseUrl({
        startLocalApi: false,
        localApiPort: 8765,
        apiBaseUrl: 'http://192.168.1.1:9999/',
      })
    ).toBe('http://192.168.1.1:9999');
  });
});

describe('workshopLlmOptionsFromStrings', () => {
  it('omits empty', () => {
    expect(workshopLlmOptionsFromStrings(undefined, undefined)).toEqual({});
  });

  it('trims and sets both', () => {
    expect(workshopLlmOptionsFromStrings('  p1  ', '  m1  ')).toEqual({ provider: 'p1', model: 'm1' });
  });
});
