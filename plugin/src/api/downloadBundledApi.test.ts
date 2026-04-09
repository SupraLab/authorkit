import { mkdtempSync, rmSync, writeFileSync } from 'fs';
import { tmpdir } from 'os';
import { join } from 'path';

import { describe, expect, it } from 'vitest';

import {
  assetDownloadUrl,
  releaseAssetName,
  releaseTagFromSemver,
} from './downloadBundledApi';

describe('releaseTagFromSemver', () => {
  it('adds v when missing', () => {
    expect(releaseTagFromSemver('0.1.0')).toBe('v0.1.0');
  });

  it('keeps leading v', () => {
    expect(releaseTagFromSemver('v0.1.0')).toBe('v0.1.0');
  });
});

describe('releaseAssetName', () => {
  it('builds zip filename', () => {
    expect(releaseAssetName('0.1.0', 'linux-x64')).toBe('author-kit-api-0.1.0-linux-x64.zip');
  });
});

describe('assetDownloadUrl', () => {
  it('builds GitHub release asset URL from extension package.json', () => {
    const dir = mkdtempSync(join(tmpdir(), 'authorkit-ext-test-'));
    try {
      writeFileSync(
        join(dir, 'package.json'),
        JSON.stringify({
          repository: { url: 'https://github.com/MyOrg/my-repo.git' },
        })
      );
      const { url, tag } = assetDownloadUrl(dir, '0.2.0', 'darwin-arm64');
      expect(tag).toBe('v0.2.0');
      expect(url).toBe(
        'https://github.com/MyOrg/my-repo/releases/download/v0.2.0/author-kit-api-0.2.0-darwin-arm64.zip'
      );
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('strips v from semver for asset name', () => {
    const dir = mkdtempSync(join(tmpdir(), 'authorkit-ext-test-'));
    try {
      writeFileSync(
        join(dir, 'package.json'),
        JSON.stringify({
          repository: { url: 'https://github.com/MyOrg/my-repo.git' },
        })
      );
      const { url } = assetDownloadUrl(dir, 'v0.3.0', 'win-amd64');
      expect(url).toContain('author-kit-api-0.3.0-win-amd64.zip');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
