import { describe, expect, it } from 'vitest';

import { platformTripletFromNode } from './platformTriplet';

describe('platformTripletFromNode', () => {
  it('maps darwin + arm64', () => {
    expect(platformTripletFromNode('darwin', 'arm64')).toBe('darwin-arm64');
  });

  it('maps darwin + x64', () => {
    expect(platformTripletFromNode('darwin', 'x64')).toBe('darwin-x64');
  });

  it('maps linux + x64', () => {
    expect(platformTripletFromNode('linux', 'x64')).toBe('linux-x64');
  });

  it('maps linux + arm64', () => {
    expect(platformTripletFromNode('linux', 'arm64')).toBe('linux-arm64');
  });

  it('maps win32 to win-amd64', () => {
    expect(platformTripletFromNode('win32', 'x64')).toBe('win-amd64');
  });

  it('falls back for unknown platform', () => {
    expect(platformTripletFromNode('freebsd', 'x64')).toBe('freebsd-x64');
  });
});
