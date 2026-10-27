//! Project open diagnostics: orphans, broken refs, unreadable files.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::Result;
use crate::manuscript::Manuscript;
use crate::notes::{NoteKind, NotesLibrary};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenDiagnostics {
    pub items: Vec<Diagnostic>,
}

impl OpenDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }
}

/// Collect diagnostics for a loaded manuscript + notes library.
pub fn inspect_project(
    root: &Path,
    manuscript: &Manuscript,
    notes: &NotesLibrary,
) -> Result<OpenDiagnostics> {
    let mut items = Vec::new();

    // Index scene ids that have no file / no loaded scene
    for act in &manuscript.index.acts {
        for ch in &act.chapters {
            for sid in &ch.scene_ids {
                if manuscript.get_scene(sid).is_none() {
                    items.push(Diagnostic {
                        severity: DiagnosticSeverity::Error,
                        code: "broken_scene_ref".into(),
                        message: format!(
                            "Référence de scène cassée dans « {} » / « {} » : {sid}",
                            act.title, ch.title
                        ),
                        path: Some("manuscript/index.json".into()),
                        id: Some(sid.clone()),
                    });
                }
            }
        }
    }

    // Orphan scene files (on disk / loaded but not in index)
    let indexed: std::collections::HashSet<&str> = manuscript
        .index
        .acts
        .iter()
        .flat_map(|a| a.chapters.iter())
        .flat_map(|c| c.scene_ids.iter().map(|s| s.as_str()))
        .collect();

    for scene in &manuscript.scenes {
        if !indexed.contains(scene.meta.id.as_str()) {
            items.push(Diagnostic {
                severity: DiagnosticSeverity::Warning,
                code: "orphan_scene".into(),
                message: format!(
                    "Scène orpheline « {} » (absente de l’index)",
                    scene.meta.title
                ),
                path: Some(
                    scene
                        .path
                        .strip_prefix(root)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| scene.path.display().to_string()),
                ),
                id: Some(scene.meta.id.clone()),
            });
        }

        // Broken note links
        for lid in &scene.meta.links {
            if notes.get(lid).is_none() {
                items.push(Diagnostic {
                    severity: DiagnosticSeverity::Warning,
                    code: "broken_note_link".into(),
                    message: format!(
                        "Lien de fiche cassé depuis « {} » : {lid}",
                        scene.meta.title
                    ),
                    path: None,
                    id: Some(lid.clone()),
                });
            }
        }
    }

    // Unreadable scene files (reported separately during load — check dir)
    let scenes_dir = root.join("manuscript/scenes");
    if scenes_dir.exists() {
        for entry in std::fs::read_dir(&scenes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(raw) => {
                    if let Err(msg) = validate_scene_frontmatter(&raw) {
                        items.push(Diagnostic {
                            severity: DiagnosticSeverity::Error,
                            code: "unreadable_scene".into(),
                            message: msg,
                            path: Some(
                                path.strip_prefix(root)
                                    .map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_else(|_| path.display().to_string()),
                            ),
                            id: None,
                        });
                    }
                }
                Err(e) => {
                    items.push(Diagnostic {
                        severity: DiagnosticSeverity::Error,
                        code: "unreadable_scene".into(),
                        message: format!("Fichier illisible : {e}"),
                        path: Some(
                            path.strip_prefix(root)
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_else(|_| path.display().to_string()),
                        ),
                        id: None,
                    });
                }
            }
        }
    }

    // Unreadable note files
    let notes_root = root.join("notes");
    for kind in NoteKind::all() {
        let dir = notes_root.join(kind.dir_name());
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(raw) => {
                    if let Err(msg) = validate_note_frontmatter(&raw) {
                        items.push(Diagnostic {
                            severity: DiagnosticSeverity::Error,
                            code: "unreadable_note".into(),
                            message: msg,
                            path: Some(
                                path.strip_prefix(root)
                                    .map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_else(|_| path.display().to_string()),
                            ),
                            id: None,
                        });
                    }
                }
                Err(e) => {
                    items.push(Diagnostic {
                        severity: DiagnosticSeverity::Error,
                        code: "unreadable_note".into(),
                        message: format!("Fiche illisible : {e}"),
                        path: Some(
                            path.strip_prefix(root)
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_else(|_| path.display().to_string()),
                        ),
                        id: None,
                    });
                }
            }
        }
    }

    Ok(OpenDiagnostics { items })
}

fn validate_note_frontmatter(raw: &str) -> std::result::Result<(), String> {
    let raw = raw.trim_start_matches('\u{feff}');
    let Some(rest) = raw.strip_prefix("---") else {
        return Err("Frontmatter de fiche manquant".into());
    };
    let Some(end) = rest.find("\n---") else {
        return Err("Frontmatter de fiche non terminé".into());
    };
    let json = rest[..end].trim();
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("JSON frontmatter fiche invalide : {e}"))?;
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if id.is_empty() {
        return Err("Identifiant de fiche manquant ou vide".into());
    }
    Ok(())
}

/// Strict frontmatter check: valid JSON with required id+title, or report error (no silent UUID).
fn validate_scene_frontmatter(raw: &str) -> std::result::Result<(), String> {
    let raw = raw.trim_start_matches('\u{feff}');
    let Some(rest) = raw.strip_prefix("---") else {
        return Err("Frontmatter manquant (attendu --- … ---)".into());
    };
    let Some(end) = rest.find("\n---") else {
        return Err("Frontmatter non terminé".into());
    };
    let json = rest[..end].trim();
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("JSON frontmatter invalide : {e}"))?;
    let id = value.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let title = value.get("title").and_then(|v| v.as_str()).unwrap_or("");
    if id.is_empty() {
        return Err("Identifiant de scène manquant ou vide".into());
    }
    if title.is_empty() {
        return Err("Titre de scène manquant ou vide".into());
    }
    Ok(())
}
