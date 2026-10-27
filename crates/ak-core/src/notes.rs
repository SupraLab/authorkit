use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::fsutil::write_atomic;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Character,
    Setting,
    Lore,
}

impl NoteKind {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Character => "characters",
            Self::Setting => "settings",
            Self::Lore => "lore",
        }
    }

    pub fn label_fr(self) -> &'static str {
        match self {
            Self::Character => "Personnages",
            Self::Setting => "Lieux",
            // Abstract worldbuilding (magic, history, factions…) — not places.
            Self::Lore => "Concepts",
        }
    }

    pub fn label_fr_singular(self) -> &'static str {
        match self {
            Self::Character => "Personnage",
            Self::Setting => "Lieu",
            Self::Lore => "Concept",
        }
    }

    pub fn all() -> [NoteKind; 3] {
        [Self::Character, Self::Setting, Self::Lore]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteFrontmatter {
    pub id: String,
    pub name: String,
    pub kind: NoteKind,
    #[serde(default)]
    pub links: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Note {
    pub meta: NoteFrontmatter,
    pub body: String,
    pub path: PathBuf,
}

impl Note {
    pub fn summary(&self, max_chars: usize) -> String {
        let t = self.body.trim();
        if t.chars().count() <= max_chars {
            t.to_string()
        } else {
            let s: String = t.chars().take(max_chars).collect();
            format!("{s}…")
        }
    }

    pub fn as_context_block(&self) -> String {
        format!(
            "### {} ({})\n{}",
            self.meta.name,
            self.meta.kind.label_fr_singular(),
            self.body.trim()
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct NotesLibrary {
    pub notes: Vec<Note>,
}

impl NotesLibrary {
    pub fn load(root: &Path) -> Result<Self> {
        let mut notes = Vec::new();
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
                match load_note(&path, kind) {
                    Ok(note) => notes.push(note),
                    Err(_) => {
                        // Surfaced via OpenDiagnostics (unreadable files).
                    }
                }
            }
        }
        notes.sort_by_key(|a| a.meta.name.to_lowercase());
        Ok(Self { notes })
    }

    pub fn by_kind(&self, kind: NoteKind) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.meta.kind == kind)
    }

    pub fn get(&self, id: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.meta.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.meta.id == id)
    }

    pub fn create(&mut self, root: &Path, kind: NoteKind, name: &str) -> Result<&Note> {
        let id = Uuid::new_v4().to_string();
        let slug = slugify(name);
        let dir = root.join("notes").join(kind.dir_name());
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{slug}-{}.md", &id[..8]));
        let meta = NoteFrontmatter {
            id: id.clone(),
            name: name.to_string(),
            kind,
            links: Vec::new(),
        };
        let body = match kind {
            NoteKind::Character => {
                "# Apparence\n\n\n# Personnalité\n\n\n# Arc\n\n\n# Notes de voix\n\n".to_string()
            }
            NoteKind::Setting => {
                "# Description\n\n\n# Ambiance\n\n\n# Détails utiles\n\n".to_string()
            }
            NoteKind::Lore => {
                "# Résumé\n\n\n# Règles / faits\n\n\n# Liens avec l’histoire\n\n".to_string()
            }
        };
        write_note(&path, &meta, &body)?;
        self.notes.push(Note { meta, body, path });
        self.notes.sort_by_key(|a| a.meta.name.to_lowercase());
        Ok(self.get(&id).expect("just inserted"))
    }

    pub fn save_note(&self, note: &Note) -> Result<()> {
        write_note(&note.path, &note.meta, &note.body)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<()> {
        let note = self
            .notes
            .iter_mut()
            .find(|n| n.meta.id == id)
            .ok_or_else(|| crate::error::Error::Msg(format!("Note introuvable: {id}")))?;
        note.meta.name = name.to_string();
        let note = note.clone();
        self.save_note(&note)?;
        self.notes.sort_by_key(|a| a.meta.name.to_lowercase());
        Ok(())
    }

    pub fn delete(&mut self, id: &str, root: &Path) -> Result<()> {
        self.trash_note(id, root)?;
        Ok(())
    }

    pub fn context_for_ids(&self, ids: &[String]) -> String {
        ids.iter()
            .filter_map(|id| self.get(id))
            .map(|n| n.as_context_block())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn load_note(path: &Path, fallback_kind: NoteKind) -> Result<Note> {
    let raw = std::fs::read_to_string(path)?;
    let (meta, body) = parse_frontmatter(&raw, fallback_kind)?;
    Ok(Note {
        meta,
        body,
        path: path.to_path_buf(),
    })
}

fn write_note(path: &Path, meta: &NoteFrontmatter, body: &str) -> Result<()> {
    let yaml = serde_json::to_string_pretty(meta)?;
    // Use JSON frontmatter between --- for simplicity and robust parsing
    let content = format!("---\n{yaml}\n---\n\n{body}");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(path, content)
}

fn parse_frontmatter(raw: &str, fallback_kind: NoteKind) -> Result<(NoteFrontmatter, String)> {
    let _ = fallback_kind;
    let raw = raw.trim_start_matches('\u{feff}');
    if let Some(rest) = raw.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let json = rest[..end].trim();
            let body = rest[end + 4..].trim_start_matches('\n').to_string();
            let meta: NoteFrontmatter = serde_json::from_str(json)
                .map_err(|e| Error::Msg(format!("Frontmatter de fiche invalide : {e}")))?;
            if meta.id.trim().is_empty() {
                return Err(Error::Msg(
                    "Identifiant de fiche manquant ou vide dans le frontmatter".into(),
                ));
            }
            return Ok((meta, body));
        }
    }
    Err(Error::Msg(
        "Frontmatter de fiche manquant (attendu --- … ---)".into(),
    ))
}

pub fn slugify(s: &str) -> String {
    let s = s.to_lowercase();
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if (c.is_whitespace() || c == '-' || c == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "note".into()
    } else {
        out
    }
}

/// Re-export for callers that need Error on bad paths.
pub fn ensure_notes_dirs(root: &Path) -> Result<()> {
    for kind in NoteKind::all() {
        std::fs::create_dir_all(root.join("notes").join(kind.dir_name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_note(name: &str, kind: NoteKind, body: &str) -> Note {
        Note {
            meta: NoteFrontmatter {
                id: "id-1".into(),
                name: name.into(),
                kind,
                links: Vec::new(),
            },
            body: body.into(),
            path: PathBuf::from("notes/characters/x.md"),
        }
    }

    #[test]
    fn slugify_empty_or_punct_yields_note() {
        assert_eq!(slugify(""), "note");
        assert_eq!(slugify("!!!"), "note");
        assert_eq!(slugify("---"), "note");
        assert_eq!(slugify("   "), "note");
    }

    #[test]
    fn slugify_drops_accents() {
        assert_eq!(slugify("café"), "caf");
        assert_eq!(slugify("Été"), "t");
        assert_eq!(slugify("naïve"), "nave");
    }

    #[test]
    fn slugify_spaces_underscores_to_dashes() {
        assert_eq!(slugify("hello world"), "hello-world");
        assert_eq!(slugify("hello_world"), "hello-world");
        assert_eq!(slugify("a  b__c"), "a-b-c");
    }

    #[test]
    fn slugify_trims_dashes() {
        assert_eq!(slugify("-hello-"), "hello");
        assert_eq!(slugify("---foo---"), "foo");
        assert_eq!(slugify("  -bar-  "), "bar");
    }

    #[test]
    fn slugify_keeps_alphanumeric() {
        assert_eq!(slugify("Hero42"), "hero42");
        assert_eq!(slugify("abc123XYZ"), "abc123xyz");
    }

    #[test]
    fn note_kind_dir_name_all_variants() {
        assert_eq!(NoteKind::Character.dir_name(), "characters");
        assert_eq!(NoteKind::Setting.dir_name(), "settings");
        assert_eq!(NoteKind::Lore.dir_name(), "lore");
    }

    #[test]
    fn note_kind_label_fr_all_variants() {
        assert_eq!(NoteKind::Character.label_fr(), "Personnages");
        assert_eq!(NoteKind::Setting.label_fr(), "Lieux");
        assert_eq!(NoteKind::Lore.label_fr(), "Concepts");
    }

    #[test]
    fn note_kind_label_fr_singular_all_variants() {
        assert_eq!(NoteKind::Character.label_fr_singular(), "Personnage");
        assert_eq!(NoteKind::Setting.label_fr_singular(), "Lieu");
        assert_eq!(NoteKind::Lore.label_fr_singular(), "Concept");
    }

    #[test]
    fn note_kind_all_has_three_variants() {
        assert_eq!(
            NoteKind::all(),
            [NoteKind::Character, NoteKind::Setting, NoteKind::Lore]
        );
    }

    #[test]
    fn note_summary_short_unchanged() {
        let note = sample_note("A", NoteKind::Character, "  short body  ");
        assert_eq!(note.summary(100), "short body");
    }

    #[test]
    fn note_summary_truncates_with_ellipsis() {
        let note = sample_note("A", NoteKind::Character, "abcdefghij");
        assert_eq!(note.summary(5), "abcde…");
    }

    #[test]
    fn note_as_context_block_format() {
        let note = sample_note("Alice", NoteKind::Character, "  She is brave.\n");
        assert_eq!(
            note.as_context_block(),
            "### Alice (Personnage)\nShe is brave."
        );
        let setting = sample_note("Castle", NoteKind::Setting, "Cold stone.");
        assert_eq!(setting.as_context_block(), "### Castle (Lieu)\nCold stone.");
        let lore = sample_note("Magic", NoteKind::Lore, "Costs blood.");
        assert_eq!(lore.as_context_block(), "### Magic (Concept)\nCosts blood.");
    }

    #[test]
    fn notes_library_create_rename_get_context_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        ensure_notes_dirs(root).unwrap();

        let mut lib = NotesLibrary::load(root).unwrap();
        assert!(lib.notes.is_empty());

        let created = lib
            .create(root, NoteKind::Character, "Hélène Dupont")
            .unwrap()
            .clone();
        assert_eq!(created.meta.name, "Hélène Dupont");
        assert_eq!(created.meta.kind, NoteKind::Character);
        assert!(!created.meta.id.is_empty());
        assert!(created.path.exists());
        assert!(created.body.contains("# Apparence"));

        let id = created.meta.id.clone();
        assert!(lib.get(&id).is_some());

        lib.rename(&id, "Hélène Renommée").unwrap();
        assert_eq!(lib.get(&id).unwrap().meta.name, "Hélène Renommée");

        // Reload to exercise parse_frontmatter via load_note
        let reloaded = NotesLibrary::load(root).unwrap();
        let note = reloaded.get(&id).expect("note persisted");
        assert_eq!(note.meta.name, "Hélène Renommée");
        assert_eq!(note.meta.kind, NoteKind::Character);
        assert_eq!(note.body, created.body);

        let ctx = reloaded.context_for_ids(&[id.clone(), "missing".into()]);
        assert!(ctx.starts_with("### Hélène Renommée (Personnage)\n"));
        assert!(ctx.contains("# Apparence"));
    }
}
