import * as path from 'path';

/** Matches `category_slug` in `api/author_kit/core/paths.py`. */
export function categorySlug(category: string): string {
  const s = category.replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase();
  return s || 'entry';
}

/** `.authorkit/<slug>/<uuid>.md` */
export function compendiumEntryMarkdownPath(
  workspaceRoot: string,
  categoryName: string,
  entryId: string
): string {
  return path.join(workspaceRoot, '.authorkit', categorySlug(categoryName), `${entryId}.md`);
}
