import * as path from 'path';
import * as vscode from 'vscode';

import * as api from './api/client';
import type { CompendiumData, SelectionAttachment, StructureData } from './api/client';
import { categorySlug } from './compendiumPaths';
import { collectScenesFromStructure } from './workshopBootstrap';
import { isValidUuid } from './uuidUtil';

export type { SelectionAttachment } from './api/client';

/**
 * Maps the active document to a scene, compendium sheet, or generic file for labels + insert targets.
 */
export async function resolveSelectionAttachment(
  workspaceRoot: string,
  documentUri: vscode.Uri,
  baseUrl: string
): Promise<SelectionAttachment> {
  const rel = path.relative(workspaceRoot, documentUri.fsPath).replace(/\\/g, '/');
  if (rel.startsWith('.authorkit/scenes/') && rel.endsWith('.md')) {
    const id = path.basename(rel, '.md');
    if (isValidUuid(id)) {
      const structure = (await api.getStructure(baseUrl, workspaceRoot)) as StructureData;
      const scenes = collectScenesFromStructure(structure);
      const hit = scenes.find((s) => s.uuid === id);
      const label = hit?.label?.trim() || id.slice(0, 8);
      return { label, kind: 'scene', scene_uuid: id };
    }
  }
  const m = /^\.authorkit\/([^/]+)\/([^/]+)\.md$/.exec(rel);
  if (m) {
    const slug = m[1];
    const id = m[2];
    const comp = (await api.getCompendium(baseUrl, workspaceRoot)) as CompendiumData;
    for (const cat of comp.categories || []) {
      if (categorySlug(cat.name) !== slug) {
        continue;
      }
      const entry = cat.entries?.find((e) => e.id === id);
      if (entry) {
        const name = entry.name.trim();
        return {
          label: name,
          kind: 'compendium',
          compendium_category: cat.name,
          compendium_name: name,
        };
      }
    }
  }
  const baseName = path.basename(documentUri.fsPath).replace(/\.[^/.]+$/, '');
  return {
    label: baseName || vscode.l10n.t('Editor selection'),
    kind: 'file',
  };
}
