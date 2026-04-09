/**
 * Path helpers testable without VS Code (e.g. `~` expansion for API binary path).
 */

/** Expand a leading `~` to `home` (Unix `$HOME` / Windows user profile). */
export function expandLeadingTildePath(
  input: string,
  home: string | undefined
): string {
  const h = home ?? '';
  return input.replace(/^~(?=$|[/\\])/, h || '~');
}
