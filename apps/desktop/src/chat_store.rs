//! Per-project chat persistence under `.authorkit-rust/chat/`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use ak_core::fsutil::{self, write_atomic};
use ak_llm::ChatMessage;

use crate::panels::nav::NavSelection;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoredMsgStatus {
    Pending,
    Streaming,
    #[default]
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditApplyMode {
    ReplaceSelection,
    InsertAtCursor,
    AppendEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditProposalStatus {
    Streaming,
    Ready,
    Accepted,
    Rejected,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditRegenSpec {
    Brush {
        brush_id: String,
    },
    Continue {
        mode: String,
        prompt: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessagePayload {
    #[default]
    Chat,
    /// User turn carrying a linked text excerpt. `content` keeps the full LLM prompt;
    /// these fields drive the visual rendering.
    UserQuote {
        quote: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        note: String,
        /// Tool key for the intent icon: `chat`, `brush:<id>`, `continue:<mode>`.
        #[serde(default)]
        tool: Option<String>,
    },
    EditProposal {
        label: String,
        mode: EditApplyMode,
        range_start: usize,
        range_end: usize,
        doc_id: String,
        doc_kind: String,
        original_slice: String,
        anchor_before: String,
        anchor_after: String,
        proposal_status: EditProposalStatus,
        conflict: bool,
        #[serde(default)]
        error: Option<String>,
        job_id: String,
        regen: EditRegenSpec,
    },
}

impl MessagePayload {
    pub fn is_edit_proposal(&self) -> bool {
        matches!(self, Self::EditProposal { .. })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub status: StoredMsgStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub payload: MessagePayload,
}

impl StoredMessage {
    pub fn to_llm(&self) -> ChatMessage {
        ChatMessage {
            role: self.role.clone(),
            content: self.content.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ComposerMode {
    #[default]
    Chat,
    Continue {
        /// `continue` | `sentence` | `paragraph` | `generate`
        mode: String,
    },
    Brush {
        id: String,
    },
}

impl ComposerMode {
    pub fn continue_mode(mode: &str) -> Self {
        Self::Continue {
            mode: mode.to_string(),
        }
    }

    pub fn brush(id: impl Into<String>) -> Self {
        Self::Brush { id: id.into() }
    }

    pub fn label<'a>(&'a self, brushes: &'a [ak_llm::Brush]) -> &'a str {
        match self {
            Self::Chat => "Chat",
            Self::Continue { mode } => match mode.as_str() {
                "sentence" => "Continuer (phrase)",
                "paragraph" => "Continuer (paragraphe)",
                "generate" => "Générer",
                _ => "Continuer",
            },
            Self::Brush { id } => brushes
                .iter()
                .find(|b| b.id == *id)
                .map(|b| b.label.as_str())
                .unwrap_or("Outil"),
        }
    }

    pub fn is_chat(&self) -> bool {
        matches!(self, Self::Chat)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposerAttachment {
    pub text: String,
    pub range_start: usize,
    pub range_end: usize,
    pub doc_id: String,
    pub doc_kind: String,
    pub anchor_before: String,
    pub anchor_after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConversation {
    pub id: String,
    pub title: String,
    /// True once the title was produced by the model (stop deriving it from messages).
    #[serde(default)]
    pub title_generated: bool,
    #[serde(default)]
    pub context_id: String,
    /// Legacy field — migrated into `context_id` on load; never written back.
    #[serde(default, skip_serializing)]
    pub scene_id: Option<String>,
    pub updated_at: String,
    /// v2 messages with ids/status. Legacy files may still deserialize via migration.
    #[serde(default)]
    pub messages: Vec<StoredMessage>,
    pub draft: String,
    #[serde(default)]
    pub composer_mode: ComposerMode,
    #[serde(default)]
    pub attachment: Option<ComposerAttachment>,
    #[serde(default)]
    pub format_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChatIndex {
    active_id: Option<String>,
    #[serde(default)]
    active_by_context: HashMap<String, String>,
    conversations: Vec<String>,
}

fn chat_dir(root: &Path) -> std::path::PathBuf {
    fsutil::meta_root(root).join("chat")
}

/// Stable context key for the current left-nav selection.
pub fn context_id_for(sel: &NavSelection) -> String {
    match sel {
        NavSelection::Scene(id) => format!("scene:{id}"),
        NavSelection::Note(id) => format!("note:{id}"),
        NavSelection::Styles => "styles".into(),
        NavSelection::Publish => "publish".into(),
    }
}

pub fn list_conversations(root: &Path) -> Vec<StoredConversation> {
    let dir = chat_dir(root);
    if !dir.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some("index.json") {
                continue;
            }
            if let Some(c) = load_conversation_path(&path) {
                out.push(c);
            }
        }
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

pub fn list_for_context(root: &Path, context_id: &str) -> Vec<StoredConversation> {
    let mut out: Vec<_> = list_conversations(root)
        .into_iter()
        .filter(|c| c.context_id == context_id)
        .collect();
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    out
}

pub fn save_conversation(root: &Path, conv: &StoredConversation) -> Result<(), String> {
    let _ = fsutil::ensure_meta_dirs(root);
    let path = chat_dir(root).join(format!("{}.json", conv.id));
    let mut conv = conv.clone();
    conv.format_version = 2;
    conv.scene_id = None;
    let raw = serde_json::to_vec_pretty(&conv).map_err(|e| e.to_string())?;
    write_atomic(&path, raw).map_err(|e| e.to_string())?;
    let mut index = load_index(root);
    if !index.conversations.contains(&conv.id) {
        index.conversations.push(conv.id.clone());
    }
    index.active_id = Some(conv.id.clone());
    if !conv.context_id.is_empty() {
        index
            .active_by_context
            .insert(conv.context_id.clone(), conv.id.clone());
    }
    save_index(root, &index)
}

pub fn load_conversation(root: &Path, id: &str) -> Option<StoredConversation> {
    let path = chat_dir(root).join(format!("{id}.json"));
    load_conversation_path(&path)
}

fn load_conversation_path(path: &Path) -> Option<StoredConversation> {
    let raw = std::fs::read_to_string(path).ok()?;
    // Try v2 first.
    if let Ok(mut c) = serde_json::from_str::<StoredConversation>(&raw) {
        migrate_conversation(&mut c);
        return Some(c);
    }
    // Legacy: messages were bare ChatMessage without id/status.
    #[derive(Deserialize)]
    struct Legacy {
        id: String,
        title: String,
        #[serde(default)]
        scene_id: Option<String>,
        #[serde(default)]
        context_id: String,
        updated_at: String,
        messages: Vec<ChatMessage>,
        draft: String,
    }
    let legacy: Legacy = serde_json::from_str(&raw).ok()?;
    let mut c = StoredConversation {
        id: legacy.id,
        title: legacy.title,
        title_generated: false,
        context_id: legacy.context_id,
        scene_id: legacy.scene_id,
        updated_at: legacy.updated_at,
        messages: legacy
            .messages
            .into_iter()
            .map(|m| StoredMessage {
                id: uuid_simple(),
                role: m.role,
                content: m.content,
                status: StoredMsgStatus::Done,
                created_at: stamp_now(),
                payload: MessagePayload::Chat,
            })
            .collect(),
        draft: legacy.draft,
        composer_mode: ComposerMode::Chat,
        attachment: None,
        format_version: 2,
    };
    migrate_conversation(&mut c);
    Some(c)
}

fn migrate_conversation(c: &mut StoredConversation) {
    if c.context_id.is_empty() {
        if let Some(sid) = c.scene_id.take() {
            if !sid.is_empty() {
                c.context_id = format!("scene:{sid}");
            }
        }
    } else {
        c.scene_id = None;
    }
    migrate_messages(c);
}

fn migrate_messages(c: &mut StoredConversation) {
    for m in &mut c.messages {
        if m.id.is_empty() {
            m.id = uuid_simple();
        }
        if m.created_at.is_empty() {
            m.created_at = stamp_now();
        }
    }
}

/// Last conversation for this context, or a fresh empty shell bound to it.
pub fn last_or_empty(root: &Path, context_id: &str) -> StoredConversation {
    let index = load_index(root);
    if let Some(id) = index.active_by_context.get(context_id) {
        if let Some(c) = load_conversation(root, id) {
            if c.context_id == context_id {
                return c;
            }
        }
    }
    if let Some(c) = list_for_context(root, context_id).into_iter().next() {
        return c;
    }
    new_conversation(context_id.to_string())
}

/// Prefer the indexed active conversation when it matches `context_id`, else last/empty.
pub fn active_or_new(root: &Path, context_id: String) -> StoredConversation {
    let index = load_index(root);
    if let Some(id) = &index.active_id {
        if let Some(c) = load_conversation(root, id) {
            if c.context_id == context_id || context_id.is_empty() {
                return c;
            }
        }
    }
    last_or_empty(root, &context_id)
}

pub fn new_conversation(context_id: String) -> StoredConversation {
    StoredConversation {
        id: uuid_simple(),
        title: "Nouvelle conversation".into(),
        title_generated: false,
        context_id,
        scene_id: None,
        updated_at: chrono_now(),
        messages: Vec::new(),
        draft: String::new(),
        composer_mode: ComposerMode::Chat,
        attachment: None,
        format_version: 2,
    }
}

fn load_index(root: &Path) -> ChatIndex {
    let path = chat_dir(root).join("index.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default()
}

fn save_index(root: &Path, index: &ChatIndex) -> Result<(), String> {
    let path = chat_dir(root).join("index.json");
    let raw = serde_json::to_vec_pretty(index).map_err(|e| e.to_string())?;
    write_atomic(&path, raw).map_err(|e| e.to_string())
}

fn uuid_simple() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Local wall-clock stamp for conversation metadata (sortable + readable).
pub fn stamp_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

fn chrono_now() -> String {
    stamp_now()
}

/// Short display form for history rows (accepts stamp_now / legacy Debug dumps).
pub fn format_stamp(raw: &str) -> String {
    if raw.starts_with("SystemTime") {
        return "—".into();
    }
    if raw.len() >= 16 && raw.as_bytes().get(4) == Some(&b'-') {
        let mm = &raw[5..7];
        let dd = &raw[8..10];
        let hm = raw.get(11..16).unwrap_or("");
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if raw.starts_with(&today) {
            return hm.to_string();
        }
        return format!("{dd}/{mm} {hm}");
    }
    raw.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn migrates_scene_id_to_context_id() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let chat = fsutil::meta_root(root).join("chat");
        fs::create_dir_all(&chat).unwrap();
        let raw = r#"{
            "id": "legacy-1",
            "title": "Old",
            "scene_id": "sc-42",
            "updated_at": "2026-01-01 12:00",
            "messages": [],
            "draft": ""
        }"#;
        fs::write(chat.join("legacy-1.json"), raw).unwrap();
        let c = load_conversation(root, "legacy-1").expect("load");
        assert_eq!(c.context_id, "scene:sc-42");
        assert!(c.scene_id.is_none());
    }

    #[test]
    fn list_for_context_filters() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut a = new_conversation("scene:a".into());
        a.id = "ca".into();
        a.title = "A".into();
        save_conversation(root, &a).unwrap();
        let mut b = new_conversation("scene:b".into());
        b.id = "cb".into();
        b.title = "B".into();
        save_conversation(root, &b).unwrap();
        let list = list_for_context(root, "scene:a");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "ca");
    }

    #[test]
    fn last_or_empty_tracks_active_by_context() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut first = new_conversation("scene:x".into());
        first.id = "c1".into();
        first.title = "First".into();
        save_conversation(root, &first).unwrap();
        let mut second = new_conversation("scene:x".into());
        second.id = "c2".into();
        second.title = "Second".into();
        save_conversation(root, &second).unwrap();
        let loaded = last_or_empty(root, "scene:x");
        assert_eq!(loaded.id, "c2");
        let empty = last_or_empty(root, "scene:missing");
        assert!(empty.messages.is_empty());
        assert_eq!(empty.context_id, "scene:missing");
    }

    #[test]
    fn format_stamp_today_other_day_and_invalid() {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(format_stamp(&format!("{today} 14:30")), "14:30");
        assert_eq!(format_stamp("2020-01-15 09:05"), "15/01 09:05");
        assert_eq!(format_stamp("SystemTime { tv_sec: 1, tv_nsec: 0 }"), "—");
    }

    #[test]
    fn composer_mode_helpers() {
        assert!(ComposerMode::Chat.is_chat());
        assert!(!ComposerMode::continue_mode("continue").is_chat());
        assert!(!ComposerMode::brush("improve").is_chat());
        assert_eq!(ComposerMode::Chat.label(&[]), "Chat");
        assert_eq!(
            ComposerMode::continue_mode("sentence").label(&[]),
            "Continuer (phrase)"
        );
        assert_eq!(
            ComposerMode::continue_mode("paragraph").label(&[]),
            "Continuer (paragraphe)"
        );
        assert_eq!(
            ComposerMode::continue_mode("generate").label(&[]),
            "Générer"
        );
        assert_eq!(
            ComposerMode::continue_mode("continue").label(&[]),
            "Continuer"
        );
        assert_eq!(ComposerMode::brush("unknown").label(&[]), "Outil");
        assert_eq!(
            ComposerMode::continue_mode("sentence"),
            ComposerMode::Continue {
                mode: "sentence".into()
            }
        );
        assert_eq!(
            ComposerMode::brush("improve"),
            ComposerMode::Brush {
                id: "improve".into()
            }
        );
    }

    #[test]
    fn context_id_for_all_nav_variants() {
        assert_eq!(
            context_id_for(&NavSelection::Scene("sc1".into())),
            "scene:sc1"
        );
        assert_eq!(context_id_for(&NavSelection::Note("n1".into())), "note:n1");
        assert_eq!(context_id_for(&NavSelection::Styles), "styles");
        assert_eq!(context_id_for(&NavSelection::Publish), "publish");
    }
}
