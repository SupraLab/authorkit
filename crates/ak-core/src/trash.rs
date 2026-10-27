//! Restorable trash for scenes, chapters, acts, and notes.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::fsutil::{self, write_atomic};
use crate::manuscript::{Act, Chapter, Manuscript, Scene, SceneMeta};
use crate::notes::{Note, NoteFrontmatter, NotesLibrary};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrashEntryKind {
    Scene {
        meta: SceneMeta,
        body: String,
        act_id: String,
        chapter_id: String,
        /// Relative path under project root, for restore filename preference.
        relative_path: String,
        /// Index position among siblings (scene_ids index).
        index_pos: usize,
    },
    Chapter {
        act_id: String,
        chapter: Chapter,
        scenes: Vec<TrashedScenePayload>,
        index_pos: usize,
    },
    Act {
        act: Act,
        scenes: Vec<TrashedScenePayload>,
        index_pos: usize,
    },
    Note {
        meta: NoteFrontmatter,
        body: String,
        relative_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashedScenePayload {
    pub meta: SceneMeta,
    pub body: String,
    pub relative_path: String,
    pub act_id: String,
    pub chapter_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashEntry {
    pub id: String,
    pub created_at: String,
    pub label: String,
    #[serde(flatten)]
    pub kind: TrashEntryKind,
}

fn trash_dir(root: &Path) -> PathBuf {
    fsutil::meta_root(root).join("trash")
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn write_entry(root: &Path, entry: &TrashEntry) -> Result<()> {
    fsutil::ensure_meta_dirs(root)?;
    let path = trash_dir(root).join(format!("{}.json", entry.id));
    let raw = serde_json::to_vec_pretty(entry)?;
    write_atomic(&path, raw)
}

fn scene_payload(scene: &Scene, root: &Path) -> TrashedScenePayload {
    let relative = scene
        .path
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| {
            format!(
                "manuscript/scenes/{}",
                scene
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("scene.md")
            )
        });
    TrashedScenePayload {
        meta: scene.meta.clone(),
        body: scene.body.clone(),
        relative_path: relative,
        act_id: scene.act_id.clone(),
        chapter_id: scene.chapter_id.clone(),
    }
}

fn write_scene_file(root: &Path, payload: &TrashedScenePayload) -> Result<PathBuf> {
    let path = root.join(&payload.relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Prefer original path; if taken, use id-based name.
    let path = if path.exists() {
        let dir = root.join("manuscript/scenes");
        std::fs::create_dir_all(&dir)?;
        dir.join(format!(
            "{}-{}.md",
            crate::notes::slugify(&payload.meta.title),
            &payload.meta.id[..8.min(payload.meta.id.len())]
        ))
    } else {
        path
    };
    let json = serde_json::to_string_pretty(&payload.meta)?;
    let content = format!("---\n{json}\n---\n\n{}", payload.body);
    write_atomic(&path, content)?;
    Ok(path)
}

impl Manuscript {
    /// Move a scene to trash and remove it from the live manuscript.
    pub fn trash_scene(&mut self, scene_id: &str, root: &Path) -> Result<TrashEntry> {
        let scene = self
            .get_scene(scene_id)
            .ok_or_else(|| Error::SceneNotFound(scene_id.into()))?
            .clone();

        let mut index_pos = 0usize;
        for act in &self.index.acts {
            for ch in &act.chapters {
                if let Some(pos) = ch.scene_ids.iter().position(|id| id == scene_id) {
                    index_pos = pos;
                }
            }
        }

        let entry = TrashEntry {
            id: Uuid::new_v4().to_string(),
            created_at: now_iso(),
            label: format!("Scène « {} »", scene.meta.title),
            kind: TrashEntryKind::Scene {
                meta: scene.meta.clone(),
                body: scene.body.clone(),
                act_id: scene.act_id.clone(),
                chapter_id: scene.chapter_id.clone(),
                relative_path: scene_payload(&scene, root).relative_path,
                index_pos,
            },
        };
        write_entry(root, &entry)?;

        // Soft-remove: drop from index/memory, then delete file only after trash written.
        for act in &mut self.index.acts {
            for ch in &mut act.chapters {
                ch.scene_ids.retain(|id| id != scene_id);
            }
        }
        self.scenes.retain(|s| s.meta.id != scene_id);
        if scene.path.exists() {
            std::fs::remove_file(&scene.path)?;
        }
        self.save_index(root)?;
        Ok(entry)
    }

    pub fn trash_chapter(
        &mut self,
        act_id: &str,
        chapter_id: &str,
        root: &Path,
    ) -> Result<TrashEntry> {
        let (chapter, index_pos) = {
            let act = self
                .index
                .acts
                .iter()
                .find(|a| a.id == act_id)
                .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
            let pos = act
                .chapters
                .iter()
                .position(|c| c.id == chapter_id)
                .ok_or_else(|| Error::Msg("Chapitre introuvable".into()))?;
            (act.chapters[pos].clone(), pos)
        };

        let scenes: Vec<TrashedScenePayload> = chapter
            .scene_ids
            .iter()
            .filter_map(|sid| self.get_scene(sid).map(|s| scene_payload(s, root)))
            .collect();

        let entry = TrashEntry {
            id: Uuid::new_v4().to_string(),
            created_at: now_iso(),
            label: format!("Chapitre « {} »", chapter.title),
            kind: TrashEntryKind::Chapter {
                act_id: act_id.to_string(),
                chapter: chapter.clone(),
                scenes,
                index_pos,
            },
        };
        write_entry(root, &entry)?;

        // Remove scenes + chapter from live state
        for sid in &chapter.scene_ids {
            if let Some(scene) = self.scenes.iter().find(|s| s.meta.id == *sid) {
                if scene.path.exists() {
                    std::fs::remove_file(&scene.path)?;
                }
            }
            self.scenes.retain(|s| s.meta.id != *sid);
        }
        if let Some(act) = self.index.acts.iter_mut().find(|a| a.id == act_id) {
            act.chapters.retain(|c| c.id != chapter_id);
        }
        self.save_index(root)?;
        Ok(entry)
    }

    pub fn trash_act(&mut self, act_id: &str, root: &Path) -> Result<TrashEntry> {
        let (act, index_pos) = {
            let pos = self
                .index
                .acts
                .iter()
                .position(|a| a.id == act_id)
                .ok_or_else(|| Error::Msg(format!("Acte introuvable: {act_id}")))?;
            (self.index.acts[pos].clone(), pos)
        };

        let mut scenes = Vec::new();
        for ch in &act.chapters {
            for sid in &ch.scene_ids {
                if let Some(s) = self.get_scene(sid) {
                    scenes.push(scene_payload(s, root));
                }
            }
        }

        let entry = TrashEntry {
            id: Uuid::new_v4().to_string(),
            created_at: now_iso(),
            label: format!("Acte « {} »", act.title),
            kind: TrashEntryKind::Act {
                act: act.clone(),
                scenes,
                index_pos,
            },
        };
        write_entry(root, &entry)?;

        for ch in &act.chapters {
            for sid in &ch.scene_ids {
                if let Some(scene) = self.scenes.iter().find(|s| s.meta.id == *sid) {
                    if scene.path.exists() {
                        std::fs::remove_file(&scene.path)?;
                    }
                }
                self.scenes.retain(|s| s.meta.id != *sid);
            }
        }
        self.index.acts.retain(|a| a.id != act_id);
        self.save_index(root)?;
        Ok(entry)
    }

    /// Restore a trash entry that belongs to the manuscript.
    pub fn restore_from_trash(&mut self, root: &Path, entry: &TrashEntry) -> Result<()> {
        match &entry.kind {
            TrashEntryKind::Scene {
                meta,
                body,
                act_id,
                chapter_id,
                relative_path,
                index_pos,
            } => {
                let payload = TrashedScenePayload {
                    meta: meta.clone(),
                    body: body.clone(),
                    relative_path: relative_path.clone(),
                    act_id: act_id.clone(),
                    chapter_id: chapter_id.clone(),
                };
                let path = write_scene_file(root, &payload)?;
                let scene = Scene {
                    meta: meta.clone(),
                    body: body.clone(),
                    path,
                    act_id: act_id.clone(),
                    chapter_id: chapter_id.clone(),
                };
                // Ensure act/chapter exist
                if self.index.acts.iter().all(|a| a.id != *act_id) {
                    self.index.acts.push(Act {
                        id: act_id.clone(),
                        title: "Acte restauré".into(),
                        chapters: vec![Chapter {
                            id: chapter_id.clone(),
                            title: "Chapitre restauré".into(),
                            scene_ids: vec![meta.id.clone()],
                        }],
                    });
                } else if let Some(act) = self.index.acts.iter_mut().find(|a| a.id == *act_id) {
                    if let Some(ch) = act.chapters.iter_mut().find(|c| c.id == *chapter_id) {
                        let pos = (*index_pos).min(ch.scene_ids.len());
                        if !ch.scene_ids.contains(&meta.id) {
                            ch.scene_ids.insert(pos, meta.id.clone());
                        }
                    } else {
                        act.chapters.push(Chapter {
                            id: chapter_id.clone(),
                            title: "Chapitre restauré".into(),
                            scene_ids: vec![meta.id.clone()],
                        });
                    }
                }
                self.scenes.retain(|s| s.meta.id != meta.id);
                self.scenes.push(scene);
                self.save_index(root)?;
            }
            TrashEntryKind::Chapter {
                act_id,
                chapter,
                scenes,
                index_pos,
            } => {
                for payload in scenes {
                    let path = write_scene_file(root, payload)?;
                    self.scenes.retain(|s| s.meta.id != payload.meta.id);
                    self.scenes.push(Scene {
                        meta: payload.meta.clone(),
                        body: payload.body.clone(),
                        path,
                        act_id: payload.act_id.clone(),
                        chapter_id: payload.chapter_id.clone(),
                    });
                }
                if let Some(act) = self.index.acts.iter_mut().find(|a| a.id == *act_id) {
                    let pos = (*index_pos).min(act.chapters.len());
                    if !act.chapters.iter().any(|c| c.id == chapter.id) {
                        act.chapters.insert(pos, chapter.clone());
                    }
                } else {
                    self.index.acts.push(Act {
                        id: act_id.clone(),
                        title: "Acte restauré".into(),
                        chapters: vec![chapter.clone()],
                    });
                }
                self.save_index(root)?;
            }
            TrashEntryKind::Act {
                act,
                scenes,
                index_pos,
            } => {
                for payload in scenes {
                    let path = write_scene_file(root, payload)?;
                    self.scenes.retain(|s| s.meta.id != payload.meta.id);
                    self.scenes.push(Scene {
                        meta: payload.meta.clone(),
                        body: payload.body.clone(),
                        path,
                        act_id: payload.act_id.clone(),
                        chapter_id: payload.chapter_id.clone(),
                    });
                }
                let pos = (*index_pos).min(self.index.acts.len());
                if !self.index.acts.iter().any(|a| a.id == act.id) {
                    self.index.acts.insert(pos, act.clone());
                }
                self.save_index(root)?;
            }
            TrashEntryKind::Note { .. } => {
                return Err(Error::Msg(
                    "Cette entrée de corbeille est une note ; utilisez NotesLibrary::restore_from_trash"
                        .into(),
                ));
            }
        }
        Ok(())
    }
}

impl NotesLibrary {
    pub fn trash_note(&mut self, id: &str, root: &Path) -> Result<TrashEntry> {
        let pos = self
            .notes
            .iter()
            .position(|n| n.meta.id == id)
            .ok_or_else(|| Error::NoteNotFound(id.into()))?;
        let note = self.notes.remove(pos);
        let relative = note
            .path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| {
                format!(
                    "notes/{}/{}",
                    note.meta.kind.dir_name(),
                    note.path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("note.md")
                )
            });
        let entry = TrashEntry {
            id: Uuid::new_v4().to_string(),
            created_at: now_iso(),
            label: format!("Fiche « {} »", note.meta.name),
            kind: TrashEntryKind::Note {
                meta: note.meta.clone(),
                body: note.body.clone(),
                relative_path: relative,
            },
        };
        write_entry(root, &entry)?;
        if note.path.exists() {
            std::fs::remove_file(&note.path)?;
        }
        Ok(entry)
    }

    pub fn restore_from_trash(&mut self, root: &Path, entry: &TrashEntry) -> Result<()> {
        let TrashEntryKind::Note {
            meta,
            body,
            relative_path,
        } = &entry.kind
        else {
            return Err(Error::Msg(
                "Cette entrée de corbeille n'est pas une note".into(),
            ));
        };
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let path = if path.exists() {
            let dir = root.join("notes").join(meta.kind.dir_name());
            std::fs::create_dir_all(&dir)?;
            dir.join(format!(
                "{}-{}.md",
                crate::notes::slugify(&meta.name),
                &meta.id[..8.min(meta.id.len())]
            ))
        } else {
            path
        };
        let json = serde_json::to_string_pretty(meta)?;
        let content = format!("---\n{json}\n---\n\n{body}");
        write_atomic(&path, content)?;
        self.notes.retain(|n| n.meta.id != meta.id);
        self.notes.push(Note {
            meta: meta.clone(),
            body: body.clone(),
            path,
        });
        self.notes.sort_by_key(|a| a.meta.name.to_lowercase());
        Ok(())
    }
}

/// List trash entries for a project (newest first).
pub fn list_trash(root: &Path) -> Result<Vec<TrashEntry>> {
    let dir = trash_dir(root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)?;
        match serde_json::from_str::<TrashEntry>(&raw) {
            Ok(e) => out.push(e),
            Err(_) => continue,
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn load_trash_entry(root: &Path, trash_id: &str) -> Result<TrashEntry> {
    let path = trash_dir(root).join(format!("{trash_id}.json"));
    if !path.exists() {
        return Err(Error::Msg(format!(
            "Entrée corbeille introuvable: {trash_id}"
        )));
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn purge_trash_entry(root: &Path, trash_id: &str) -> Result<()> {
    let path = trash_dir(root).join(format!("{trash_id}.json"));
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Restore any trash entry and remove it from the trash folder.
pub fn restore_trash_entry(
    project: &mut crate::project::Project,
    trash_id: &str,
) -> Result<TrashEntry> {
    let entry = load_trash_entry(&project.root, trash_id)?;
    match &entry.kind {
        TrashEntryKind::Note { .. } => {
            project.notes.restore_from_trash(&project.root, &entry)?;
        }
        _ => {
            project
                .manuscript
                .restore_from_trash(&project.root, &entry)?;
        }
    }
    purge_trash_entry(&project.root, trash_id)?;
    Ok(entry)
}
