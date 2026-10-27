use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Not an AuthorKit project: missing authorkit.toml at {0}")]
    NotAProject(PathBuf),
    #[error("Scene not found: {0}")]
    SceneNotFound(String),
    #[error("Note not found: {0}")]
    NoteNotFound(String),
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, Error>;
