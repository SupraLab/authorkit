use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::fsutil::write_atomic;
use crate::notes::slugify;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SceneStatus {
    #[default]
    ToWrite,
    Draft,
    ToRevise,
    Done,
}

impl SceneStatus {
    pub fn label_fr(self) -> &'static str {
        match self {
            Self::ToWrite => "À écrire",
            Self::Draft => "Brouillon",
            Self::ToRevise => "À réviser",
            Self::Done => "Terminé",
        }
    }

    pub fn all() -> [SceneStatus; 4] {
        [Self::ToWrite, Self::Draft, Self::ToRevise, Self::Done]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneMeta {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub synopsis: String,
    #[serde(default)]
    pub status: SceneStatus,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub meta: SceneMeta,
    pub body: String,
    pub path: PathBuf,
    pub act_id: String,
    pub chapter_id: String,
}

impl Scene {
    pub fn word_count(&self) -> usize {
        self.body.split_whitespace().count()
    }

    pub fn as_context_block(&self) -> String {
        format!("### Scène: {}\n{}", self.meta.title, self.body.trim())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub scene_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Act {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManuscriptIndex {
    pub acts: Vec<Act>,
}

#[derive(Debug, Clone, Default)]
pub struct Manuscript {
    pub index: ManuscriptIndex,
    pub scenes: Vec<Scene>,
}

impl Manuscript {
    pub fn load(root: &Path) -> Result<Self> {
        let index_path = root.join("manuscript/index.json");
        let index: ManuscriptIndex = if index_path.exists() {
            let raw = std::fs::read_to_string(&index_path)?;
            serde_json::from_str(&raw)?
        } else {
            ManuscriptIndex::default()
        };

        let mut scenes = Vec::new();
        let mut load_errors: Vec<(PathBuf, String)> = Vec::new();
        let scenes_dir = root.join("manuscript/scenes");
        if scenes_dir.exists() {
            for entry in std::fs::read_dir(&scenes_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                match load_scene(&path) {
                    Ok(scene) => scenes.push(scene),
                    Err(e) => load_errors.push((path, e.to_string())),
                }
            }
        }
        // Stash load errors on a thread-local? Prefer returning via Project::open diagnostics.
        // Attach as empty act ownership; diagnostics module re-reads unreadable files.
        let _ = load_errors;

        // Resolve act/chapter ownership from index
        for act in &index.acts {
            for ch in &act.chapters {
                for sid in &ch.scene_ids {
                    if let Some(s) = scenes.iter_mut().find(|s| s.meta.id == *sid) {
                        s.act_id = act.id.clone();
                        s.chapter_id = ch.id.clone();
                    }
                }
            }
        }

        Ok(Self { index, scenes })
    }

    pub fn save_index(&self, root: &Path) -> Result<()> {
        let path = root.join("manuscript/index.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.index)?;
        write_atomic(&path, raw)
    }

    pub fn get_scene(&self, id: &str) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.meta.id == id)
    }

    pub fn get_scene_mut(&mut self, id: &str) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.meta.id == id)
    }

    pub fn save_scene(&self, scene: &Scene) -> Result<()> {
        write_scene(&scene.path, &scene.meta, &scene.body)
    }

    pub fn add_act(&mut self, title: &str) -> String {
        let id = Uuid::new_v4().to_string();
        self.index.acts.push(Act {
            id: id.clone(),
            title: title.to_string(),
            chapters: Vec::new(),
        });
        id
    }

    pub fn add_chapter(&mut self, act_id: &str, title: &str) -> Result<String> {
        let act = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == act_id)
            .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
        let id = Uuid::new_v4().to_string();
        act.chapters.push(Chapter {
            id: id.clone(),
            title: title.to_string(),
            scene_ids: Vec::new(),
        });
        Ok(id)
    }

    pub fn add_scene(
        &mut self,
        root: &Path,
        act_id: &str,
        chapter_id: &str,
        title: &str,
    ) -> Result<&Scene> {
        let id = Uuid::new_v4().to_string();
        let dir = root.join("manuscript/scenes");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}-{}.md", slugify(title), &id[..8]));
        let meta = SceneMeta {
            id: id.clone(),
            title: title.to_string(),
            links: Vec::new(),
            synopsis: String::new(),
            status: SceneStatus::ToWrite,
        };
        let body = String::new();
        write_scene(&path, &meta, &body)?;

        let chapter = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == act_id)
            .and_then(|a| a.chapters.iter_mut().find(|c| c.id == chapter_id))
            .ok_or_else(|| Error::Msg("Chapitre introuvable".into()))?;
        chapter.scene_ids.push(id.clone());

        self.scenes.push(Scene {
            meta,
            body,
            path,
            act_id: act_id.to_string(),
            chapter_id: chapter_id.to_string(),
        });
        self.save_index(root)?;
        Ok(self.get_scene(&id).expect("just inserted"))
    }

    pub fn ensure_default_structure(&mut self, root: &Path) -> Result<()> {
        if self.index.acts.is_empty() {
            let act_id = self.add_act("Acte I");
            let ch_id = self.add_chapter(&act_id, "Chapitre 1")?;
            self.add_scene(root, &act_id, &ch_id, "Scène d'ouverture")?;
            self.save_index(root)?;
        }
        Ok(())
    }

    pub fn total_words(&self) -> usize {
        self.scenes.iter().map(|s| s.word_count()).sum()
    }

    pub fn link_note(&mut self, scene_id: &str, note_id: &str, root: &Path) -> Result<()> {
        let scene = self
            .get_scene_mut(scene_id)
            .ok_or_else(|| Error::SceneNotFound(scene_id.into()))?;
        if !scene.meta.links.contains(&note_id.to_string()) {
            scene.meta.links.push(note_id.to_string());
        }
        let scene = scene.clone();
        self.save_scene(&scene)?;
        self.save_index(root)?;
        Ok(())
    }

    pub fn unlink_note(&mut self, scene_id: &str, note_id: &str, root: &Path) -> Result<()> {
        let scene = self
            .get_scene_mut(scene_id)
            .ok_or_else(|| Error::SceneNotFound(scene_id.into()))?;
        scene.meta.links.retain(|id| id != note_id);
        let scene = scene.clone();
        self.save_scene(&scene)?;
        let _ = root;
        Ok(())
    }

    pub fn rename_act(&mut self, act_id: &str, title: &str, root: &Path) -> Result<()> {
        let act = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == act_id)
            .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
        act.title = title.to_string();
        self.save_index(root)
    }

    pub fn rename_chapter(
        &mut self,
        act_id: &str,
        chapter_id: &str,
        title: &str,
        root: &Path,
    ) -> Result<()> {
        let ch = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == act_id)
            .and_then(|a| a.chapters.iter_mut().find(|c| c.id == chapter_id))
            .ok_or_else(|| Error::Msg("Chapitre introuvable".into()))?;
        ch.title = title.to_string();
        self.save_index(root)
    }

    pub fn rename_scene(&mut self, scene_id: &str, title: &str) -> Result<()> {
        let scene = self
            .get_scene_mut(scene_id)
            .ok_or_else(|| Error::SceneNotFound(scene_id.into()))?;
        scene.meta.title = title.to_string();
        let scene = scene.clone();
        self.save_scene(&scene)
    }

    /// Soft-delete via trash (preferred). Hard delete remains for tests/migration.
    pub fn delete_scene(&mut self, scene_id: &str, root: &Path) -> Result<()> {
        self.trash_scene(scene_id, root)?;
        Ok(())
    }

    pub fn delete_chapter(&mut self, act_id: &str, chapter_id: &str, root: &Path) -> Result<()> {
        self.trash_chapter(act_id, chapter_id, root)?;
        Ok(())
    }

    pub fn delete_act(&mut self, act_id: &str, root: &Path) -> Result<()> {
        self.trash_act(act_id, root)?;
        Ok(())
    }

    /// Move a scene within its chapter, or to another chapter.
    pub fn move_scene(
        &mut self,
        scene_id: &str,
        to_act_id: &str,
        to_chapter_id: &str,
        to_index: usize,
        root: &Path,
    ) -> Result<()> {
        // Remove from all chapters
        for act in &mut self.index.acts {
            for ch in &mut act.chapters {
                ch.scene_ids.retain(|id| id != scene_id);
            }
        }
        let chapter = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == to_act_id)
            .and_then(|a| a.chapters.iter_mut().find(|c| c.id == to_chapter_id))
            .ok_or_else(|| Error::Msg("Chapitre de destination introuvable".into()))?;
        let pos = to_index.min(chapter.scene_ids.len());
        chapter.scene_ids.insert(pos, scene_id.to_string());

        if let Some(scene) = self.get_scene_mut(scene_id) {
            scene.act_id = to_act_id.to_string();
            scene.chapter_id = to_chapter_id.to_string();
        } else {
            return Err(Error::SceneNotFound(scene_id.into()));
        }
        self.save_index(root)
    }

    pub fn move_chapter(
        &mut self,
        act_id: &str,
        chapter_id: &str,
        to_act_id: &str,
        to_index: usize,
        root: &Path,
    ) -> Result<()> {
        let chapter = {
            let act = self
                .index
                .acts
                .iter_mut()
                .find(|a| a.id == act_id)
                .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
            let pos = act
                .chapters
                .iter()
                .position(|c| c.id == chapter_id)
                .ok_or_else(|| Error::Msg("Chapitre introuvable".into()))?;
            act.chapters.remove(pos)
        };
        let dest = self
            .index
            .acts
            .iter_mut()
            .find(|a| a.id == to_act_id)
            .ok_or_else(|| Error::Msg(format!("Acte de destination introuvable: {to_act_id}")))?;
        let pos = to_index.min(dest.chapters.len());
        let scene_ids = chapter.scene_ids.clone();
        dest.chapters.insert(pos, chapter);
        for sid in scene_ids {
            if let Some(s) = self.get_scene_mut(&sid) {
                s.act_id = to_act_id.to_string();
                s.chapter_id = chapter_id.to_string();
            }
        }
        self.save_index(root)
    }

    pub fn move_act(&mut self, act_id: &str, to_index: usize, root: &Path) -> Result<()> {
        let pos = self
            .index
            .acts
            .iter()
            .position(|a| a.id == act_id)
            .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
        let act = self.index.acts.remove(pos);
        let dest = to_index.min(self.index.acts.len());
        self.index.acts.insert(dest, act);
        self.save_index(root)
    }

    /// Flat ordered list of scenes following the index.
    pub fn ordered_scenes(&self) -> Vec<&Scene> {
        let mut out = Vec::new();
        for act in &self.index.acts {
            for ch in &act.chapters {
                for sid in &ch.scene_ids {
                    if let Some(s) = self.get_scene(sid) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }
}

fn load_scene(path: &Path) -> Result<Scene> {
    let raw = std::fs::read_to_string(path)?;
    let (meta, body) = parse_scene_frontmatter(&raw)?;
    Ok(Scene {
        meta,
        body,
        path: path.to_path_buf(),
        act_id: String::new(),
        chapter_id: String::new(),
    })
}

fn write_scene(path: &Path, meta: &SceneMeta, body: &str) -> Result<()> {
    let json = serde_json::to_string_pretty(meta)?;
    let content = format!("---\n{json}\n---\n\n{body}");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(path, content)
}

/// Parse scene frontmatter. Invalid identity is reported as an error (no silent UUID swap).
fn parse_scene_frontmatter(raw: &str) -> Result<(SceneMeta, String)> {
    let raw = raw.trim_start_matches('\u{feff}');
    if let Some(rest) = raw.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let json = rest[..end].trim();
            let body = rest[end + 4..].trim_start_matches('\n').to_string();
            let meta: SceneMeta = serde_json::from_str(json)
                .map_err(|e| Error::Msg(format!("Frontmatter de scène invalide : {e}")))?;
            if meta.id.trim().is_empty() {
                return Err(Error::Msg(
                    "Identifiant de scène manquant ou vide dans le frontmatter".into(),
                ));
            }
            return Ok((meta, body));
        }
    }
    Err(Error::Msg(
        "Frontmatter de scène manquant (attendu --- … ---)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scene(title: &str, body: &str) -> Scene {
        Scene {
            meta: SceneMeta {
                id: "scene-1".into(),
                title: title.into(),
                links: Vec::new(),
                synopsis: String::new(),
                status: SceneStatus::Draft,
            },
            body: body.into(),
            path: PathBuf::from("manuscript/scenes/x.md"),
            act_id: "act-1".into(),
            chapter_id: "ch-1".into(),
        }
    }

    #[test]
    fn scene_word_count() {
        assert_eq!(sample_scene("T", "").word_count(), 0);
        assert_eq!(sample_scene("T", "one two three").word_count(), 3);
        assert_eq!(sample_scene("T", "  a   b\nc  ").word_count(), 3);
    }

    #[test]
    fn scene_as_context_block() {
        let scene = sample_scene("Ouverture", "  Il pleuvait.\n");
        assert_eq!(
            scene.as_context_block(),
            "### Scène: Ouverture\nIl pleuvait."
        );
    }

    #[test]
    fn scene_status_label_fr_all_variants() {
        assert_eq!(SceneStatus::ToWrite.label_fr(), "À écrire");
        assert_eq!(SceneStatus::Draft.label_fr(), "Brouillon");
        assert_eq!(SceneStatus::ToRevise.label_fr(), "À réviser");
        assert_eq!(SceneStatus::Done.label_fr(), "Terminé");
        assert_eq!(
            SceneStatus::all(),
            [
                SceneStatus::ToWrite,
                SceneStatus::Draft,
                SceneStatus::ToRevise,
                SceneStatus::Done
            ]
        );
    }
}
