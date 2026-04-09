/**
 * Which API semver to download: GitHub tag override vs bundled package version.
 */

export function effectiveApiReleaseSemver(githubApiReleaseTag: string, bundledSemver: string): string {
  const t = githubApiReleaseTag.trim();
  if (t) {
    return t.replace(/^v/, '');
  }
  return bundledSemver;
}
