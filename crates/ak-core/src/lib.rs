//! AuthorKit project model: manuscript, notes, links, styles.

mod diagnostics;
mod error;
pub mod fsutil;
mod manuscript;
mod notes;
mod project;
mod styles;
pub mod trash;
pub mod versions;

pub use diagnostics::{Diagnostic, DiagnosticSeverity, OpenDiagnostics};
pub use error::{Error, Result};
pub use manuscript::{Act, Chapter, Manuscript, Scene, SceneMeta, SceneStatus};
pub use notes::{Note, NoteKind, NotesLibrary};
pub use project::{Project, ProjectMeta, ProjectOpen};
pub use styles::WritingStyles;
pub use trash::{list_trash, restore_trash_entry, TrashEntry, TrashEntryKind};
pub use versions::{
    list_versions, load_version, restore_version_preserving_current, save_version, VersionSnapshot,
};
