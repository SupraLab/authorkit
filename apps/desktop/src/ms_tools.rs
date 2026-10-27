//! Manuscript tools: search, import preview, read-through.

use ak_core::{NoteKind, Project};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub kind: SearchHitKind,
    pub id: String,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchHitKind {
    Scene,
    Note,
}

pub fn search_project(project: &Project, query: &str) -> Vec<SearchHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for scene in project.manuscript.ordered_scenes() {
        let title = scene.meta.title.to_lowercase();
        let body = scene.body.to_lowercase();
        let syn = scene.meta.synopsis.to_lowercase();
        if title.contains(&q) || body.contains(&q) || syn.contains(&q) {
            hits.push(SearchHit {
                kind: SearchHitKind::Scene,
                id: scene.meta.id.clone(),
                title: scene.meta.title.clone(),
                snippet: snippet_around(&scene.body, &q),
            });
        }
    }
    for note in &project.notes.notes {
        let name = note.meta.name.to_lowercase();
        let body = note.body.to_lowercase();
        if name.contains(&q) || body.contains(&q) {
            hits.push(SearchHit {
                kind: SearchHitKind::Note,
                id: note.meta.id.clone(),
                title: format!(
                    "{} · {}",
                    note.meta.kind.label_fr_singular(),
                    note.meta.name
                ),
                snippet: snippet_around(&note.body, &q),
            });
        }
    }
    hits
}

fn snippet_around(text: &str, q: &str) -> String {
    let lower = text.to_lowercase();
    if let Some(pos) = lower.find(q) {
        let start = pos.saturating_sub(40);
        let end = (pos + q.len() + 40).min(text.len());
        // Align to char boundaries
        let start = text
            .char_indices()
            .find(|(i, _)| *i >= start)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end = text
            .char_indices()
            .find(|(i, _)| *i >= end)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        let mut s = text[start..end].to_string();
        if start > 0 {
            s.insert(0, '…');
        }
        if end < text.len() {
            s.push('…');
        }
        s.replace('\n', " ")
    } else {
        text.chars().take(80).collect::<String>().replace('\n', " ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportScenePreview {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub scenes: Vec<ImportScenePreview>,
}

/// Split Markdown/plain text into scenes by `#` / `##` headings or `***` breaks.
pub fn preview_import(raw: &str) -> ImportPreview {
    let mut scenes = Vec::new();
    let mut current_title = "Scène importée".to_string();
    let mut current_body = String::new();

    let flush = |title: &str, body: &str, out: &mut Vec<ImportScenePreview>| {
        let body = body.trim();
        if body.is_empty() && title == "Scène importée" {
            return;
        }
        out.push(ImportScenePreview {
            title: title.trim().trim_start_matches('#').trim().to_string(),
            body: body.to_string(),
        });
    };

    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("# ") || t.starts_with("## ") {
            flush(&current_title, &current_body, &mut scenes);
            current_title = t.trim_start_matches('#').trim().to_string();
            current_body.clear();
        } else if t == "***" || t == "---" || t == "* * *" {
            flush(&current_title, &current_body, &mut scenes);
            current_title = format!("Scène {}", scenes.len() + 1);
            current_body.clear();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    flush(&current_title, &current_body, &mut scenes);
    if scenes.is_empty() {
        scenes.push(ImportScenePreview {
            title: "Scène importée".into(),
            body: raw.to_string(),
        });
    }
    ImportPreview { scenes }
}

/// Apply import preview into the first act/chapter (creating structure if needed).
pub fn apply_import(project: &mut Project, preview: &ImportPreview) -> Result<usize, String> {
    if project.manuscript.index.acts.is_empty() {
        project
            .manuscript
            .ensure_default_structure(&project.root)
            .map_err(|e| e.to_string())?;
    }
    let act_id = project.manuscript.index.acts[0].id.clone();
    let ch_id = project.manuscript.index.acts[0].chapters[0].id.clone();
    let mut n = 0;
    for scene in &preview.scenes {
        let s = project
            .manuscript
            .add_scene(&project.root, &act_id, &ch_id, &scene.title)
            .map_err(|e| e.to_string())?;
        let sid = s.meta.id.clone();
        if let Some(sc) = project.manuscript.get_scene_mut(&sid) {
            sc.body = scene.body.clone();
            let sc = sc.clone();
            project
                .manuscript
                .save_scene(&sc)
                .map_err(|e| e.to_string())?;
        }
        n += 1;
    }
    Ok(n)
}

pub fn chapter_read_through(project: &Project, act_id: &str, chapter_id: &str) -> String {
    let Some(ch) = project
        .manuscript
        .index
        .acts
        .iter()
        .find(|a| a.id == act_id)
        .and_then(|a| a.chapters.iter().find(|c| c.id == chapter_id))
    else {
        return String::new();
    };
    let mut out = format!("# {}\n\n", ch.title);
    for sid in &ch.scene_ids {
        if let Some(s) = project.manuscript.get_scene(sid) {
            out.push_str(&format!("## {}\n\n{}\n\n", s.meta.title, s.body.trim()));
        }
    }
    out
}

pub fn note_kind_label(kind: NoteKind) -> &'static str {
    kind.label_fr_singular()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ak_core::{NoteKind, Project};

    #[test]
    fn preview_import_splits_headings_and_breaks() {
        let by_heading = preview_import("# Un\nAAA\n## Deux\nBBB");
        assert_eq!(by_heading.scenes.len(), 2);
        assert_eq!(by_heading.scenes[0].title, "Un");
        assert_eq!(by_heading.scenes[0].body, "AAA");
        assert_eq!(by_heading.scenes[1].title, "Deux");
        assert_eq!(by_heading.scenes[1].body, "BBB");

        let by_break = preview_import("premier\n***\ndeuxième");
        assert_eq!(by_break.scenes.len(), 2);
        assert_eq!(by_break.scenes[0].body, "premier");
        assert_eq!(by_break.scenes[1].body, "deuxième");

        let empty = preview_import("");
        assert_eq!(empty.scenes.len(), 1);
        assert_eq!(empty.scenes[0].title, "Scène importée");
        assert!(empty.scenes[0].body.is_empty());

        let plain = preview_import("juste du texte");
        assert_eq!(plain.scenes.len(), 1);
        assert_eq!(plain.scenes[0].body, "juste du texte");
    }

    #[test]
    fn search_and_read_through_on_temp_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = Project::create(dir.path(), "Roman", "Auteur").unwrap();
        let scene_id = project.manuscript.ordered_scenes()[0].meta.id.clone();
        {
            let scene = project.manuscript.get_scene_mut(&scene_id).unwrap();
            scene.body = "La pluie tombe sur Paris.".into();
            let scene = scene.clone();
            project.manuscript.save_scene(&scene).unwrap();
        }

        assert!(search_project(&project, "").is_empty());
        let hits = search_project(&project, "pluie");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, SearchHitKind::Scene);
        assert!(hits[0].snippet.to_lowercase().contains("pluie"));

        let act_id = project.manuscript.index.acts[0].id.clone();
        let ch_id = project.manuscript.index.acts[0].chapters[0].id.clone();
        let read = chapter_read_through(&project, &act_id, &ch_id);
        assert!(read.contains("La pluie tombe sur Paris."));
        assert!(chapter_read_through(&project, "missing", "missing").is_empty());
    }

    #[test]
    fn note_kind_label_delegates() {
        assert_eq!(note_kind_label(NoteKind::Character), "Personnage");
        assert_eq!(note_kind_label(NoteKind::Setting), "Lieu");
        assert_eq!(note_kind_label(NoteKind::Lore), "Concept");
    }

    #[test]
    fn apply_import_creates_scenes_from_preview() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = Project::create(dir.path(), "Roman", "Auteur").unwrap();
        // Clear default structure so apply_import rebuilds / fills first chapter.
        let preview = ImportPreview {
            scenes: vec![
                ImportScenePreview {
                    title: "Ouverture".into(),
                    body: "Il pleuvait.".into(),
                },
                ImportScenePreview {
                    title: "Suite".into(),
                    body: "Puis le soleil.".into(),
                },
            ],
        };
        let n = apply_import(&mut project, &preview).unwrap();
        assert_eq!(n, 2);
        let titles: Vec<_> = project
            .manuscript
            .ordered_scenes()
            .iter()
            .map(|s| s.meta.title.as_str())
            .collect();
        assert!(titles.contains(&"Ouverture"));
        assert!(titles.contains(&"Suite"));
        let bodies: String = project
            .manuscript
            .ordered_scenes()
            .iter()
            .map(|s| s.body.as_str())
            .collect();
        assert!(bodies.contains("Il pleuvait."));
        assert!(bodies.contains("Puis le soleil."));
    }
}
