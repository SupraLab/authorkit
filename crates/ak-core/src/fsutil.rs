//! Atomic file writes and shared path helpers.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{Error, Result};

/// Write `bytes` to `path` atomically: temp file in the same directory, then rename.
pub fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    let bytes = bytes.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp_name = format!(".{}.{}.tmp", file_name, std::process::id());
    let tmp_path = parent.join(&tmp_name);

    let write_result = (|| -> Result<()> {
        let mut f = std::fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }

    // On Windows rename fails if destination exists; remove first when needed.
    if path.exists() {
        #[cfg(windows)]
        {
            let _ = std::fs::remove_file(path);
        }
    }
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        Error::Io(e)
    })?;
    Ok(())
}

/// Project-local AuthorKit metadata root: `{root}/.authorkit-rust/`.
pub fn meta_root(project_root: &Path) -> PathBuf {
    project_root.join(".authorkit-rust")
}

pub fn ensure_meta_dirs(project_root: &Path) -> Result<()> {
    let root = meta_root(project_root);
    for sub in ["versions", "trash", "chat", "reviews"] {
        std::fs::create_dir_all(root.join(sub))?;
    }
    let state = root.join("state.json");
    if !state.exists() {
        let doc = serde_json::json!({
            "format_version": 1,
        });
        write_atomic(&state, serde_json::to_vec_pretty(&doc)?)?;
    }
    Ok(())
}

/// File modification time as unix millis, if available.
pub fn mtime_millis(path: &Path) -> Option<u128> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
}

/// Simple content fingerprint for conflict detection.
pub fn content_hash(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_stable_and_differs() {
        assert_eq!(content_hash(b"hello"), content_hash(b"hello"));
        assert_ne!(content_hash(b"hello"), content_hash(b"world"));
        assert_ne!(content_hash(b""), content_hash(b"x"));
    }

    #[test]
    fn meta_root_ends_with_authorkit_rust() {
        let root = Path::new("/tmp/my-project");
        let meta = meta_root(root);
        assert!(meta.ends_with(".authorkit-rust"));
        assert_eq!(meta, PathBuf::from("/tmp/my-project/.authorkit-rust"));
    }

    #[test]
    fn ensure_meta_dirs_creates_layout_and_state() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        ensure_meta_dirs(root).unwrap();

        let meta = meta_root(root);
        for sub in ["versions", "trash", "chat", "reviews"] {
            assert!(meta.join(sub).is_dir(), "missing dir {sub}");
        }
        let state = meta.join("state.json");
        assert!(state.is_file());
        let raw = std::fs::read_to_string(&state).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["format_version"], 1);

        // Idempotent: second call keeps existing state.json
        ensure_meta_dirs(root).unwrap();
        assert_eq!(std::fs::read_to_string(&state).unwrap(), raw);
    }

    #[test]
    fn write_atomic_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("file.txt");
        write_atomic(&path, "payload").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "payload");
        write_atomic(&path, "updated").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "updated");
    }

    #[test]
    fn mtime_millis_some_on_written_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.txt");
        write_atomic(&path, "x").unwrap();
        assert!(mtime_millis(&path).is_some());
        assert!(mtime_millis(&dir.path().join("missing")).is_none());
    }
}
