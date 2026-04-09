import * as fs from 'fs/promises';
import * as path from 'path';
import { execFile } from 'child_process';
import { promisify } from 'util';
import * as vscode from 'vscode';

import { parseGithubRepo } from './bundledConstants';

const execFileAsync = promisify(execFile);

export type InstalledManifest = {
  tag: string;
  semver: string;
  triplet: string;
  executablePath: string;
  downloadedAt: string;
};

export function releaseAssetName(semver: string, triplet: string): string {
  return `author-kit-api-${semver}-${triplet}.zip`;
}

export function releaseTagFromSemver(semver: string): string {
  return semver.startsWith('v') ? semver : `v${semver}`;
}

export function assetDownloadUrl(
  extensionRoot: string,
  semverBare: string,
  triplet: string
): { url: string; tag: string } {
  const { owner, repo } = parseGithubRepo(extensionRoot);
  const bare = semverBare.replace(/^v/, '');
  const tag = releaseTagFromSemver(bare);
  const name = releaseAssetName(bare, triplet);
  const url = `https://github.com/${owner}/${repo}/releases/download/${tag}/${name}`;
  return { url, tag };
}

async function extractZipArchive(zipPath: string, destDir: string, log: vscode.OutputChannel): Promise<void> {
  await fs.mkdir(destDir, { recursive: true });
  try {
    await execFileAsync('tar', ['-xf', zipPath, '-C', destDir], { maxBuffer: 50 * 1024 * 1024 });
  } catch (e) {
    log.appendLine(`tar extract failed: ${e}`);
    throw e;
  }
}

function executableInsideBundle(bundleRoot: string): string {
  const base = path.join(bundleRoot, 'author-kit-api');
  const name = process.platform === 'win32' ? 'author-kit-api.exe' : 'author-kit-api';
  return path.join(base, name);
}

export async function ensureBundledApiOnDisk(
  context: vscode.ExtensionContext,
  semver: string,
  triplet: string,
  log: vscode.OutputChannel,
  options?: { force?: boolean }
): Promise<InstalledManifest> {
  const semverBare = semver.replace(/^v/, '');
  const tag = releaseTagFromSemver(semverBare);
  const root = path.join(context.globalStorageUri.fsPath, 'author-kit-api', tag);
  const manifestPath = path.join(root, 'installed.json');
  const extractDir = path.join(root, 'extracted');
  const exePath = executableInsideBundle(extractDir);

  if (!options?.force) {
    try {
      const prev = JSON.parse(await fs.readFile(manifestPath, 'utf8')) as InstalledManifest;
      try {
        await fs.access(prev.executablePath);
        if (prev.triplet === triplet && prev.semver === semverBare) {
          return prev;
        }
      } catch {
        // missing binary — re-download
      }
    } catch {
      // no manifest
    }
  }

  await fs.rm(root, { recursive: true, force: true }).catch(() => undefined);
  await fs.mkdir(root, { recursive: true });

  const { url } = assetDownloadUrl(context.extensionPath, semverBare, triplet);
  log.appendLine(`Downloading AuthorKit API: ${url}`);
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Download failed (${res.status}): ${url}`);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  const zipPath = path.join(root, 'bundle.zip');
  await fs.writeFile(zipPath, buf);

  await extractZipArchive(zipPath, extractDir, log);
  await fs.rm(zipPath, { force: true }).catch(() => undefined);

  try {
    if (process.platform !== 'win32') {
      await fs.chmod(exePath, 0o755);
    }
  } catch {
    // best effort
  }

  await fs.access(exePath);

  const manifest: InstalledManifest = {
    tag,
    semver: semverBare,
    triplet,
    executablePath: exePath,
    downloadedAt: new Date().toISOString(),
  };
  await fs.writeFile(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');
  log.appendLine(`AuthorKit API installed at ${exePath}`);
  return manifest;
}
