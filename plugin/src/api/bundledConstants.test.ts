import { describe, expect, it } from 'vitest';

import {
  bundledSemverFromPackage,
  parseGithubRepoFromRepositoryUrl,
} from './bundledConstants';

describe('bundledSemverFromPackage', () => {
  it('prefers bundledApiVersion', () => {
    expect(bundledSemverFromPackage({ bundledApiVersion: '0.2.0', version: '0.1.0' })).toBe('0.2.0');
  });

  it('falls back to version', () => {
    expect(bundledSemverFromPackage({ version: '1.0.0' })).toBe('1.0.0');
  });

  it('defaults when empty', () => {
    expect(bundledSemverFromPackage({})).toBe('0.1.0');
    expect(bundledSemverFromPackage({ version: '' })).toBe('0.1.0');
  });
});

describe('parseGithubRepoFromRepositoryUrl', () => {
  it('parses https .git URL', () => {
    expect(
      parseGithubRepoFromRepositoryUrl('https://github.com/SupraLab/authorkit.git')
    ).toEqual({ owner: 'SupraLab', repo: 'authorkit' });
  });

  it('parses ssh URL', () => {
    expect(parseGithubRepoFromRepositoryUrl('git@github.com:acme/foo-bar.git')).toEqual({
      owner: 'acme',
      repo: 'foo-bar',
    });
  });

  it('returns defaults when no match', () => {
    expect(parseGithubRepoFromRepositoryUrl('')).toEqual({ owner: 'SupraLab', repo: 'authorkit' });
  });
});
