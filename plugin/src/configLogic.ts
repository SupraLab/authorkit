/**
 * Pure configuration resolution — testable without the VS Code API.
 * Thin wrappers in `config.ts` read `vscode.workspace` and call these functions.
 */

export const DEFAULT_API_BASE_URL = 'http://127.0.0.1:8765';
export const DEFAULT_LOCAL_API_PORT = 8765;

/** Strip trailing slash; default when unset. */
export function normalizeApiBaseUrl(
  raw: string | undefined,
  fallback: string = DEFAULT_API_BASE_URL
): string {
  return (raw || fallback).replace(/\/$/, '');
}

export function normalizeLocalApiPort(n: unknown): number {
  return typeof n === 'number' && Number.isFinite(n) ? n : DEFAULT_LOCAL_API_PORT;
}

export function localApiHttpBase(port: number): string {
  return `http://127.0.0.1:${normalizeLocalApiPort(port)}`;
}

export function resolveApiBaseUrl(input: {
  startLocalApi: boolean;
  localApiPort: unknown;
  apiBaseUrl: string | undefined;
}): string {
  if (input.startLocalApi) {
    return localApiHttpBase(normalizeLocalApiPort(input.localApiPort));
  }
  return normalizeApiBaseUrl(input.apiBaseUrl);
}

export function workshopLlmOptionsFromStrings(
  activeLlmProfile: string | undefined,
  workshopModelOverride: string | undefined
): { provider?: string; model?: string } {
  const o: { provider?: string; model?: string } = {};
  const profile = activeLlmProfile?.trim();
  const model = workshopModelOverride?.trim();
  if (profile) {
    o.provider = profile;
  }
  if (model) {
    o.model = model;
  }
  return o;
}
