import { describe, expect, it } from 'vitest';

import { categorySlug, compendiumEntryMarkdownPath } from './compendiumPaths';

describe('categorySlug', () => {
  it('slugifies spaces and punctuation', () => {
    expect(categorySlug('World Building')).toBe('world-building');
  });

  it('falls back when empty after strip', () => {
    expect(categorySlug('@@@')).toBe('entry');
  });
});

describe('compendiumEntryMarkdownPath', () => {
  it('joins workspace and slug', () => {
    const p = compendiumEntryMarkdownPath('/proj', 'Characters', 'uuid-1');
    expect(p).toMatch(/\.authorkit[/\\]characters[/\\]uuid-1\.md$/);
  });
});
