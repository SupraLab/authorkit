use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::diagnostics::{inspect_project, OpenDiagnostics};
use crate::error::{Error, Result};
use crate::fsutil::{self, write_atomic};
use crate::manuscript::Manuscript;
use crate::notes::{ensure_notes_dirs, NotesLibrary};
use crate::styles::WritingStyles;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub title: String,
    pub author: String,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub premise: String,
}

fn default_lang() -> String {
    "fr".into()
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            title: "Sans titre".into(),
            author: String::new(),
            language: "fr".into(),
            status: "brouillon".into(),
            premise: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AuthorKitToml {
    book: ProjectMeta,
    #[serde(default)]
    styles: WritingStyles,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
    pub styles: WritingStyles,
    pub manuscript: Manuscript,
    pub notes: NotesLibrary,
}

/// Result of opening a project, including non-fatal diagnostics.
#[derive(Debug, Clone)]
pub struct ProjectOpen {
    pub project: Project,
    pub diagnostics: OpenDiagnostics,
}

impl Project {
    /// Create a new project. Refuses if `authorkit.toml` already exists.
    pub fn create(root: &Path, title: &str, author: &str) -> Result<Self> {
        let toml_path = root.join("authorkit.toml");
        if toml_path.exists() {
            return Err(Error::Msg(format!(
                "Un projet AuthorKit existe déjà dans « {} ». Création refusée.",
                root.display()
            )));
        }
        // Also refuse if manuscript index already present (partial project).
        if root.join("manuscript/index.json").exists() {
            return Err(Error::Msg(format!(
                "Un manuscrit existe déjà dans « {} ». Création refusée.",
                root.display()
            )));
        }

        std::fs::create_dir_all(root)?;
        let meta = ProjectMeta {
            title: title.to_string(),
            author: author.to_string(),
            ..Default::default()
        };
        let styles = WritingStyles::default();
        write_toml(root, &meta, &styles)?;

        std::fs::create_dir_all(root.join("manuscript/scenes"))?;
        std::fs::create_dir_all(root.join("prompts/brushes"))?;
        ensure_notes_dirs(root)?;
        fsutil::ensure_meta_dirs(root)?;
        write_gitignore(root)?;

        let mut manuscript = Manuscript::default();
        manuscript.ensure_default_structure(root)?;
        let notes = NotesLibrary::load(root)?;

        Ok(Self {
            root: root.to_path_buf(),
            meta,
            styles,
            manuscript,
            notes,
        })
    }

    pub fn open(root: &Path) -> Result<Self> {
        Ok(Self::open_with_diagnostics(root)?.project)
    }

    pub fn open_with_diagnostics(root: &Path) -> Result<ProjectOpen> {
        let toml_path = root.join("authorkit.toml");
        if !toml_path.exists() {
            return Err(Error::NotAProject(root.to_path_buf()));
        }
        let raw = std::fs::read_to_string(&toml_path)?;
        let parsed: AuthorKitToml = toml::from_str(&raw)?;
        let manuscript = Manuscript::load(root)?;
        let notes = match NotesLibrary::load(root) {
            Ok(n) => n,
            Err(e) => {
                // Surface as empty notes + diagnostic via inspect; but hard IO still fails.
                return Err(e);
            }
        };
        let _ = fsutil::ensure_meta_dirs(root);
        let diagnostics = inspect_project(root, &manuscript, &notes)?;
        Ok(ProjectOpen {
            project: Self {
                root: root.to_path_buf(),
                meta: parsed.book,
                styles: parsed.styles,
                manuscript,
                notes,
            },
            diagnostics,
        })
    }

    pub fn save_meta(&self) -> Result<()> {
        write_toml(&self.root, &self.meta, &self.styles)
    }

    pub fn reload_notes(&mut self) -> Result<()> {
        self.notes = NotesLibrary::load(&self.root)?;
        Ok(())
    }

    pub fn reload_manuscript(&mut self) -> Result<()> {
        self.manuscript = Manuscript::load(&self.root)?;
        Ok(())
    }

    /// Snapshot current scene/note before a destructive AI edit.
    pub fn snapshot_document(
        &self,
        doc_kind: &str,
        doc_id: &str,
        reason: &str,
    ) -> Result<crate::versions::VersionSnapshot> {
        match doc_kind {
            "scene" => {
                let scene = self
                    .manuscript
                    .get_scene(doc_id)
                    .ok_or_else(|| Error::SceneNotFound(doc_id.into()))?;
                let meta_json = serde_json::to_string(&scene.meta)?;
                crate::versions::save_version(
                    &self.root,
                    "scene",
                    doc_id,
                    &scene.meta.title,
                    &scene.body,
                    &meta_json,
                    reason,
                )
            }
            "note" => {
                let note = self
                    .notes
                    .get(doc_id)
                    .ok_or_else(|| Error::NoteNotFound(doc_id.into()))?;
                let meta_json = serde_json::to_string(&note.meta)?;
                crate::versions::save_version(
                    &self.root,
                    "note",
                    doc_id,
                    &note.meta.name,
                    &note.body,
                    &meta_json,
                    reason,
                )
            }
            other => Err(Error::Msg(format!("Type de document inconnu: {other}"))),
        }
    }
}

fn write_toml(root: &Path, meta: &ProjectMeta, styles: &WritingStyles) -> Result<()> {
    let doc = AuthorKitToml {
        book: meta.clone(),
        styles: styles.clone(),
    };
    let raw = toml::to_string_pretty(&doc)?;
    write_atomic(&root.join("authorkit.toml"), raw)
}

fn write_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".gitignore");
    if !path.exists() {
        write_atomic(&path, ".authorkit-rust/\n.DS_Store\n")?;
    }
    Ok(())
}
