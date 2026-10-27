//! Coherence reviews and content ideas.

use std::path::{Path, PathBuf};

use ak_core::fsutil::{meta_root, write_atomic};
use ak_core::{Project, Scene};
use ak_llm::{ChatMessage, GenerationRequest, LlmContext, OpenAiCompatClient};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Reviews
// ---------------------------------------------------------------------------

/// Triage state of a review item (set by the author, never by the model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ReviewStatus {
    #[default]
    #[serde(rename = "todo")]
    ToDo,
    #[serde(rename = "done")]
    Done,
    #[serde(rename = "ignored")]
    Ignored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    /// Local identifier (never produced by the model).
    #[serde(default)]
    pub id: String,
    /// Scene this item was raised for (empty when the review had no scene).
    #[serde(default)]
    pub scene_id: String,
    /// `info` | `warn` | `error`.
    pub severity: String,
    pub title: String,
    pub detail: String,
    /// Proposed fix, may be empty.
    #[serde(default)]
    pub suggestion: String,
    /// Verbatim excerpt of the scene the item refers to, if the model gave one
    /// that really occurs in the text.
    #[serde(default)]
    pub passage: Option<String>,
    #[serde(default)]
    pub status: ReviewStatus,
    /// True when the scene text changed since this review was produced.
    #[serde(default)]
    pub stale: bool,
}

impl ReviewItem {
    /// Stable identity used to carry statuses over when a scene is re-reviewed.
    fn fingerprint(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}",
            self.title.trim().to_lowercase(),
            self.passage.as_deref().unwrap_or("").trim(),
            self.severity
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewReport {
    pub items: Vec<ReviewItem>,
    /// Raw model answer, set locally (never part of the model schema).
    pub raw: String,
    #[serde(default)]
    pub scene_id: String,
    /// Fingerprint of the reviewed scene text (see [`text_fingerprint`]).
    #[serde(default)]
    pub text_hash: String,
    /// RFC 3339 creation time.
    #[serde(default)]
    pub created_at: String,
}

impl ReviewReport {
    /// True if `current_text` differs from the text that was reviewed.
    pub fn is_stale_for(&self, current_text: &str) -> bool {
        !self.text_hash.is_empty() && self.text_hash != text_fingerprint(current_text)
    }

    /// Recompute `stale` on every item against the current scene text.
    /// Returns the report-level staleness. Reverting the text clears it again.
    pub fn refresh_stale(&mut self, current_text: &str) -> bool {
        let stale = self.is_stale_for(current_text);
        for it in &mut self.items {
            it.stale = stale;
        }
        stale
    }

    pub fn set_status(&mut self, item_id: &str, status: ReviewStatus) -> bool {
        match self.items.iter_mut().find(|i| i.id == item_id) {
            Some(it) => {
                it.status = status;
                true
            }
            None => false,
        }
    }

    /// Keep Done/Ignored decisions from `previous` for items that match.
    pub fn inherit_statuses(&mut self, previous: &ReviewReport) {
        for it in &mut self.items {
            let fp = it.fingerprint();
            if let Some(old) = previous
                .items
                .iter()
                .find(|o| o.status != ReviewStatus::ToDo && o.fingerprint() == fp)
            {
                it.status = old.status;
            }
        }
    }
}

/// Stable (cross-version) FNV-1a 64 fingerprint of a text, hex encoded.
/// Leading/trailing whitespace is ignored.
pub fn text_fingerprint(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.trim().as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Why a review could not be produced or stored.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("Réponse vide du modèle : aucune revue produite.")]
    EmptyResponse,
    #[error("Réponse du modèle illisible (JSON invalide : {reason}). Début de la réponse : « {excerpt} »")]
    InvalidJson { reason: String, excerpt: String },
    #[error("Stockage des revues : {0}")]
    Storage(String),
}

/// What the model is allowed to return. `raw`, ids, status, scene and
/// staleness are deliberately absent: they are set locally.
#[derive(Debug, Deserialize)]
struct LlmReviewResponse {
    items: Vec<LlmReviewItem>,
}

#[derive(Debug, Deserialize)]
struct LlmReviewItem {
    #[serde(default)]
    severity: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    suggestion: String,
    #[serde(default)]
    passage: Option<String>,
}

/// Parse a model answer into a [`ReviewReport`].
///
/// * blank answer → [`ReviewError::EmptyResponse`]
/// * not JSON / wrong shape → [`ReviewError::InvalidJson`]
/// * `{"items": []}` → a valid, empty report (the model found nothing)
///
/// `scene_text` is the reviewed text: it is fingerprinted for staleness and
/// used to drop `passage` values that do not occur verbatim in it.
pub fn parse_review_response(
    raw: &str,
    scene_id: &str,
    scene_text: &str,
) -> Result<ReviewReport, ReviewError> {
    if raw.trim().is_empty() {
        return Err(ReviewError::EmptyResponse);
    }
    let json = extract_json(raw);
    let parsed: LlmReviewResponse = match serde_json::from_str::<LlmReviewResponse>(&json) {
        Ok(r) => r,
        Err(first) => match serde_json::from_str::<Vec<LlmReviewItem>>(&json) {
            Ok(items) => LlmReviewResponse { items },
            Err(_) => {
                return Err(ReviewError::InvalidJson {
                    reason: first.to_string(),
                    excerpt: excerpt_for_error(raw),
                })
            }
        },
    };

    let items = parsed
        .items
        .into_iter()
        .filter(|i| !(i.title.trim().is_empty() && i.detail.trim().is_empty()))
        .map(|i| {
            let passage = i
                .passage
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty() && scene_text.contains(p.as_str()));
            let title = if i.title.trim().is_empty() {
                "Remarque".to_string()
            } else {
                i.title.trim().to_string()
            };
            ReviewItem {
                id: uuid::Uuid::new_v4().to_string(),
                scene_id: scene_id.to_string(),
                severity: normalize_severity(&i.severity),
                title,
                detail: i.detail.trim().to_string(),
                suggestion: i.suggestion.trim().to_string(),
                passage,
                status: ReviewStatus::ToDo,
                stale: false,
            }
        })
        .collect();

    Ok(ReviewReport {
        items,
        raw: raw.to_string(),
        scene_id: scene_id.to_string(),
        text_hash: text_fingerprint(scene_text),
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn normalize_severity(s: &str) -> String {
    match s.trim().to_lowercase().as_str() {
        "error" | "erreur" | "critical" | "critique" => "error",
        "warn" | "warning" | "avertissement" => "warn",
        _ => "info",
    }
    .to_string()
}

fn excerpt_for_error(raw: &str) -> String {
    let flat: String = raw
        .trim()
        .chars()
        .take(200)
        .collect::<String>()
        .replace('\n', " ");
    ak_llm::redact_secrets(&flat, None)
}

/// Extract the JSON payload of a model answer: fenced block (with or without a
/// language tag, possibly surrounded by prose), else the outermost `{…}`.
fn extract_json(raw: &str) -> String {
    let t = raw.trim();
    if let Some(open) = t.find("```") {
        let after = &t[open + 3..];
        let body_start = match after.find('\n') {
            Some(nl) if !after[..nl].contains(['{', '[']) => nl + 1,
            _ => 0,
        };
        let body = &after[body_start..];
        let end = body.find("```").unwrap_or(body.len());
        let candidate = body[..end].trim();
        if candidate.starts_with('{') || candidate.starts_with('[') {
            return candidate.to_string();
        }
    }
    if t.starts_with('{') || t.starts_with('[') {
        return t.to_string();
    }
    if let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) {
        if b > a {
            return t[a..=b].to_string();
        }
    }
    t.to_string()
}

pub fn run_review(client: &OpenAiCompatClient, ctx: &LlmContext) -> ak_llm::Result<ReviewReport> {
    let system = ctx.system_prompt();
    let user = "Fais une revue de cohérence locale de la scène / draft fourni.\n\
         Cherche: contradictions avec les Notes, trous de motivation, problèmes de voix, \
         anachronismes internes, détails oubliés.\n\
         Réponds en JSON strict avec la forme:\n\
         {\"items\":[{\"severity\":\"info|warn|error\",\"title\":\"...\",\"detail\":\"...\",\
         \"suggestion\":\"correction proposée\",\"passage\":\"extrait EXACT du draft concerné (optionnel)\"}]}\n\
         Si tout est cohérent, réponds {\"items\":[]}.\n\
         Sans markdown, sans commentaire hors JSON.";
    let raw = client.complete(&GenerationRequest {
        system,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user.into(),
        }],
    })?;
    parse_review_response(&raw, &ctx.scene_id, &ctx.draft_body)
        .map_err(|e| ak_llm::Error::Msg(e.to_string()))
}

// --- persistence: `{project}/.authorkit-rust/reviews/{scene}.json` ----------

const REVIEWS_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct StoredReview {
    format_version: u32,
    #[serde(flatten)]
    report: ReviewReport,
}

pub fn reviews_dir(project_root: &Path) -> PathBuf {
    meta_root(project_root).join("reviews")
}

fn review_path(project_root: &Path, scene_id: &str) -> PathBuf {
    let safe: String = scene_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let name = if safe.is_empty() {
        "project".to_string()
    } else {
        safe
    };
    reviews_dir(project_root).join(format!("{name}.json"))
}

/// Save the latest review of a scene (atomic write). If a previous review of
/// the same scene exists, Done/Ignored decisions on matching items are kept.
pub fn save_review(project_root: &Path, report: &mut ReviewReport) -> Result<PathBuf, ReviewError> {
    if let Ok(Some(prev)) = load_review(project_root, &report.scene_id) {
        report.inherit_statuses(&prev);
    }
    persist_review(project_root, report)
}

/// Save as-is (e.g. after the author changed an item status).
pub fn persist_review(project_root: &Path, report: &ReviewReport) -> Result<PathBuf, ReviewError> {
    let path = review_path(project_root, &report.scene_id);
    let stored = StoredReview {
        format_version: REVIEWS_FORMAT_VERSION,
        report: report.clone(),
    };
    let bytes =
        serde_json::to_vec_pretty(&stored).map_err(|e| ReviewError::Storage(e.to_string()))?;
    write_atomic(&path, bytes).map_err(|e| ReviewError::Storage(e.to_string()))?;
    Ok(path)
}

/// Load the stored review of a scene. `Ok(None)` when none exists.
pub fn load_review(
    project_root: &Path,
    scene_id: &str,
) -> Result<Option<ReviewReport>, ReviewError> {
    let path = review_path(project_root, scene_id);
    let raw = match std::fs::read_to_string(&path) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ReviewError::Storage(e.to_string())),
    };
    let stored: StoredReview = serde_json::from_str(&raw)
        .map_err(|e| ReviewError::Storage(format!("{} illisible : {e}", path.display())))?;
    Ok(Some(stored.report))
}

/// Load a stored review and mark it stale against the current scene text.
pub fn load_review_for_text(
    project_root: &Path,
    scene_id: &str,
    current_text: &str,
) -> Result<Option<ReviewReport>, ReviewError> {
    Ok(load_review(project_root, scene_id)?.map(|mut r| {
        r.refresh_stale(current_text);
        r
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIdea {
    pub title: String,
    pub pitch: String,
}

pub fn content_ideas(
    client: &OpenAiCompatClient,
    ctx: &LlmContext,
) -> ak_llm::Result<Vec<ContentIdea>> {
    let system = ctx.system_prompt();
    let user = "Propose 5 idées concrètes pour la suite de cette scène / chapitre.\n\
         JSON strict: {\"ideas\":[{\"title\":\"...\",\"pitch\":\"...\"}]}\n\
         Sans markdown hors JSON.";
    let raw = client.complete(&GenerationRequest {
        system,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user.into(),
        }],
    })?;
    let cleaned = strip_code_fence(&raw);
    #[derive(Deserialize)]
    struct Wrap {
        ideas: Vec<ContentIdea>,
    }
    match serde_json::from_str::<Wrap>(&cleaned) {
        Ok(w) => Ok(w.ideas),
        Err(_) => Ok(vec![ContentIdea {
            title: "Suite".into(),
            pitch: raw,
        }]),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlueprintCharacter {
    pub name: String,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlueprintChapter {
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NovelBlueprint {
    pub title: String,
    pub premise: String,
    pub characters: Vec<BlueprintCharacter>,
    pub chapters: Vec<BlueprintChapter>,
    pub chapter1_preview: String,
}

/// System + user prompts for blueprint generation (usable by the UI worker).
pub fn blueprint_prompts(idea: &str) -> (String, String) {
    let system =
        "Tu es un architecte de romans. Réponds en français. JSON strict uniquement.".into();
    let user = format!(
        "À partir de cette idée de roman, génère un blueprint:\n\n{idea}\n\n\
         JSON:\n{{\n  \"title\": \"...\",\n  \"premise\": \"...\",\n  \
         \"characters\":[{{\"name\":\"...\",\"summary\":\"...\"}}],\n  \
         \"chapters\":[{{\"title\":\"...\",\"summary\":\"...\"}}],\n  \
         \"chapter1_preview\": \"texte prose du début du ch.1\"\n}}"
    );
    (system, user)
}

/// Parse a blueprint JSON reply (optionally fenced).
pub fn parse_blueprint(raw: &str) -> ak_llm::Result<NovelBlueprint> {
    let cleaned = strip_code_fence(raw);
    serde_json::from_str(&cleaned)
        .map_err(|e| ak_llm::Error::Msg(format!("Blueprint JSON invalide: {e}\n{raw}")))
}

/// Generate a novel blueprint without touching the manuscript.
pub fn generate_blueprint(
    client: &OpenAiCompatClient,
    idea: &str,
) -> ak_llm::Result<NovelBlueprint> {
    let (system, user) = blueprint_prompts(idea);
    let raw = client.complete(&GenerationRequest {
        system,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user,
        }],
    })?;
    parse_blueprint(&raw)
}

/// Apply a blueprint to an empty-ish project. Caller must have snapshotted / confirmed.
pub fn apply_blueprint(project: &mut Project, bp: &NovelBlueprint) -> ak_llm::Result<String> {
    // Soft-trash existing acts before rebuild so recovery is possible.
    let act_ids: Vec<String> = project
        .manuscript
        .index
        .acts
        .iter()
        .map(|a| a.id.clone())
        .collect();
    for aid in act_ids {
        let _ = project.manuscript.trash_act(&aid, &project.root);
    }

    project.meta.title = bp.title.clone();
    project.meta.premise = bp.premise.clone();
    let _ = project.save_meta();

    project.manuscript.index.acts.clear();
    project.manuscript.scenes.clear();
    let act_id = project.manuscript.add_act("Acte I");
    for (i, ch) in bp.chapters.iter().enumerate() {
        let ch_id = project
            .manuscript
            .add_chapter(&act_id, &ch.title)
            .map_err(|e| ak_llm::Error::Msg(e.to_string()))?;
        let scene_title = if i == 0 {
            "Ouverture".to_string()
        } else {
            format!("Scène — {}", ch.title)
        };
        let body = if i == 0 {
            bp.chapter1_preview.clone()
        } else {
            format!("<!-- Résumé: {} -->\n\n", ch.summary)
        };
        let scene = project
            .manuscript
            .add_scene(&project.root, &act_id, &ch_id, &scene_title)
            .map_err(|e| ak_llm::Error::Msg(e.to_string()))?;
        let sid = scene.meta.id.clone();
        if let Some(s) = project.manuscript.get_scene_mut(&sid) {
            s.body = body;
            let s = s.clone();
            let _ = project.manuscript.save_scene(&s);
        }
    }
    let _ = project.manuscript.save_index(&project.root);

    for c in &bp.characters {
        let note = project
            .notes
            .create(&project.root, ak_core::NoteKind::Character, &c.name)
            .map_err(|e| ak_llm::Error::Msg(e.to_string()))?;
        let id = note.meta.id.clone();
        if let Some(n) = project.notes.get_mut(&id) {
            n.body = format!(
                "# Résumé\n\n{}\n\n# Apparence\n\n\n# Personnalité\n\n",
                c.summary
            );
            let n = n.clone();
            let _ = project.notes.save_note(&n);
        }
    }

    Ok(format!(
        "Roman « {} » généré: {} chapitres, {} personnages.",
        project.meta.title,
        bp.chapters.len(),
        project.notes.by_kind(ak_core::NoteKind::Character).count()
    ))
}

/// Bootstrap a novel into `project` (destructive). Prefer generate + apply with preview.
pub fn bootstrap_novel(
    client: &OpenAiCompatClient,
    project: &mut Project,
    idea: &str,
) -> ak_llm::Result<String> {
    let bp = generate_blueprint(client, idea)?;
    apply_blueprint(project, &bp)
}

pub fn scene_context(project: &Project, scene: &Scene) -> LlmContext {
    let notes: Vec<_> = scene
        .meta
        .links
        .iter()
        .filter_map(|id| project.notes.get(id))
        .collect();
    LlmContext::from_project_scene(project, scene, &notes)
}

fn strip_code_fence(s: &str) -> String {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```") {
        let rest = rest.strip_prefix("json").unwrap_or(rest);
        if let Some(end) = rest.rfind("```") {
            return rest[..end].trim().to_string();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: &str = "Marie ouvrit la porte. « Tu es en retard », dit Paul.\n\nElle sourit.";

    const VALID: &str = r#"{"items":[
        {"severity":"warn","title":"Voix de Paul","detail":"Paul vouvoie ailleurs.",
         "suggestion":"Utiliser « vous ».","passage":"« Tu es en retard »"},
        {"severity":"CRITICAL","title":"Contradiction","detail":"Marie est orpheline dans la fiche."}
    ]}"#;

    #[test]
    fn valid_json() {
        let r = parse_review_response(VALID, "scene-1", SCENE).unwrap();
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.scene_id, "scene-1");
        assert_eq!(r.raw, VALID, "raw is set locally");
        let a = &r.items[0];
        assert_eq!(a.severity, "warn");
        assert_eq!(a.suggestion, "Utiliser « vous ».");
        assert_eq!(a.passage.as_deref(), Some("« Tu es en retard »"));
        assert_eq!(a.status, ReviewStatus::ToDo);
        assert_eq!(a.scene_id, "scene-1");
        assert!(!a.stale);
        assert!(!a.id.is_empty());
        let b = &r.items[1];
        assert_eq!(b.severity, "error", "critical normalised");
        assert_eq!(b.passage, None);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn model_cannot_inject_local_fields() {
        let raw = r#"{"raw":"evil","items":[{"title":"t","detail":"d","status":"done",
            "id":"hijack","stale":true,"scene_id":"other"}]}"#;
        let r = parse_review_response(raw, "s", SCENE).unwrap();
        assert_eq!(r.raw, raw);
        let it = &r.items[0];
        assert_eq!(it.status, ReviewStatus::ToDo);
        assert_ne!(it.id, "hijack");
        assert!(!it.stale);
        assert_eq!(it.scene_id, "s");
    }

    #[test]
    fn fenced_json() {
        for wrapped in [
            format!("```json\n{VALID}\n```"),
            format!("```\n{VALID}\n```"),
            format!("Voici la revue :\n```json\n{VALID}\n```\nBonne continuation !"),
            format!("Voici la revue : {VALID} fin."),
        ] {
            let r = parse_review_response(&wrapped, "s", SCENE)
                .unwrap_or_else(|e| panic!("{wrapped}: {e}"));
            assert_eq!(r.items.len(), 2, "{wrapped}");
            assert_eq!(r.raw, wrapped);
        }
    }

    #[test]
    fn invalid_is_an_explicit_error() {
        for bad in [
            "Tout va bien, rien à signaler.",
            "{\"items\": [",
            "{\"foo\": 1}",
            "```json\nnope\n```",
        ] {
            match parse_review_response(bad, "s", SCENE) {
                Err(ReviewError::InvalidJson { excerpt, .. }) => assert!(!excerpt.is_empty()),
                other => panic!("{bad}: expected InvalidJson, got {other:?}"),
            }
        }
    }

    #[test]
    fn blank_response_is_empty_error() {
        for blank in ["", "   ", "\n\t\n"] {
            assert!(matches!(
                parse_review_response(blank, "s", SCENE),
                Err(ReviewError::EmptyResponse)
            ));
        }
        assert!(ReviewError::EmptyResponse.to_string().contains("vide"));
    }

    #[test]
    fn empty_items_is_a_valid_empty_report() {
        let r = parse_review_response(r#"{"items":[]}"#, "s", SCENE).unwrap();
        assert!(r.items.is_empty());
        assert_eq!(r.raw, r#"{"items":[]}"#);
        // Blank-only items are dropped, not surfaced as noise.
        let r =
            parse_review_response(r#"{"items":[{"title":" ","detail":""}]}"#, "s", SCENE).unwrap();
        assert!(r.items.is_empty());
        // Bare array is tolerated.
        let r = parse_review_response(r#"[{"title":"t","detail":"d"}]"#, "s", SCENE).unwrap();
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].severity, "info");
    }

    #[test]
    fn hallucinated_passage_is_dropped() {
        let raw = r#"{"items":[{"title":"t","detail":"d","passage":"phrase inexistante"}]}"#;
        let r = parse_review_response(raw, "s", SCENE).unwrap();
        assert_eq!(r.items[0].passage, None);
    }

    #[test]
    fn stale_follows_text_changes() {
        let mut r = parse_review_response(VALID, "s", SCENE).unwrap();
        assert!(!r.refresh_stale(SCENE));
        assert!(r.items.iter().all(|i| !i.stale));
        // Surrounding whitespace is not a change.
        assert!(!r.refresh_stale(&format!("\n{SCENE}  \n")));
        let edited = SCENE.replace("Paul", "Pierre");
        assert!(r.refresh_stale(&edited));
        assert!(r.items.iter().all(|i| i.stale));
        // Reverting the edit clears staleness.
        assert!(!r.refresh_stale(SCENE));
        assert!(r.items.iter().all(|i| !i.stale));
    }

    #[test]
    fn fingerprint_is_stable() {
        // Pinned: persisted reports depend on this value never changing.
        assert_eq!(text_fingerprint("abc"), "e71fa2190541574b");
        assert_eq!(text_fingerprint("  abc\n"), text_fingerprint("abc"));
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ak-reviews-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn persistence_roundtrip_and_status_carry_over() {
        let root = tmp();
        let mut r = parse_review_response(VALID, "scene-1", SCENE).unwrap();
        let path = save_review(&root, &mut r).unwrap();
        assert!(path.starts_with(root.join(".authorkit-rust").join("reviews")));
        assert!(path.exists());

        let mut loaded = load_review(&root, "scene-1").unwrap().unwrap();
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(loaded.items[0].id, r.items[0].id);
        assert_eq!(loaded.items[0].suggestion, "Utiliser « vous ».");
        assert_eq!(loaded.raw, VALID);

        // Author triage is persisted…
        let first = loaded.items[0].id.clone();
        assert!(loaded.set_status(&first, ReviewStatus::Ignored));
        assert!(!loaded.set_status("nope", ReviewStatus::Done));
        persist_review(&root, &loaded).unwrap();
        assert_eq!(
            load_review(&root, "scene-1").unwrap().unwrap().items[0].status,
            ReviewStatus::Ignored
        );

        // …and survives a re-review that raises the same item again.
        let mut again = parse_review_response(VALID, "scene-1", SCENE).unwrap();
        save_review(&root, &mut again).unwrap();
        assert_eq!(again.items[0].status, ReviewStatus::Ignored);
        assert_eq!(again.items[1].status, ReviewStatus::ToDo);

        // Staleness on load.
        let fresh = load_review_for_text(&root, "scene-1", SCENE)
            .unwrap()
            .unwrap();
        assert!(fresh.items.iter().all(|i| !i.stale));
        let stale = load_review_for_text(&root, "scene-1", "autre texte")
            .unwrap()
            .unwrap();
        assert!(stale.items.iter().all(|i| i.stale));

        assert!(load_review(&root, "missing").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scene_ids_cannot_escape_reviews_dir() {
        let root = tmp();
        let mut r = parse_review_response(VALID, "../../evil", SCENE).unwrap();
        let path = save_review(&root, &mut r).unwrap();
        assert_eq!(path.parent().unwrap(), reviews_dir(&root));
        let mut r = parse_review_response(VALID, "", SCENE).unwrap();
        let path = save_review(&root, &mut r).unwrap();
        assert_eq!(path.file_name().unwrap(), "project.json");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corrupt_stored_review_is_an_error_not_a_panic() {
        let root = tmp();
        let dir = reviews_dir(&root);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s.json"), "{not json").unwrap();
        assert!(matches!(
            load_review(&root, "s"),
            Err(ReviewError::Storage(_))
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn blueprint_prompts_contain_idea_and_json_keys() {
        let idea = "Un détective amnésique à Marseille";
        let (system, user) = blueprint_prompts(idea);
        assert!(system.contains("JSON"));
        assert!(user.contains(idea));
        for key in [
            "\"title\"",
            "\"premise\"",
            "\"characters\"",
            "\"chapters\"",
            "\"chapter1_preview\"",
        ] {
            assert!(user.contains(key), "missing key {key} in user prompt");
        }
    }

    const BLUEPRINT_JSON: &str = r#"{
        "title": "La Porte Oubliée",
        "premise": "Une clé ouvre des portes vers des souvenirs volés.",
        "characters": [
            {"name": "Léa", "summary": "Archiviste curieuse."},
            {"name": "Marc", "summary": "Gardien silencieux."}
        ],
        "chapters": [
            {"title": "L'entrée", "summary": "Léa trouve la clé."},
            {"title": "Le passage", "summary": "Marc révèle le secret."}
        ],
        "chapter1_preview": "Léa glissa la clé dans la serrure rouillée."
    }"#;

    #[test]
    fn parse_blueprint_raw_and_fenced() {
        let bp = parse_blueprint(BLUEPRINT_JSON).unwrap();
        assert_eq!(bp.title, "La Porte Oubliée");
        assert_eq!(bp.characters.len(), 2);
        assert_eq!(bp.chapters.len(), 2);
        assert!(bp.chapter1_preview.contains("clé"));

        let fenced = format!("```json\n{BLUEPRINT_JSON}\n```");
        let bp2 = parse_blueprint(&fenced).unwrap();
        assert_eq!(bp2.title, bp.title);
        assert_eq!(bp2.premise, bp.premise);
    }

    #[test]
    fn parse_blueprint_invalid_is_err() {
        assert!(parse_blueprint("not json").is_err());
        assert!(parse_blueprint(r#"{"title":1}"#).is_err());
        assert!(parse_blueprint("```json\nnope\n```").is_err());
    }

    #[test]
    fn apply_blueprint_builds_project_from_tempfile() {
        let dir = std::env::temp_dir().join(format!("ak-blueprint-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut project = Project::create(&dir, "Placeholder", "Auteur").expect("create");
        let bp = parse_blueprint(BLUEPRINT_JSON).unwrap();
        let msg = apply_blueprint(&mut project, &bp).unwrap();
        assert!(msg.contains("La Porte Oubliée"));
        assert!(msg.contains("2 chapitres"));

        assert_eq!(project.meta.title, "La Porte Oubliée");
        assert_eq!(
            project.meta.premise,
            "Une clé ouvre des portes vers des souvenirs volés."
        );

        let chapters: Vec<_> = project
            .manuscript
            .index
            .acts
            .iter()
            .flat_map(|a| a.chapters.iter())
            .collect();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "L'entrée");
        assert_eq!(chapters[1].title, "Le passage");

        let first_scene_id = &chapters[0].scene_ids[0];
        let first = project.manuscript.get_scene(first_scene_id).unwrap();
        assert_eq!(first.body, "Léa glissa la clé dans la serrure rouillée.");

        let chars: Vec<_> = project
            .notes
            .by_kind(ak_core::NoteKind::Character)
            .collect();
        assert_eq!(chars.len(), 2);
        let lea = chars.iter().find(|n| n.meta.name == "Léa").unwrap();
        assert!(lea.body.contains("Archiviste curieuse."));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scene_context_pulls_linked_notes_and_premise() {
        let dir = std::env::temp_dir().join(format!("ak-scene-ctx-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut project = Project::create(&dir, "Ctx", "Auteur").unwrap();
        project.meta.premise = "Paris sous la pluie.".into();
        let note_id = project
            .notes
            .create(&dir, ak_core::NoteKind::Character, "Alice")
            .unwrap()
            .meta
            .id
            .clone();
        {
            let n = project.notes.get_mut(&note_id).unwrap();
            n.body = "Détective sceptique.".into();
            let n = n.clone();
            project.notes.save_note(&n).unwrap();
        }
        let scene_id = project.manuscript.ordered_scenes()[0].meta.id.clone();
        project
            .manuscript
            .link_note(&scene_id, &note_id, &dir)
            .unwrap();
        // Dangling link is ignored by scene_context (filter_map).
        {
            let s = project.manuscript.get_scene_mut(&scene_id).unwrap();
            s.meta.links.push("gone".into());
            s.body = "Alice ouvrit le dossier.".into();
            s.meta.title = "Ouverture".into();
            let s = s.clone();
            project.manuscript.save_scene(&s).unwrap();
        }

        let scene = project.manuscript.get_scene(&scene_id).unwrap();
        let ctx = scene_context(&project, scene);
        assert_eq!(ctx.scene_id, scene_id);
        assert_eq!(ctx.draft_title, "Ouverture");
        assert_eq!(ctx.draft_body, "Alice ouvrit le dossier.");
        assert_eq!(ctx.premise, "Paris sous la pluie.");
        assert_eq!(ctx.notes.len(), 1);
        assert_eq!(ctx.notes[0].id, note_id);
        assert!(ctx.notes[0].title.contains("Alice"));
        assert!(ctx.notes[0].text.contains("Détective"));
        assert!(!ctx.system_prompt().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
