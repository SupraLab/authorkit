//! Local document version history (scenes & notes).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::fsutil::{self, write_atomic};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSnapshot {
    pub id: String,
    pub created_at: String,
    pub doc_kind: String,
    pub doc_id: String,
    pub title: String,
    pub reason: String,
    pub body: String,
    pub meta_json: String,
}

fn versions_dir(root: &Path, doc_kind: &str, doc_id: &str) -> PathBuf {
    fsutil::meta_root(root)
        .join("versions")
        .join(doc_kind)
        .join(doc_id)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Keep at most this many snapshots per document.
const MAX_VERSIONS: usize = 50;

pub fn save_version(
    root: &Path,
    doc_kind: &str,
    doc_id: &str,
    title: &str,
    body: &str,
    meta_json: &str,
    reason: &str,
) -> Result<VersionSnapshot> {
    fsutil::ensure_meta_dirs(root)?;
    let snap = VersionSnapshot {
        id: Uuid::new_v4().to_string(),
        created_at: now_iso(),
        doc_kind: doc_kind.to_string(),
        doc_id: doc_id.to_string(),
        title: title.to_string(),
        reason: reason.to_string(),
        body: body.to_string(),
        meta_json: meta_json.to_string(),
    };
    let dir = versions_dir(root, doc_kind, doc_id);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", snap.id));
    write_atomic(&path, serde_json::to_vec_pretty(&snap)?)?;
    prune_old(root, doc_kind, doc_id)?;
    Ok(snap)
}

fn prune_old(root: &Path, doc_kind: &str, doc_id: &str) -> Result<()> {
    let mut list = list_versions(root, doc_kind, doc_id)?;
    prune_auto_hourly(root, doc_kind, doc_id, &list);
    list = list_versions(root, doc_kind, doc_id)?;
    if list.len() <= MAX_VERSIONS {
        return Ok(());
    }
    // list is newest first; drop oldest
    while list.len() > MAX_VERSIONS {
        if let Some(old) = list.pop() {
            let path = versions_dir(root, doc_kind, doc_id).join(format!("{}.json", old.id));
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

/// Drop hourly auto snapshots older than 24h (best-effort).
fn prune_auto_hourly(root: &Path, doc_kind: &str, doc_id: &str, list: &[VersionSnapshot]) {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    for snap in list {
        if snap.reason != "auto" {
            continue;
        }
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&snap.created_at) {
            if ts.with_timezone(&chrono::Utc) < cutoff {
                let path = versions_dir(root, doc_kind, doc_id).join(format!("{}.json", snap.id));
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

pub fn list_versions(root: &Path, doc_kind: &str, doc_id: &str) -> Result<Vec<VersionSnapshot>> {
    let dir = versions_dir(root, doc_kind, doc_id);
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
        if let Ok(s) = serde_json::from_str::<VersionSnapshot>(&raw) {
            out.push(s);
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn load_version(
    root: &Path,
    doc_kind: &str,
    doc_id: &str,
    version_id: &str,
) -> Result<VersionSnapshot> {
    let path = versions_dir(root, doc_kind, doc_id).join(format!("{version_id}.json"));
    if !path.exists() {
        return Err(Error::Msg(format!("Version introuvable: {version_id}")));
    }
    let raw = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

/// Restore a version into the live document, after saving the current content as a safety snapshot.
pub fn restore_version_preserving_current(
    root: &Path,
    doc_kind: &str,
    doc_id: &str,
    version_id: &str,
    current_title: &str,
    current_body: &str,
    current_meta_json: &str,
) -> Result<VersionSnapshot> {
    let target = load_version(root, doc_kind, doc_id, version_id)?;
    save_version(
        root,
        doc_kind,
        doc_id,
        current_title,
        current_body,
        current_meta_json,
        "avant restauration",
    )?;
    Ok(target)
}
