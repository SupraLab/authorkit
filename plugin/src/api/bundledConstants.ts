import * as fs from 'fs';
import * as path from 'path';

/** Parsed `package.json` fields used for GitHub release URLs (unit-tested without disk). */
export function bundledSemverFromPackage(pkg: {
  bundledApiVersion?: string;
  version?: string;
}): string {
  const v = typeof pkg.bundledApiVersion === 'string' ? pkg.bundledApiVersion : pkg.version;
  return v || '0.1.0';
}

/** `owner/repo` from a Git remote URL (https or ssh). */
export function parseGithubRepoFromRepositoryUrl(url: string): { owner: string; repo: string } {
  const m = url.match(/github\.com[/:]([^/]+)\/([^/.]+)/i);
  if (m) {
    return { owner: m[1], repo: m[2] };
  }
  return { owner: 'SupraLab', repo: 'authorkit' };
}

export function readBundledApiSemver(extensionRoot: string): string {
  const raw = fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf8');
  const pkg = JSON.parse(raw) as { bundledApiVersion?: string; version?: string };
  return bundledSemverFromPackage(pkg);
}

export function parseGithubRepo(extensionRoot: string): { owner: string; repo: string } {
  const raw = fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf8');
  const pkg = JSON.parse(raw) as { repository?: { url?: string } };
  const url = pkg.repository?.url || '';
  return parseGithubRepoFromRepositoryUrl(url);
}
