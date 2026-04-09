/**
 * Maps Node-like `platform` / `arch` to GitHub release asset suffix
 * (must match `AUTHOR_KIT_PLATFORM` / `api/scripts/build-standalone.sh`).
 */
export function platformTripletFromNode(platform: NodeJS.Platform | string, arch: string): string {
  const p = platform;
  const a = arch;
  if (p === 'darwin') {
    return a === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
  }
  if (p === 'linux') {
    return a === 'arm64' ? 'linux-arm64' : 'linux-x64';
  }
  if (p === 'win32') {
    return 'win-amd64';
  }
  return `${p}-${a}`;
}

/** Current process mapping (see `platformTripletFromNode`). */
export function nodePlatformTriplet(): string {
  return platformTripletFromNode(process.platform, process.arch);
}
