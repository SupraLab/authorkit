import * as fs from 'fs/promises';
import * as path from 'path';
import { spawn, type ChildProcess } from 'child_process';
import * as vscode from 'vscode';

import * as api from './client';
import { readBundledApiSemver } from './bundledConstants';
import { ensureBundledApiOnDisk } from './downloadBundledApi';
import { nodePlatformTriplet } from './platformTriplet';
import {
  getGithubApiReleaseTag,
  getLocalApiBinaryPath,
  getLocalApiPort,
  getStartLocalApi,
  localApiBaseUrl,
} from '../config';
import { expandLeadingTildePath } from '../pathUtils';
import { effectiveApiReleaseSemver } from './releaseSemverLogic';

let child: ChildProcess | undefined;
let output: vscode.OutputChannel | undefined;

function logChannel(): vscode.OutputChannel {
  if (!output) {
    output = vscode.window.createOutputChannel('AuthorKit API');
  }
  return output;
}

async function resolveExecutable(
  context: vscode.ExtensionContext,
  semver: string,
  triplet: string
): Promise<string> {
  const manual = getLocalApiBinaryPath().trim();
  if (manual) {
    const expanded = expandLeadingTildePath(manual, process.env.HOME || process.env.USERPROFILE);
    const p = path.resolve(expanded);
    await fs.access(p);
    logChannel().appendLine(`Using local API binary: ${p}`);
    return p;
  }
  const m = await ensureBundledApiOnDisk(context, semver, triplet, logChannel());
  return m.executablePath;
}

async function waitForHealthy(maxMs: number): Promise<boolean> {
  const base = localApiBaseUrl();
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    try {
      await api.health(base);
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 300));
    }
  }
  return false;
}

export function stopLocalApiProcess(): void {
  if (child && !child.killed) {
    child.kill();
  }
  child = undefined;
}

function effectiveApiSemver(context: vscode.ExtensionContext): string {
  return effectiveApiReleaseSemver(getGithubApiReleaseTag(), readBundledApiSemver(context.extensionPath));
}

/**
 * Ensures the standalone API process is running when **Start local API** is enabled.
 */
export async function ensureLocalApiRunning(context: vscode.ExtensionContext): Promise<void> {
  if (!getStartLocalApi()) {
    return;
  }

  const triplet = nodePlatformTriplet();
  const semver = effectiveApiSemver(context);
  const log = logChannel();

  if (await waitForHealthy(1500)) {
    return;
  }

  stopLocalApiProcess();

  let exe: string;
  try {
    exe = await resolveExecutable(context, semver, triplet);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log.appendLine(`Failed to resolve API binary: ${msg}`);
    void vscode.window.showErrorMessage(
      vscode.l10n.t(
        'AuthorKit: could not install or find the API binary ({0}). Check your network or set a local path in settings.',
        msg
      )
    );
    return;
  }

  const port = getLocalApiPort();
  const env = {
    ...process.env,
    AUTHORKIT_HOST: '127.0.0.1',
    AUTHORKIT_PORT: String(port),
  };
  const cwd = path.dirname(exe);
  log.appendLine(`Spawning: ${exe} (cwd ${cwd}, port ${port})`);

  child = spawn(exe, [], { env, cwd, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout?.on('data', (d: Buffer) => log.append(d.toString()));
  child.stderr?.on('data', (d: Buffer) => log.append(d.toString()));
  child.on('exit', (code) => {
    log.appendLine(`AuthorKit API exited with code ${code ?? 'unknown'}`);
    child = undefined;
  });

  if (!(await waitForHealthy(45_000))) {
    void vscode.window.showErrorMessage(
      vscode.l10n.t(
        'AuthorKit: local API did not respond on {0}. See output "AuthorKit API".',
        localApiBaseUrl()
      )
    );
  }
}

export async function redownloadBundledApi(context: vscode.ExtensionContext): Promise<void> {
  const triplet = nodePlatformTriplet();
  const semver = effectiveApiSemver(context);
  const log = logChannel();
  stopLocalApiProcess();
  try {
    await ensureBundledApiOnDisk(context, semver, triplet, log, { force: true });
    if (getStartLocalApi()) {
      void vscode.window.showInformationMessage(
        vscode.l10n.t('AuthorKit API bundle downloaded; starting local process…')
      );
      await ensureLocalApiRunning(context);
    } else {
      void vscode.window.showInformationMessage(
        vscode.l10n.t('AuthorKit API bundle downloaded. Enable "Start local API" to run it from the extension.')
      );
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log.appendLine(`Re-download failed: ${msg}`);
    void vscode.window.showErrorMessage(vscode.l10n.t('AuthorKit: re-download failed: {0}', msg));
  }
}

export function registerLocalApiOnActivate(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (
        e.affectsConfiguration('authorkit.startLocalApi') ||
        e.affectsConfiguration('authorkit.localApiPort') ||
        e.affectsConfiguration('authorkit.localApiBinaryPath') ||
        e.affectsConfiguration('authorkit.githubApiReleaseTag')
      ) {
        stopLocalApiProcess();
        if (getStartLocalApi()) {
          void ensureLocalApiRunning(context);
        }
      }
    })
  );
}
