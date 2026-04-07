import * as api from './api/client';
import type { CompendiumData, StructureData } from './api/client';
import { getCharactersCategoryName, getWorldCategoryName } from './config';

export interface SceneOption {
  uuid: string;
  /** Short label for UI */
  label: string;
}

export interface CompendiumOption {
  name: string;
  id?: string;
  /** Project-relative path to sheet when id is set */
  sheetRel?: string;
}

export interface WorkshopBootstrapPayload {
  scenes: SceneOption[];
  characters: CompendiumOption[];
  world: CompendiumOption[];
}

export function collectScenesFromStructure(structure: StructureData): SceneOption[] {
  const out: SceneOption[] = [];
  const acts = structure.acts as
    | Array<{
        chapters?: Array<{
          scenes?: Array<{ name?: string; uuid?: string }>;
        }>;
      }>
    | undefined;
  if (!acts) {
    return out;
  }
  for (const act of acts) {
    for (const ch of act.chapters || []) {
      for (const sc of ch.scenes || []) {
        const uuid = sc.uuid?.trim();
        if (!uuid) {
          continue;
        }
        out.push({
          uuid,
          label: sc.name?.trim() || uuid.slice(0, 8),
        });
      }
    }
  }
  return out;
}

export async function loadWorkshopBootstrap(
  baseUrl: string,
  workspaceRoot: string
): Promise<WorkshopBootstrapPayload> {
  const structure = (await api.getStructure(baseUrl, workspaceRoot)) as StructureData;
  const scenes = collectScenesFromStructure(structure);

  const charsName = getCharactersCategoryName();
  const worldName = getWorldCategoryName();
  const comp = (await api.getCompendium(baseUrl, workspaceRoot)) as CompendiumData;

  const slugRel = (categoryName: string, id: string): string => {
    const slug = categoryName.replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-+|-+$/g, '').toLowerCase() || 'entry';
    return `.authorkit/${slug}/${id}.md`;
  };

  const mapCat = (categoryName: string): CompendiumOption[] => {
    const cat = comp.categories?.find((c) => c.name === categoryName);
    const entries = cat?.entries || [];
    return entries.map((e) => ({
      name: e.name,
      id: e.id,
      sheetRel: e.id ? slugRel(categoryName, e.id) : undefined,
    }));
  };

  return {
    scenes,
    characters: mapCat(charsName),
    world: mapCat(worldName),
  };
}
