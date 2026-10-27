use std::collections::HashMap;
use std::path::Path;

use ak_coherence::{
    apply_blueprint, blueprint_prompts, load_review_for_text, parse_blueprint,
    parse_review_response, persist_review, scene_context, ContentIdea, NovelBlueprint,
};
use ak_core::{NoteKind, Project};
use ak_export::{export_docx_with, export_epub_with, export_markdown_with, ExportOptions};
use ak_llm::{
    budget_history, builtin_brushes, AssembledContext, ChatMessage, ContinueMode, JobIds, JobKind,
    JobPurpose, LlmEvent, LlmJob, LlmSettings, LlmWorker, OpenAiCompatClient, ProviderKind,
    RequestBudget,
};
use eframe::egui::{self, CentralPanel, Panel, ViewportCommand};
use egui::RichText;
use egui_commonmark::CommonMarkCache;
use shadcn_egui::components::button::{Button, ButtonSize, ButtonVariant};
use shadcn_egui::components::input::Input;
use shadcn_egui::components::label::Label;
use shadcn_egui::Theme;

use crate::brand::BrandAssets;
use crate::i18n::Lang;
use crate::panels::editor::{self, EditorAction, EditorKind, EditorState, SaveStatus};
use crate::panels::nav::{self, NavAction, NavSelection};
use crate::panels::onboarding::{self, OnboardingAction, OnboardingState};
use crate::panels::sidebar::{self, ChatMsg, MsgStatus, SidebarAction, SidebarState};
use crate::panels::welcome;
use crate::providers;
use crate::theme::{self, ThemeMode, UiPrefs, FONT_TITLE, FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

/// Left nav: keep room for indented scene titles without crushing action icons.
const NAV_WIDTH_MIN: f32 = 280.0;
const NAV_WIDTH_MAX: f32 = 420.0;

#[derive(Debug, Clone)]
enum RenameTarget {
    Act { id: String },
    Chapter { act_id: String, id: String },
    Scene { id: String },
    Note { id: String },
}

#[derive(Debug, Clone)]
enum PendingCreate {
    Scene { act_id: String, chapter_id: String },
    Note { kind: NoteKind },
}

/// Keeps modal payload alive through the exit fade.
#[derive(Debug, Clone)]
struct OpenModal<T> {
    data: T,
    open: bool,
}

impl<T> OpenModal<T> {
    fn new(data: T) -> Self {
        Self { data, open: true }
    }
}

#[derive(Debug, Clone)]
enum PendingDelete {
    Act {
        id: String,
        title: String,
    },
    Chapter {
        act_id: String,
        id: String,
        title: String,
    },
    Scene {
        id: String,
        title: String,
    },
    Note {
        id: String,
        title: String,
    },
}

impl PendingDelete {
    fn title(&self) -> &str {
        match self {
            Self::Act { title, .. }
            | Self::Chapter { title, .. }
            | Self::Scene { title, .. }
            | Self::Note { title, .. } => title,
        }
    }

    fn kind_label(&self, lang: crate::i18n::Lang) -> &'static str {
        match self {
            Self::Act { .. } => lang.kind_act(),
            Self::Chapter { .. } => lang.kind_chapter(),
            Self::Scene { .. } => lang.kind_scene(),
            Self::Note { .. } => lang.kind_note(),
        }
    }
}

pub struct AuthorKitApp {
    brand: BrandAssets,
    provider_icons: providers::ProviderIcons,
    pub project: Option<Project>,
    pub selection: NavSelection,
    pub editor: EditorState,
    pub sidebar: SidebarState,
    prefs: UiPrefs,
    llm: LlmSettings,
    api_key_draft: String,
    /// When true, show the editable API-key field instead of the masked chip.
    api_key_editing: bool,
    brushes: Vec<ak_llm::Brush>,
    /// Non-blocking status (errors / info) — never a confirm dialog.
    toast: String,
    toast_until: f64,
    bootstrap_open: bool,
    bootstrap_idea: String,
    /// Preview before applying a generated blueprint.
    blueprint_preview: Option<NovelBlueprint>,
    /// If true, apply blueprint to current project (after confirm); else create new.
    blueprint_apply_existing: bool,
    settings_model: String,
    settings_base: String,
    onboarding: OnboardingState,
    settings_open: bool,
    settings_tab: usize,
    styles_open: bool,
    publish_open: bool,
    rename: Option<OpenModal<(RenameTarget, String)>>,
    create: Option<OpenModal<(PendingCreate, String)>>,
    pending_delete: Option<OpenModal<PendingDelete>>,
    md_cache: CommonMarkCache,
    /// Open diagnostics shown once after project open.
    open_diagnostics: Vec<String>,
    versions_open: bool,
    trash_open: bool,
    /// External conflict: path changed on disk since load.
    external_conflict: Option<String>,
    llm_worker: LlmWorker,
    /// Bumps when editor body changes — stale LLM replies check this.
    text_version: u64,
    pending_clipboard: Option<String>,
    search_open: bool,
    search_query: String,
    search_hits: Vec<crate::ms_tools::SearchHit>,
    import_open: bool,
    import_preview: Option<crate::ms_tools::ImportPreview>,
    read_through_open: bool,
    read_through_text: String,
    pending_sidebar_job: Option<PendingSidebarJob>,
    /// In-flight synopsis generation job id (routes Edit events away from AI float).
    synopsis_job_id: Option<String>,
    /// Conversations with an in-flight title job (avoid duplicate requests).
    title_pending: std::collections::HashSet<String>,
    /// Blueprint generation on worker thread.
    blueprint_job_id: Option<String>,
    close_block: Option<String>,
    close_error_open: bool,
    pending_replies: HashMap<String, String>,
    export_options: ExportOptions,
    conflict_modal_open: bool,
    move_target: Option<OpenModal<MoveTarget>>,
    /// Last auto snapshot time (wall clock) for open document.
    last_version_snapshot: Option<f64>,
    review_scene_loaded: Option<String>,
    review_debounce_version: u64,
    /// True while dragging the left-nav resize handle (not any click inside the panel).
    nav_resizing: bool,
    /// True while dragging the right-sidebar resize handle.
    sidebar_resizing: bool,
}

#[derive(Debug, Clone)]
struct PreparedRequest {
    system: String,
    history: Vec<ChatMessage>,
    assembled: AssembledContext,
    history_omitted: usize,
}

#[derive(Debug, Clone)]
enum MoveTarget {
    Scene { id: String },
    Chapter { act_id: String, id: String },
}

/// Re-export for tests and resolve_anchor.
pub use crate::chat_store::EditApplyMode as AiApplyMode;

#[derive(Debug, Clone)]
enum PendingSidebarJob {
    Ideas,
    Review { scene_id: String, draft: String },
}

impl AuthorKitApp {
    pub fn new(ctx: &egui::Context) -> Self {
        LlmSettings::ensure_dir();
        let prefs = UiPrefs::load();
        let llm = LlmSettings::load();
        let onboarding = OnboardingState::for_first_launch(prefs.onboarding_done, &llm);
        Self {
            brand: BrandAssets::load(ctx),
            provider_icons: providers::ProviderIcons::load(ctx),
            project: None,
            selection: NavSelection::Scene(String::new()),
            editor: EditorState::default(),
            sidebar: SidebarState::default(),
            api_key_draft: String::new(),
            api_key_editing: false,
            settings_model: llm.active().map(|p| p.model.clone()).unwrap_or_default(),
            settings_base: llm.active().map(|p| p.base_url.clone()).unwrap_or_default(),
            llm,
            brushes: builtin_brushes(),
            toast: String::new(),
            toast_until: 0.0,
            bootstrap_open: false,
            bootstrap_idea: String::new(),
            blueprint_preview: None,
            blueprint_apply_existing: false,
            onboarding,
            settings_open: false,
            settings_tab: 0,
            styles_open: false,
            publish_open: false,
            prefs,
            rename: None,
            create: None,
            pending_delete: None,
            md_cache: CommonMarkCache::default(),
            open_diagnostics: Vec::new(),
            versions_open: false,
            trash_open: false,
            external_conflict: None,
            llm_worker: LlmWorker::spawn(),
            text_version: 0,
            pending_clipboard: None,
            search_open: false,
            search_query: String::new(),
            search_hits: Vec::new(),
            import_open: false,
            import_preview: None,
            read_through_open: false,
            read_through_text: String::new(),
            pending_sidebar_job: None,
            synopsis_job_id: None,
            title_pending: std::collections::HashSet::new(),
            blueprint_job_id: None,
            close_block: None,
            close_error_open: false,
            pending_replies: HashMap::new(),
            export_options: ExportOptions::default(),
            conflict_modal_open: false,
            move_target: None,
            last_version_snapshot: None,
            review_scene_loaded: None,
            review_debounce_version: 0,
            nav_resizing: false,
            sidebar_resizing: false,
        }
    }

    pub fn for_tests(project_root: &Path) -> Self {
        let home = project_root.join(".authorkit-test-home");
        let _ = std::fs::create_dir_all(&home);
        // SAFETY: test-only entry point; set before prefs/secrets load.
        unsafe { std::env::set_var("AUTHORKIT_HOME", &home) };
        let ctx = egui::Context::default();
        let mut app = Self::new(&ctx);
        app.brand = BrandAssets::load(&ctx);
        if let Ok(opened) = Project::open_with_diagnostics(project_root) {
            let p = opened.project;
            if let Some(scene) = p.manuscript.ordered_scenes().first() {
                app.selection = NavSelection::Scene(scene.meta.id.clone());
            }
            app.project = Some(p);
            app.bind_chat_to_selection();
        }
        app
    }

    fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = msg.into();
        self.toast_until = -1.0; // resolved on next toast_ui frame
    }

    fn try_close_viewport(&mut self, ctx: &egui::Context) {
        if self.project.is_none() {
            return;
        }
        let mut errors = Vec::new();
        if let Err(e) = self.flush_editor() {
            errors.push(e);
        }
        if let Err(e) = self.persist_chat() {
            errors.push(e);
        }
        if errors.is_empty() {
            return;
        }
        let msg = errors.join(" · ");
        self.close_block = Some(msg);
        self.close_error_open = true;
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
    }

    fn close_error_modal(&mut self, ctx: &egui::Context) {
        if !self.close_error_open {
            return;
        }
        let mut open = true;
        let msg = self
            .close_block
            .clone()
            .unwrap_or_else(|| "Erreur d’enregistrement.".into());
        let dark = Theme::current(ctx).dark;
        let mut retry = false;
        let mut force = false;
        let mut cancel = false;
        widgets::FadeDialog::new("ak_close_err", "Enregistrement")
            .description(&msg)
            .width(440.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary(ui, "Réessayer").clicked() {
                        retry = true;
                    }
                    if widgets::ghost(ui, "Fermer sans enregistrer").clicked() {
                        force = true;
                    }
                    if widgets::ghost(ui, "Annuler").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel {
            open = false;
        }
        if retry {
            let ok = self.flush_editor().is_ok() && self.persist_chat().is_ok();
            if ok {
                self.close_error_open = false;
                self.close_block = None;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        } else if force {
            self.close_error_open = false;
            self.close_block = None;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else if !open {
            self.close_error_open = false;
        }
    }

    fn external_conflict_modal(&mut self, ctx: &egui::Context) {
        if self.external_conflict.is_none() {
            self.conflict_modal_open = false;
            return;
        }
        self.conflict_modal_open = true;
        let mut open = true;
        let dark = Theme::current(ctx).dark;
        let mut reload = false;
        let mut overwrite = false;
        let mut copy = false;
        widgets::FadeDialog::new("ak_ext_conflict", "Conflit externe")
            .description("Ce document a été modifié sur le disque. Que souhaitez-vous faire ?")
            .width(480.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary(ui, "Recharger").clicked() {
                        reload = true;
                    }
                    if widgets::ghost(ui, "Comparer").clicked() {
                        self.set_toast("Comparaison — ouvrez l’historique des versions.");
                    }
                    if widgets::ghost(ui, "Écraser").clicked() {
                        overwrite = true;
                    }
                    if widgets::ghost(ui, "Enregistrer une copie").clicked() {
                        copy = true;
                    }
                });
            });
        if reload {
            self.reload_editor_from_disk();
            self.external_conflict = None;
            open = false;
        } else if overwrite {
            self.editor.disk_mtime_ms = None;
            self.editor.disk_hash = None;
            self.external_conflict = None;
            let _ = self.flush_editor();
            open = false;
        } else if copy {
            self.save_conflict_copy();
            open = false;
        }
        if !open {
            self.conflict_modal_open = false;
        }
    }

    fn reload_editor_from_disk(&mut self) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        match self.editor.kind {
            EditorKind::Scene => {
                if let Some(scene) = project.manuscript.get_scene(&self.editor.id) {
                    self.editor.body = scene.body.clone();
                    self.editor.title = scene.meta.title.clone();
                    self.editor.synopsis = scene.meta.synopsis.clone();
                    self.editor.scene_status = scene.meta.status;
                    self.editor.disk_mtime_ms = ak_core::fsutil::mtime_millis(&scene.path);
                    self.editor.disk_hash =
                        Some(ak_core::fsutil::content_hash(scene.body.as_bytes()));
                }
            }
            EditorKind::Note => {
                if let Some(note) = project.notes.get(&self.editor.id) {
                    self.editor.body = note.body.clone();
                    self.editor.title = note.meta.name.clone();
                    self.editor.disk_mtime_ms = ak_core::fsutil::mtime_millis(&note.path);
                    self.editor.disk_hash =
                        Some(ak_core::fsutil::content_hash(note.body.as_bytes()));
                }
            }
            _ => {}
        }
        self.editor.dirty = false;
        self.editor.save_status = SaveStatus::Clean;
    }

    fn save_conflict_copy(&mut self) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let suffix = chrono::Local::now().format("%Y%m%d-%H%M");
        match self.editor.kind {
            EditorKind::Scene => {
                let title = format!("{} (copie {suffix})", self.editor.title);
                let root = project.root.clone();
                if project.notes.create(&root, NoteKind::Lore, &title).is_ok() {
                    self.set_toast("Copie enregistrée dans Univers (fiche Lore).");
                }
            }
            _ => self.set_toast("Copie locale — exportez le texte si besoin."),
        }
        self.external_conflict = None;
    }

    fn move_target_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.move_target.as_ref() else {
            return;
        };
        let mut open = modal.open;
        let target = modal.data.clone();
        let dark = Theme::current(ctx).dark;
        let mut picked: Option<(String, String)> = None;
        widgets::FadeDialog::new("ak_move_target", "Déplacer vers…")
            .width(420.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let Some(project) = self.project.as_ref() else {
                    return;
                };
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for act in &project.manuscript.index.acts {
                            ui.label(RichText::new(&act.title).strong());
                            for ch in &act.chapters {
                                if ui.button(format!("  {}", ch.title)).clicked() {
                                    picked = Some((act.id.clone(), ch.id.clone()));
                                }
                            }
                        }
                    });
            });
        if let Some((to_act, to_ch)) = picked {
            let Some(project) = self.project.as_mut() else {
                return;
            };
            let root = project.root.clone();
            let err = match &target {
                MoveTarget::Scene { id } => project
                    .manuscript
                    .move_scene(id, &to_act, &to_ch, usize::MAX, &root)
                    .map_err(|e| e.to_string()),
                MoveTarget::Chapter { act_id, id } => project
                    .manuscript
                    .move_chapter(act_id, id, &to_act, usize::MAX, &root)
                    .map_err(|e| e.to_string()),
            };
            match err {
                Ok(()) => self.set_toast("Élément déplacé."),
                Err(e) => self.set_toast(e),
            }
            open = false;
        }
        if let Some(modal) = self.move_target.as_mut() {
            modal.open = open;
        }
        if !open {
            self.move_target = None;
        }
    }

    fn load_review_for_current_scene(&mut self) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        if self.editor.kind != EditorKind::Scene {
            self.sidebar.review = None;
            return;
        }
        let sid = self.editor.id.clone();
        let body = self.editor.body.clone();
        if self.review_scene_loaded.as_deref() == Some(sid.as_str()) {
            return;
        }
        self.sidebar.ideas.clear();
        self.sidebar.review = load_review_for_text(&project.root, &sid, &body)
            .ok()
            .flatten();
        self.review_scene_loaded = Some(sid);
    }

    fn tick_review_stale(&mut self, ctx: &egui::Context) {
        if self.editor.kind != EditorKind::Scene {
            return;
        }
        if self.review_debounce_version == self.editor.text_version {
            return;
        }
        self.review_debounce_version = self.editor.text_version;
        if let Some(r) = self.sidebar.review.as_mut() {
            if r.scene_id == self.editor.id && r.refresh_stale(&self.editor.body) {
                ctx.request_repaint();
            }
        }
    }

    fn refresh_context_panel(&mut self) {
        if self.sidebar.tab != sidebar::SidebarTab::Context {
            return;
        }
        let prepared = self.build_request_context();
        self.sidebar.context_pieces = prepared.assembled.pieces.clone();
        self.sidebar.context_omitted_history = prepared.history_omitted;
        self.sidebar.context_doc_chars = prepared.assembled.documentary_chars;
        self.sidebar.context_budget_chars = prepared.assembled.budget.max_chars;
    }

    fn build_request_context(&mut self) -> PreparedRequest {
        let _ = self.flush_editor();
        let budget = RequestBudget::default();
        let history_raw: Vec<ChatMessage> = self
            .sidebar
            .messages
            .iter()
            .filter(|m| {
                matches!(m.status, MsgStatus::Done | MsgStatus::Streaming) && !m.content.is_empty()
            })
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();
        let hist_cap = budget
            .total_chars
            .saturating_sub(budget.reserve_reply)
            .saturating_sub(budget.documentary.max_chars);
        let budgeted = budget_history(&history_raw, hist_cap);
        let mut ctx = if let (Some(project), NavSelection::Scene(id)) =
            (self.project.as_ref(), &self.selection)
        {
            if let Some(scene) = project.manuscript.get_scene(id) {
                let mut c = scene_context(project, scene);
                c.draft_body = self.editor.body.clone();
                c.selection = self.editor.selected_text().unwrap_or("").to_string();
                c
            } else {
                ak_llm::LlmContext {
                    styles: project.styles.clone(),
                    draft_body: self.editor.body.clone(),
                    ..Default::default()
                }
            }
        } else if let Some(project) = self.project.as_ref() {
            ak_llm::LlmContext {
                styles: project.styles.clone(),
                draft_body: self.editor.body.clone(),
                ..Default::default()
            }
        } else {
            ak_llm::LlmContext::default()
        };
        ctx.budget = Some(budget.documentary);
        let assembled = ctx.assemble();
        PreparedRequest {
            system: assembled.system_prompt(),
            history: budgeted.messages,
            assembled,
            history_omitted: budgeted.omitted_turns,
        }
    }

    fn snapshot_editor(&mut self, reason: &str) -> Result<(), String> {
        self.flush_editor()?;
        let Some(project) = self.project.as_ref() else {
            return Ok(());
        };
        let kind = match self.editor.kind {
            EditorKind::Scene => "scene",
            EditorKind::Note => "note",
            _ => return Ok(()),
        };
        project
            .snapshot_document(kind, &self.editor.id, reason)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn maybe_snapshot_after_flush(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let due = self
            .last_version_snapshot
            .map(|t| now - t > 600.0)
            .unwrap_or(true);
        if due && self.snapshot_editor("auto").is_ok() {
            self.last_version_snapshot = Some(now);
        }
    }
}

impl eframe::App for AuthorKitApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        theme::apply(&ctx, self.prefs.theme);
        let pal = Theme::current(&ctx).palette;

        match onboarding::show(
            &ctx,
            &mut self.onboarding,
            &mut self.llm,
            self.prefs.locale,
            &self.provider_icons,
        ) {
            OnboardingAction::Saved | OnboardingAction::Skipped => {
                self.prefs.onboarding_done = true;
                self.prefs.save();
            }
            OnboardingAction::None => {}
        }

        Panel::top("top")
            .exact_size(52.0)
            .frame(
                egui::Frame::new()
                    .fill(pal.background)
                    .inner_margin(egui::Margin::symmetric(14, 0)),
            )
            .show_inside(ui, |ui| {
                let h = ui.available_height();
                ui.horizontal(|ui| {
                    ui.set_height(h);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        // Same size as chrome; app name is the only heavy weight.
                        self.brand.show_wordmark(ui, 28.0, FONT_UI);
                        if let Some(p) = &self.project {
                            ui.add_space(4.0);
                            ui.label(RichText::new("·").size(FONT_UI).color(pal.muted_foreground));
                            ui.label(
                                RichText::new(&p.meta.title)
                                    .size(FONT_UI)
                                    .color(pal.muted_foreground),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let has_project = self.project.is_some();
                        let lang = self.prefs.locale;
                        // Home: theme + settings. Open project: full chrome.
                        if widgets::icon_btn(ui, &lucide::SETTINGS, lang.settings_llm_tip())
                            .clicked()
                        {
                            self.settings_open = true;
                            self.settings_tab = 0;
                            if let Some(p) = self.llm.active() {
                                self.settings_model = p.model.clone();
                                self.settings_base = p.base_url.clone();
                            }
                        }
                        if has_project {
                            if widgets::icon_btn(ui, &lucide::BOOK_X, "Fermer le projet").clicked()
                            {
                                self.close_project();
                            }
                            ui.add_space(4.0);
                            let sep_h = 18.0_f32;
                            let (sep, _) = ui
                                .allocate_exact_size(egui::vec2(1.0, sep_h), egui::Sense::hover());
                            ui.painter().vline(
                                sep.center().x,
                                sep.y_range(),
                                egui::Stroke::new(1.0_f32, pal.border),
                            );
                            ui.add_space(4.0);
                            if widgets::icon_btn(ui, &lucide::UPLOAD, lang.publish_tip()).clicked()
                            {
                                self.publish_open = true;
                            }
                            if widgets::icon_btn(ui, &lucide::HISTORY, "Historique des versions")
                                .clicked()
                            {
                                self.versions_open = true;
                            }
                            if widgets::icon_btn(ui, &lucide::TRASH_2, "Corbeille").clicked() {
                                self.trash_open = true;
                            }
                            if widgets::icon_btn(ui, &lucide::PEN_LINE, lang.novel_styles_tip())
                                .clicked()
                            {
                                self.styles_open = true;
                            }
                            if widgets::icon_btn(ui, &lucide::SEARCH, "Rechercher").clicked() {
                                self.search_open = true;
                            }
                            if widgets::icon_btn(ui, &lucide::FILE_PLUS, "Importer un texte")
                                .clicked()
                            {
                                self.open_import_dialog();
                            }
                        }
                        let theme_icon = match self.prefs.theme {
                            ThemeMode::Light => &lucide::MOON,
                            ThemeMode::Dark => &lucide::SUN,
                        };
                        if widgets::icon_btn(ui, theme_icon, lang.theme_tip()).clicked() {
                            self.prefs.theme = self.prefs.theme.toggle();
                            self.prefs.save();
                            theme::apply(&ctx, self.prefs.theme);
                        }
                        if has_project {
                            if widgets::icon_btn(
                                ui,
                                if self.prefs.show_sidebar {
                                    &lucide::PANEL_RIGHT_CLOSE
                                } else {
                                    &lucide::PANEL_RIGHT
                                },
                                lang.sidebar_tip(),
                            )
                            .clicked()
                            {
                                self.prefs.show_sidebar = !self.prefs.show_sidebar;
                                self.prefs.save();
                            }
                            if widgets::icon_btn(
                                ui,
                                if self.prefs.show_nav {
                                    &lucide::PANEL_LEFT_CLOSE
                                } else {
                                    &lucide::PANEL_LEFT
                                },
                                lang.nav_tip(),
                            )
                            .clicked()
                            {
                                self.prefs.show_nav = !self.prefs.show_nav;
                                self.prefs.save();
                            }
                        }
                    });
                });
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range(),
                    rect.bottom(),
                    egui::Stroke::new(1.0_f32, pal.border),
                );
            });

        if ctx.input(|i| i.viewport().close_requested()) {
            self.try_close_viewport(&ctx);
        }
        self.close_error_modal(&ctx);
        self.external_conflict_modal(&ctx);
        self.move_target_modal(&ctx);

        if self.project.is_none() {
            CentralPanel::default()
                .frame(egui::Frame::new().fill(pal.background))
                .show_inside(ui, |ui| {
                    let action = welcome::show(
                        ui,
                        &self.brand,
                        &self.prefs.recent_projects,
                        self.prefs.locale,
                    );
                    if action.new_project {
                        self.create_project_dialog();
                    }
                    if action.open_project {
                        self.open_project_dialog();
                    }
                    if let Some(path) = action.open_recent {
                        self.open_project_at(&path);
                    }
                    if action.bootstrap {
                        self.bootstrap_open = true;
                        if self.project.is_none() {
                            self.create_project_dialog();
                        }
                    }
                });
            self.bootstrap_modal(&ctx);
            self.settings_modal(&ctx);
            self.toast_ui(&ctx);
            return;
        }

        self.refresh_context_panel();
        self.tick_review_stale(&ctx);
        // Autosave debounce (~0.7s after last edit)
        self.tick_autosave(&ctx);

        // Bottom BEFORE side panels → full-width status bar under nav + editor + sidebar.
        Panel::bottom("status")
            .exact_size(32.0)
            .frame(
                egui::Frame::new()
                    .fill(pal.card)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show_inside(ui, |ui| {
                let top = ui.max_rect().top();
                ui.painter().hline(
                    ui.max_rect().x_range(),
                    top,
                    egui::Stroke::new(1.0_f32, pal.border),
                );
                self.status_bar(ui);
            });

        if self.prefs.show_nav {
            Panel::left("nav")
                .resizable(true)
                .default_size(self.prefs.nav_width.clamp(NAV_WIDTH_MIN, NAV_WIDTH_MAX))
                .size_range(NAV_WIDTH_MIN..=NAV_WIDTH_MAX)
                .frame(
                    egui::Frame::new()
                        .fill(pal.card)
                        .inner_margin(egui::Margin::symmetric(14, 12)),
                )
                .show_inside(ui, |outer| {
                    let mut child = Self::fixed_panel_child(outer);
                    let ui = &mut child;
                    let project = self.project.as_ref().unwrap();
                    let out = nav::show(
                        ui,
                        project,
                        &self.selection,
                        self.prefs.manuscript_height,
                        self.prefs.locale,
                    );
                    if Self::track_side_panel_resize(
                        ui,
                        false, // left panel → handle on the right edge
                        &mut self.nav_resizing,
                        &mut self.prefs.nav_width,
                        NAV_WIDTH_MIN,
                        NAV_WIDTH_MAX,
                    ) {
                        self.prefs.save();
                    }
                    for action in out.actions {
                        self.handle_nav(action);
                    }
                });
        }

        if self.prefs.show_sidebar {
            Panel::right("sidebar")
                .resizable(true)
                .default_size(self.prefs.sidebar_width.clamp(280.0, 520.0))
                .size_range(280.0..=520.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.card)
                        .inner_margin(egui::Margin::symmetric(14, 12)),
                )
                .show_inside(ui, |outer| {
                    let mut child = Self::fixed_panel_child(outer);
                    let ui = &mut child;
                    let scene = self.current_scene().cloned();
                    let actions = {
                        let project = self.project.as_ref().unwrap();
                        sidebar::show(
                            ui,
                            project,
                            scene.as_ref(),
                            &mut self.sidebar,
                            &self.brushes,
                        )
                    };
                    if Self::track_side_panel_resize(
                        ui,
                        true, // right panel → handle on the left edge
                        &mut self.sidebar_resizing,
                        &mut self.prefs.sidebar_width,
                        280.0,
                        520.0,
                    ) {
                        self.prefs.save();
                    }
                    for a in actions {
                        self.handle_sidebar(a);
                    }
                });
        }

        CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.background)
                    .inner_margin(egui::Margin::symmetric(24, 10)),
            )
            .show_inside(ui, |ui| {
                let actions = self.draw_center(ui);
                for a in actions {
                    self.handle_editor(a);
                }
            });

        if let Some(action) = editor::show_slash_palette(&ctx, &mut self.editor) {
            self.handle_editor(action);
        }
        if let Some(action) = editor::show_synopsis_modal(&ctx, &mut self.editor) {
            self.handle_editor(action);
        }
        self.bootstrap_modal(&ctx);
        self.settings_modal(&ctx);
        self.styles_modal(&ctx);
        self.publish_modal(&ctx);
        self.rename_modal(&ctx);
        self.create_modal(&ctx);
        self.confirm_delete_modal(&ctx);
        self.versions_modal(&ctx);
        self.trash_modal(&ctx);
        self.search_modal(&ctx);
        self.import_modal(&ctx);
        self.read_through_modal(&ctx);
        let undo_scope = ctx.memory(|m| {
            let fid = m.focused();
            fid.is_none() || self.editor.draft_focus_id.is_some_and(|id| fid == Some(id))
        });
        if ctx.input(|i| i.modifiers.command || i.modifiers.ctrl) {
            if undo_scope {
                if ctx.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.shift) {
                    let _ = self.editor.redo();
                } else if ctx.input(|i| i.key_pressed(egui::Key::Z)) {
                    let _ = self.editor.undo();
                } else if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
                    let _ = self.editor.redo();
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::S)) {
                match self.flush_editor() {
                    Ok(()) => self.set_toast("Enregistré."),
                    Err(e) => self.set_toast(format!("Échec d’enregistrement : {e}")),
                }
            } else if ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.shift) {
                self.search_open = true;
            }
        }
        self.poll_llm_events(&ctx);
        // Keep frames coming while tokens stream in.
        if self.llm_worker.is_busy() {
            ctx.request_repaint();
        }
        if let Some(text) = self.pending_clipboard.take() {
            ctx.copy_text(text);
        }
        self.toast_ui(&ctx);
    }

    fn on_exit(&mut self) {
        self.prefs.save();
    }
}

impl AuthorKitApp {
    /// egui stores a side panel's width from its content rect, so any overflow grows
    /// the panel on every repaint (hover). Lay content out in a non-allocating child
    /// and claim exactly the panel's inner rect.
    fn fixed_panel_child(outer: &mut egui::Ui) -> egui::Ui {
        let rect = outer.max_rect();
        outer.advance_cursor_after_rect(rect);
        outer.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        )
    }

    /// Persist panel width only while dragging the resize handle (edge), never on hover/click inside.
    /// Returns true when the drag ended and prefs should be saved.
    fn track_side_panel_resize(
        ui: &egui::Ui,
        handle_on_left: bool,
        resizing: &mut bool,
        width: &mut f32,
        min_w: f32,
        max_w: f32,
    ) -> bool {
        // `ui` is the panel's inner rect; the handle sits on the outer edge (frame margin 14).
        let rect = ui.max_rect().expand2(egui::vec2(14.0, 0.0));
        let handle_x = if handle_on_left {
            rect.left()
        } else {
            rect.right()
        };
        let near_handle = ui.input(|i| {
            i.pointer
                .interact_pos()
                .is_some_and(|p| (p.x - handle_x).abs() <= 6.0 && rect.y_range().contains(p.y))
        });
        if near_handle && ui.input(|i| i.pointer.primary_pressed()) {
            *resizing = true;
        }
        if *resizing && ui.input(|i| i.pointer.primary_down()) {
            let w = rect.width().clamp(min_w, max_w);
            if (w - *width).abs() > 0.5 {
                *width = w;
            }
        }
        if ui.input(|i| i.pointer.primary_released()) {
            if *resizing {
                *width = rect.width().clamp(min_w, max_w);
                *resizing = false;
                return true;
            }
            *resizing = false;
        }
        false
    }
}

impl AuthorKitApp {
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let pal = Theme::current(ui.ctx()).palette;
        let lang = self.prefs.locale;

        let acts = project.manuscript.index.acts.len();
        let chapters: usize = project
            .manuscript
            .index
            .acts
            .iter()
            .map(|a| a.chapters.len())
            .sum();
        let scenes = project.manuscript.ordered_scenes().len();
        let total_words = project.manuscript.total_words();

        let (context, local_words) = match &self.selection {
            NavSelection::Scene(id) => {
                let path = scene_breadcrumb(project, id);
                let w = project
                    .manuscript
                    .get_scene(id)
                    .map(|s| s.word_count())
                    .unwrap_or(0);
                (path, Some(w))
            }
            NavSelection::Note(id) => {
                if let Some(note) = project.notes.get(id) {
                    (
                        format!(
                            "{} · {} · {}",
                            lang.universe(),
                            lang.note_kind_singular(note.meta.kind),
                            note.meta.name
                        ),
                        Some(note.body.split_whitespace().count()),
                    )
                } else {
                    (lang.universe().into(), None)
                }
            }
            NavSelection::Styles | NavSelection::Publish => (String::new(), None),
        };

        let mut metrics = vec![lang.structure(acts, chapters, scenes)];
        if let Some(w) = local_words {
            metrics.push(lang.words(w));
        }
        metrics.push(lang.words_total(total_words));
        let metrics = metrics.join(" · ");

        let bar = ui.max_rect();
        ui.allocate_rect(bar, egui::Sense::hover());

        // Left — where you are.
        let left = egui::Rect::from_min_max(
            bar.left_top(),
            egui::pos2(bar.left() + bar.width() * 0.42, bar.bottom()),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(left)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_min_height(bar.height());
                if !context.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&context)
                                .size(FONT_UI_SM)
                                .color(pal.foreground),
                        )
                        .truncate(),
                    );
                }
            },
        );

        // Center — save status.
        let now = ui.input(|i| i.time);
        let save_label = self.save_status_label(now);
        if !save_label.is_empty() {
            // Centered on the bar (= window center) using last frame's content width;
            // a left-to-right row would otherwise start at the slot's left edge.
            let w_id = egui::Id::new("ak_status_save_w");
            let max_w = bar.width() * 0.24;
            let content_w: f32 = ui
                .ctx()
                .data(|d| d.get_temp(w_id))
                .unwrap_or(max_w)
                .min(max_w);
            let is_external = self.external_conflict.is_some();
            let color = match &self.editor.save_status {
                SaveStatus::Failed(_) if !is_external => egui::Color32::from_rgb(200, 80, 80),
                _ => pal.muted_foreground,
            };
            let row = ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_center_size(
                        bar.center(),
                        egui::vec2(max_w, bar.height()),
                    ))
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                |ui| {
                    ui.set_min_height(bar.height());
                    ui.add_space(((max_w - content_w) / 2.0).max(0.0));
                    if matches!(self.editor.save_status, SaveStatus::Saving) {
                        ui.add(egui::Spinner::new().size(FONT_UI_SM));
                        ui.add_space(4.0);
                    }
                    ui.add(
                        egui::Label::new(RichText::new(&save_label).size(FONT_UI_SM).color(color))
                            .truncate(),
                    );
                    if matches!(self.editor.save_status, SaveStatus::Failed(_))
                        && !is_external
                        && ui
                            .add(
                                Button::new("Réessayer")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm),
                            )
                            .clicked()
                    {
                        self.editor.dirty = true;
                        self.editor.dirty_at = Some(0.0);
                        let _ = self.flush_editor();
                    }
                },
            );
            let lead = ((max_w - content_w) / 2.0).max(0.0);
            let measured = row.response.rect.width() - lead;
            if (measured - content_w).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(w_id, measured));
                ui.ctx().request_repaint();
            }
        }

        // Right — acts · chapters · scenes · page words · total words.
        let right = egui::Rect::from_min_max(
            egui::pos2(bar.left() + bar.width() * 0.62, bar.top()),
            bar.right_bottom(),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(right)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
            |ui| {
                ui.set_min_height(bar.height());
                ui.add(
                    egui::Label::new(
                        RichText::new(&metrics)
                            .size(FONT_UI_SM)
                            .color(pal.muted_foreground),
                    )
                    .truncate(),
                );
            },
        );
    }

    /// Ephemeral banner — never a blocking modal.
    fn toast_ui(&mut self, ctx: &egui::Context) {
        if self.toast.is_empty() {
            return;
        }
        let now = ctx.input(|i| i.time);
        if self.toast_until < 0.0 {
            self.toast_until = now + 2.5;
        } else if now > self.toast_until {
            self.toast.clear();
            return;
        }
        let pal = Theme::current(ctx).palette;
        let msg = self.toast.clone();
        egui::Area::new(egui::Id::new("ak_flash"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 56.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(pal.card)
                    .stroke(egui::Stroke::new(1.0_f32, pal.border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .shadow(egui::Shadow {
                        offset: [0, 1],
                        blur: 6,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(16),
                    })
                    .show(ui, |ui| {
                        ui.label(RichText::new(msg).size(FONT_UI).color(pal.foreground));
                    });
            });
    }

    fn confirm_delete_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.pending_delete.as_mut() else {
            return;
        };
        let mut open = modal.open;
        let mut confirm = false;
        let mut cancel = false;
        let lang = self.prefs.locale;
        let pending = modal.data.clone();
        let kind = pending.kind_label(lang);
        let name = pending.title().to_string();
        let dark = Theme::current(ctx).dark;
        let desc = lang.delete_confirm(kind, &name);
        widgets::FadeDialog::new("ak_confirm_delete", lang.confirm_delete())
            .description(&desc)
            .width(420.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                ui.add_space(8.0);
                widgets::modal_footer(ui, |ui| {
                    if ui
                        .add(
                            shadcn_egui::components::button::Button::new(lang.delete())
                                .variant(
                                    shadcn_egui::components::button::ButtonVariant::Destructive,
                                )
                                .icon(&lucide::TRASH_2),
                        )
                        .clicked()
                    {
                        confirm = true;
                    }
                    if widgets::ghost(ui, lang.cancel()).clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            self.execute_delete(pending);
            open = false;
        } else if cancel {
            open = false;
        }
        if let Some(modal) = self.pending_delete.as_mut() {
            modal.open = open;
        }
        if widgets::dialog_settled(ctx, "ak_confirm_delete", open) {
            self.pending_delete = None;
        }
    }

    fn execute_delete(&mut self, pending: PendingDelete) {
        if let Err(e) = self.flush_editor() {
            self.set_toast(format!("Enregistrement requis avant suppression : {e}"));
            return;
        }
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let result = match &pending {
            PendingDelete::Act { id, .. } => project.manuscript.delete_act(id, &project.root),
            PendingDelete::Chapter { act_id, id, .. } => {
                project.manuscript.delete_chapter(act_id, id, &project.root)
            }
            PendingDelete::Scene { id, .. } => project.manuscript.delete_scene(id, &project.root),
            PendingDelete::Note { id, .. } => project.notes.delete(id, &project.root),
        };
        if let Err(e) = result {
            self.set_toast(format!("Suppression impossible : {e}"));
            return;
        }
        self.set_toast("Déplacé vers la corbeille.");
        match pending {
            PendingDelete::Scene { id, .. } => {
                if matches!(&self.selection, NavSelection::Scene(s) if s == &id) {
                    self.selection = self
                        .project
                        .as_ref()
                        .and_then(|p| {
                            p.manuscript
                                .ordered_scenes()
                                .first()
                                .map(|s| s.meta.id.clone())
                        })
                        .map(NavSelection::Scene)
                        .unwrap_or(NavSelection::Scene(String::new()));
                    self.editor = EditorState::default();
                }
            }
            PendingDelete::Note { id, .. } => {
                if matches!(&self.selection, NavSelection::Note(n) if n == &id) {
                    self.selection = self
                        .project
                        .as_ref()
                        .and_then(|p| {
                            p.manuscript
                                .ordered_scenes()
                                .first()
                                .map(|s| NavSelection::Scene(s.meta.id.clone()))
                        })
                        .unwrap_or(NavSelection::Scene(String::new()));
                    self.editor = EditorState::default();
                }
            }
            _ => {}
        }
    }

    fn versions_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.versions_open;
        let dark = Theme::current(ctx).dark;
        let mut restore_id: Option<String> = None;
        widgets::FadeDialog::new("ak_versions", "Historique des versions")
            .width(480.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let Some(project) = self.project.as_ref() else {
                    ui.label("Aucun projet ouvert.");
                    return;
                };
                let kind = match self.editor.kind {
                    EditorKind::Scene => "scene",
                    EditorKind::Note => "note",
                    _ => {
                        ui.label("Ouvrez une scène ou une fiche.");
                        return;
                    }
                };
                let versions = ak_core::list_versions(&project.root, kind, &self.editor.id)
                    .unwrap_or_default();
                if versions.is_empty() {
                    ui.label("Aucune version enregistrée pour ce document.");
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for v in &versions {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&v.created_at).size(FONT_UI_SM));
                                    ui.label(RichText::new(&v.reason).weak().size(FONT_UI_SM));
                                    let preview: String = v.body.chars().take(80).collect();
                                    ui.label(RichText::new(preview).size(FONT_UI_SM));
                                });
                                if ui.button("Restaurer").clicked() {
                                    restore_id = Some(v.id.clone());
                                }
                            });
                            ui.separator();
                        }
                    });
            });
        if let Some(vid) = restore_id {
            if let Some(project) = self.project.as_ref() {
                let kind = match self.editor.kind {
                    EditorKind::Scene => "scene",
                    EditorKind::Note => "note",
                    _ => "",
                };
                if !kind.is_empty() {
                    match ak_core::restore_version_preserving_current(
                        &project.root,
                        kind,
                        &self.editor.id,
                        &vid,
                        &self.editor.title,
                        &self.editor.body,
                        "{}",
                    ) {
                        Ok(snap) => {
                            self.editor.body = snap.body;
                            if !snap.title.is_empty() {
                                self.editor.title = snap.title;
                            }
                            if kind == "scene" {
                                if let Ok(meta) =
                                    serde_json::from_str::<serde_json::Value>(&snap.meta_json)
                                {
                                    if let Some(s) = meta.get("synopsis").and_then(|v| v.as_str()) {
                                        self.editor.synopsis = s.to_string();
                                    }
                                    if let Some(st) = meta.get("status") {
                                        if let Ok(status) =
                                            serde_json::from_value::<ak_core::SceneStatus>(
                                                st.clone(),
                                            )
                                        {
                                            self.editor.scene_status = status;
                                        }
                                    }
                                }
                            }
                            self.editor.mark_dirty(0.0);
                            self.set_toast("Version restaurée (copie de sécurité conservée).");
                            open = false;
                        }
                        Err(e) => self.set_toast(format!("Restauration impossible : {e}")),
                    }
                }
            }
        }
        self.versions_open = open;
    }

    fn search_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.search_open;
        if !open {
            return;
        }
        let dark = Theme::current(ctx).dark;
        let mut go: Option<(crate::ms_tools::SearchHitKind, String)> = None;
        widgets::FadeDialog::new("ak_search", "Recherche")
            .width(480.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("Manuscrit et fiches…"),
                );
                if resp.changed() {
                    if let Some(p) = self.project.as_ref() {
                        self.search_hits = crate::ms_tools::search_project(p, &self.search_query);
                    }
                }
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for hit in &self.search_hits {
                            if ui
                                .selectable_label(false, format!("{} — {}", hit.title, hit.snippet))
                                .clicked()
                            {
                                go = Some((hit.kind, hit.id.clone()));
                            }
                        }
                    });
            });
        if let Some((kind, id)) = go {
            match kind {
                crate::ms_tools::SearchHitKind::Scene => {
                    self.apply_selection(NavSelection::Scene(id));
                }
                crate::ms_tools::SearchHitKind::Note => {
                    self.apply_selection(NavSelection::Note(id));
                }
            }
            open = false;
        }
        self.search_open = open;
    }

    fn open_import_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Importer Markdown / texte")
            .add_filter("Texte", &["md", "txt", "markdown"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(raw) => {
                    self.import_preview = Some(crate::ms_tools::preview_import(&raw));
                    self.import_open = true;
                }
                Err(e) => self.set_toast(format!("Lecture impossible : {e}")),
            }
        }
    }

    fn import_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.import_open;
        if !open {
            return;
        }
        let dark = Theme::current(ctx).dark;
        let mut confirm = false;
        let mut cancel = false;
        let preview = self.import_preview.clone();
        widgets::FadeDialog::new("ak_import", "Aperçu de l’import")
            .width(520.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                if let Some(p) = &preview {
                    ui.label(format!("{} scènes détectées", p.scenes.len()));
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            for s in &p.scenes {
                                ui.label(RichText::new(&s.title).strong());
                                let snip: String = s.body.chars().take(100).collect();
                                ui.label(RichText::new(snip).size(FONT_UI_SM).weak());
                                ui.separator();
                            }
                        });
                }
                ui.add_space(12.0);
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary(ui, "Importer").clicked() {
                        confirm = true;
                    }
                    if widgets::ghost(ui, "Annuler").clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            if let (Some(preview), Some(project)) =
                (self.import_preview.take(), self.project.as_mut())
            {
                match crate::ms_tools::apply_import(project, &preview) {
                    Ok(n) => {
                        self.set_toast(format!("{n} scènes importées."));
                        open = false;
                    }
                    Err(e) => self.set_toast(e),
                }
            }
        } else if cancel {
            self.import_preview = None;
            open = false;
        }
        self.import_open = open;
    }

    fn read_through_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.read_through_open;
        if !open {
            return;
        }
        let dark = Theme::current(ctx).dark;
        widgets::FadeDialog::new("ak_read_through", "Lecture continue")
            .width(640.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(480.0)
                    .show(ui, |ui| {
                        ui.label(&self.read_through_text);
                    });
            });
        self.read_through_open = open;
    }

    fn trash_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.trash_open;
        let dark = Theme::current(ctx).dark;
        let mut restore_id: Option<String> = None;
        widgets::FadeDialog::new("ak_trash", "Corbeille")
            .width(480.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let Some(project) = self.project.as_ref() else {
                    ui.label("Aucun projet ouvert.");
                    return;
                };
                let entries = ak_core::list_trash(&project.root).unwrap_or_default();
                if entries.is_empty() {
                    ui.label("La corbeille est vide.");
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for e in &entries {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(&e.label).size(FONT_UI));
                                    ui.label(RichText::new(&e.created_at).weak().size(FONT_UI_SM));
                                });
                                if ui.button("Restaurer").clicked() {
                                    restore_id = Some(e.id.clone());
                                }
                            });
                            ui.separator();
                        }
                    });
            });
        if let Some(tid) = restore_id {
            if let Some(project) = self.project.as_mut() {
                match ak_core::restore_trash_entry(project, &tid) {
                    Ok(entry) => {
                        self.set_toast(format!("Restauré : {}", entry.label));
                        open = false;
                    }
                    Err(e) => self.set_toast(format!("Restauration impossible : {e}")),
                }
            }
        }
        self.trash_open = open;
    }

    fn settings_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        let dark = Theme::current(ctx).dark;
        let lang = self.prefs.locale;
        let title = lang.settings();
        let was_open = open;
        widgets::FadeDialog::new("ak_settings", title)
            .width(560.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let tabs = [lang.tab_llm(), lang.tab_interface()];
                let _ = shadcn_egui::components::tabs::tab_bar(ui, &mut self.settings_tab, &tabs);
                ui.add_space(12.0);
                // Body height = tallest tab seen so far, so switching tabs never resizes.
                let h_id = egui::Id::new("ak_settings_body_h");
                let body_h: f32 = ui.ctx().data(|d| d.get_temp(h_id)).unwrap_or(0.0);
                if body_h == 0.0 && self.settings_tab != 0 {
                    // Opened on a short tab: measure the AI tab once, off-screen.
                    let probe = ui.scope_builder(egui::UiBuilder::new().invisible(), |ui| {
                        ui.disable();
                        self.draw_settings_llm(ui);
                    });
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(h_id, probe.response.rect.height()));
                    ui.ctx().request_discard("settings body measured");
                    return;
                }
                let body = ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), body_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_height(body_h);
                        match self.settings_tab {
                            0 => self.draw_settings_llm(ui),
                            _ => self.draw_settings_interface(ui),
                        }
                    },
                );
                let measured = body.response.rect.height();
                if measured > body_h + 0.5 {
                    ui.ctx().data_mut(|d| d.insert_temp(h_id, measured));
                }
            });
        if was_open && !open {
            self.prefs.save();
            self.api_key_draft.clear();
            self.api_key_editing = false;
        }
        self.settings_open = open;
    }

    fn styles_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.styles_open;
        let dark = Theme::current(ctx).dark;
        let mut save = false;
        let mut close = false;
        let lang = self.prefs.locale;
        let styles_desc = lang.styles_desc();
        widgets::FadeDialog::new("ak_styles", lang.novel_styles())
            .description(styles_desc)
            .width(520.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                if let Some(project) = self.project.as_mut() {
                    ui.label(lang.rules());
                    ui.add(
                        egui::TextEdit::multiline(&mut project.styles.rules)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(lang.voice());
                    ui.add(
                        egui::TextEdit::multiline(&mut project.styles.voice_notes)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.label(lang.avoid());
                    ui.add(
                        egui::TextEdit::multiline(&mut project.styles.avoid)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(12.0);
                    widgets::modal_footer(ui, |ui| {
                        if widgets::primary(ui, lang.save()).clicked() {
                            save = true;
                            close = true;
                        }
                        if widgets::ghost(ui, lang.close()).clicked() {
                            close = true;
                        }
                    });
                }
            });
        if close {
            open = false;
        }
        if save {
            if let Some(project) = self.project.as_ref() {
                match project.save_meta() {
                    Ok(()) => self.set_toast(lang.styles_saved()),
                    Err(e) => self.set_toast(e.to_string()),
                }
            }
        }
        self.styles_open = open;
    }

    fn publish_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.publish_open;
        let dark = Theme::current(ctx).dark;
        let mut export: Option<ExportKind> = None;
        let lang = self.prefs.locale;
        let (title, words) = self
            .project
            .as_ref()
            .map(|p| (p.meta.title.clone(), p.manuscript.total_words()))
            .unwrap_or_default();
        let desc = lang.publish_desc(&title, words);
        widgets::FadeDialog::new("ak_publish", lang.publish())
            .description(&desc)
            .width(440.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                ui.checkbox(
                    &mut self.export_options.include_chapter_titles,
                    "Titres de chapitres",
                );
                ui.checkbox(
                    &mut self.export_options.include_scene_titles,
                    "Titres de scènes",
                );
                ui.checkbox(
                    &mut self.export_options.strip_annotations,
                    "Retirer les annotations",
                );
                ui.checkbox(&mut self.export_options.title_page, "Page de titre");
                ui.add_space(8.0);
                if ui
                    .add(
                        shadcn_egui::components::button::Button::new(lang.export_md())
                            .variant(shadcn_egui::components::button::ButtonVariant::Outline)
                            .icon(&lucide::FILE_TEXT),
                    )
                    .clicked()
                {
                    export = Some(ExportKind::Markdown);
                }
                ui.add_space(6.0);
                if ui
                    .add(
                        shadcn_egui::components::button::Button::new(lang.export_epub())
                            .variant(shadcn_egui::components::button::ButtonVariant::Outline)
                            .icon(&lucide::BOOK),
                    )
                    .clicked()
                {
                    export = Some(ExportKind::Epub);
                }
                ui.add_space(6.0);
                if ui
                    .add(
                        shadcn_egui::components::button::Button::new(lang.export_docx())
                            .variant(shadcn_egui::components::button::ButtonVariant::Outline)
                            .icon(&lucide::DOWNLOAD),
                    )
                    .clicked()
                {
                    export = Some(ExportKind::Docx);
                }
            });
        if export.is_some() {
            open = false;
        }
        self.publish_open = open;
        if let Some(kind) = export {
            self.do_export(kind);
        }
    }

    fn tick_autosave(&mut self, ctx: &egui::Context) {
        if !self.editor.dirty {
            return;
        }
        let Some(at) = self.editor.dirty_at else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if now - at < 0.7 {
            return;
        }
        self.editor.saving_since = Some(now);
        match self.flush_editor() {
            Ok(()) => {
                self.editor.dirty_at = None;
                self.editor.saving_since = Some(now);
                ctx.request_repaint_after(std::time::Duration::from_millis(320));
                self.prefs.save();
            }
            Err(_) => {
                // Keep dirty; status already Failed. Retry after another debounce.
                self.editor.dirty_at = Some(now);
            }
        }
    }

    fn draw_center(&mut self, ui: &mut egui::Ui) -> Vec<EditorAction> {
        match &self.selection {
            NavSelection::Scene(id) => {
                let id = id.clone();
                if let Some(project) = self.project.as_ref() {
                    if let Some(scene) = project.manuscript.get_scene(&id) {
                        let scene = scene.clone();
                        let out = editor::show_scene(
                            ui,
                            project,
                            &scene,
                            &mut self.editor,
                            &self.brushes,
                            &mut self.prefs.code_mode,
                            &mut self.md_cache,
                            self.prefs.locale,
                        );
                        if self.review_scene_loaded.as_deref() != Some(id.as_str()) {
                            self.load_review_for_current_scene();
                        }
                        return out.actions;
                    }
                }
                ui.label(self.prefs.locale.scene_missing());
                Vec::new()
            }
            NavSelection::Note(id) => {
                let id = id.clone();
                if let Some(project) = self.project.as_ref() {
                    if let Some(note) = project.notes.get(&id) {
                        let note = note.clone();
                        return editor::show_note(
                            ui,
                            &note,
                            &mut self.editor,
                            &self.brushes,
                            &mut self.prefs.code_mode,
                            &mut self.md_cache,
                            self.prefs.locale,
                        )
                        .actions;
                    }
                }
                ui.label(self.prefs.locale.note_missing());
                Vec::new()
            }
            NavSelection::Styles | NavSelection::Publish => {
                // Legacy nav targets — open modals instead of full-page views.
                Vec::new()
            }
        }
    }

    fn draw_settings_llm(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.locale;
        let pal = Theme::current(ui.ctx()).palette;

        ui.label(
            RichText::new(lang.provider())
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
        );
        ui.add_space(4.0);
        let llm = &self.llm;
        let picked = providers::show_grid(ui, &self.provider_icons, |preset| {
            llm.active()
                .is_some_and(|p| p.provider == preset.provider && p.base_url == preset.base_url)
        });
        if let Some(preset) = picked {
            providers::apply_preset(&mut self.llm, preset);
            self.settings_model = preset.default_model.into();
            self.settings_base = preset.base_url.into();
        }

        ui.add_space(12.0);
        ui.label(
            RichText::new(lang.model())
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
        );
        ui.add_space(4.0);
        let models = providers::models_for(&self.llm);
        providers::model_select(
            ui,
            "ak_settings_model",
            &mut self.settings_model,
            &models,
            lang.choose_model(),
        );

        ui.add_space(8.0);
        let _ = ui.add(Label::new(lang.base_url()));
        let _ = ui.add(Input::new(&mut self.settings_base).max_width(f32::INFINITY));

        if self
            .llm
            .active()
            .is_some_and(|p| p.provider != ProviderKind::Ollama)
        {
            ui.add_space(8.0);
            let has_key = self.llm.active().is_some_and(|p| self.llm.has_api_key(p));
            let masked = self
                .llm
                .active()
                .and_then(|p| self.llm.get_api_key(p).ok().flatten())
                .map(|k| mask_api_key(&k));
            let _ = ui.add(Label::new(lang.api_key()));
            ui.add_space(4.0);

            let show_editor = self.api_key_editing || !has_key;
            if !show_editor {
                if let Some(masked) = masked.as_deref() {
                    // Vega `Input` geometry: 36px tall, 10px side padding, input border.
                    const INPUT_H: f32 = 36.0;
                    let dark = Theme::current(ui.ctx()).dark;
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), INPUT_H),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect(
                        rect,
                        8.0,
                        if dark {
                            pal.input.gamma_multiply(0.3)
                        } else {
                            egui::Color32::TRANSPARENT
                        },
                        egui::Stroke::new(1.0_f32, pal.input),
                        egui::StrokeKind::Inside,
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(rect.shrink2(egui::vec2(10.0, 0.0)))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            widgets::icon_colored(
                                ui,
                                &lucide::KEY_ROUND,
                                14.0,
                                pal.muted_foreground,
                            );
                            ui.label(
                                RichText::new(masked)
                                    .size(FONT_UI)
                                    .monospace()
                                    .color(pal.foreground),
                            );
                            ui.label(
                                RichText::new(lang.api_key_saved())
                                    .size(FONT_UI_SM)
                                    .color(pal.muted_foreground),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if widgets::icon_ghost_xs(
                                        ui,
                                        &lucide::PENCIL,
                                        "Modifier la clé",
                                    )
                                    .clicked()
                                    {
                                        self.api_key_editing = true;
                                        self.api_key_draft.clear();
                                    }
                                },
                            );
                        },
                    );
                }
            } else {
                let _ = ui.add(
                    Input::new(&mut self.api_key_draft)
                        .id_salt("ak_settings_api_key")
                        .password(true)
                        .hint(if has_key {
                            lang.api_key_replace_hint()
                        } else {
                            lang.api_key_hint()
                        })
                        .max_width(f32::INFINITY),
                );
                if has_key {
                    ui.horizontal(|ui| {
                        if widgets::ghost(ui, "Annuler").clicked() {
                            self.api_key_editing = false;
                            self.api_key_draft.clear();
                        }
                        ui.label(
                            RichText::new("Collez une nouvelle clé pour remplacer l’actuelle.")
                                .size(FONT_UI_SM)
                                .color(pal.muted_foreground),
                        );
                    });
                }
            }
        }

        ui.add_space(12.0);
        let mut save_clicked = false;
        ui.horizontal(|ui| {
            if widgets::outline(ui, "Tester la connexion").clicked() {
                match self.client() {
                    Ok(client) => {
                        let result = client.complete(&ak_llm::GenerationRequest {
                            system: "Tu es un test de connexion. Réponds uniquement: OK".into(),
                            messages: vec![ak_llm::ChatMessage {
                                role: "user".into(),
                                content: "ping".into(),
                            }],
                        });
                        match result {
                            Ok(reply) => {
                                let snippet: String = reply.chars().take(60).collect();
                                self.set_toast(format!("Connexion OK — {snippet}"));
                            }
                            Err(e) => self.set_toast(format!(
                                "Échec connexion : {}",
                                ak_llm::redact_secrets(&e.to_string(), None)
                            )),
                        }
                    }
                    Err(e) => self.set_toast(e),
                }
            }
            if let Some(p) = self.llm.active() {
                ui.label(
                    RichText::new(format!("Actif : {} / {}", p.name, p.model))
                        .size(FONT_UI_SM)
                        .weak(),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                save_clicked = widgets::primary(ui, lang.save()).clicked();
            });
        });
        if save_clicked {
            self.llm.normalize_keyring_accounts();
            if let Some(p) = self.llm.active_mut() {
                p.model = self.settings_model.clone();
                p.base_url = self.settings_base.clone();
            }
            // Re-normalize after base_url change (credential account follows endpoint).
            self.llm.normalize_keyring_accounts();

            let mut ok_msg = lang.llm_saved().to_string();
            if let Some(p) = self.llm.active().cloned() {
                if p.provider != ProviderKind::Ollama {
                    if !self.api_key_draft.trim().is_empty() {
                        match self.llm.set_api_key(&p, &self.api_key_draft) {
                            Ok(()) => {
                                self.api_key_draft.clear();
                                self.api_key_editing = false;
                                ok_msg = lang.llm_key_saved().into();
                            }
                            Err(e) => {
                                self.set_toast(format!("Clé non enregistrée: {e}"));
                                let _ = self.llm.save();
                                return;
                            }
                        }
                    } else if !self.llm.has_api_key(&p) {
                        self.set_toast(lang.llm_key_missing());
                        let _ = self.llm.save();
                        return;
                    }
                }
            }
            self.set_toast(match self.llm.save() {
                Ok(()) => ok_msg,
                Err(e) => format!("Erreur: {e}"),
            });
        }
    }

    fn draw_settings_interface(&mut self, ui: &mut egui::Ui) {
        let lang = self.prefs.locale;
        let pal = Theme::current(ui.ctx()).palette;
        let ctx = ui.ctx().clone();

        ui.label(
            RichText::new(lang.language())
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for l in Lang::all() {
                let selected = self.prefs.locale == l;
                if ui
                    .add(
                        Button::new(l.label())
                            .variant(if selected {
                                ButtonVariant::Default
                            } else {
                                ButtonVariant::Outline
                            })
                            .size(ButtonSize::Sm),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.prefs.locale = l;
                    self.prefs.save();
                }
            }
        });

        ui.add_space(16.0);
        ui.label(
            RichText::new(lang.appearance())
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let light = self.prefs.theme == ThemeMode::Light;
            if ui
                .add(
                    Button::new(lang.theme_light())
                        .variant(if light {
                            ButtonVariant::Default
                        } else {
                            ButtonVariant::Outline
                        })
                        .size(ButtonSize::Sm)
                        .icon(&lucide::SUN),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.prefs.theme = ThemeMode::Light;
                self.prefs.save();
                theme::apply(&ctx, self.prefs.theme);
            }
            if ui
                .add(
                    Button::new(lang.theme_dark())
                        .variant(if !light {
                            ButtonVariant::Default
                        } else {
                            ButtonVariant::Outline
                        })
                        .size(ButtonSize::Sm)
                        .icon(&lucide::MOON),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.prefs.theme = ThemeMode::Dark;
                self.prefs.save();
                theme::apply(&ctx, self.prefs.theme);
            }
        });
    }

    pub fn apply_selection(&mut self, sel: NavSelection) {
        if let Err(e) = self.flush_editor() {
            self.set_toast(format!("Enregistrement échoué — document conservé : {e}"));
            return;
        }
        if let Err(e) = self.persist_chat() {
            self.set_toast(format!("Conversation non enregistrée : {e}"));
        }
        self.selection = sel;
        self.editor = EditorState::default();
        self.review_scene_loaded = None;
        if matches!(self.selection, NavSelection::Scene(_)) {
            self.load_review_for_current_scene();
        } else {
            self.sidebar.review = None;
        }
        self.bind_chat_to_selection();
    }

    /// Persist the open editor document. Never clears dirty on failure.
    fn save_status_label(&self, now: f64) -> String {
        if matches!(self.editor.save_status, SaveStatus::Saving) {
            return "Sauvegarde en cours".into();
        }
        if matches!(self.editor.save_status, SaveStatus::Unsaved) {
            return "Modifications non enregistrées".into();
        }
        if let SaveStatus::Failed(msg) = &self.editor.save_status {
            if self.external_conflict.is_some() {
                return msg.clone();
            }
            return format!("Échec : {msg}");
        }
        if matches!(
            self.editor.save_status,
            SaveStatus::Saved | SaveStatus::Clean
        ) {
            if let Some(t) = self.editor.last_saved_at {
                return format!("Dernière sauvegarde à {}", t.format("%H:%M"));
            }
        }
        if self.editor.saving_since.is_some_and(|s| now - s < 0.3) {
            return "Sauvegarde en cours".into();
        }
        String::new()
    }

    pub fn flush_editor(&mut self) -> Result<(), String> {
        if !self.editor.dirty {
            return Ok(());
        }
        self.editor.save_status = SaveStatus::Saving;
        let Some(project) = self.project.as_mut() else {
            self.editor.save_status = SaveStatus::Clean;
            self.editor.dirty = false;
            return Ok(());
        };

        // External conflict detection
        let path_opt = match self.editor.kind {
            EditorKind::Scene => project
                .manuscript
                .get_scene(&self.editor.id)
                .map(|s| s.path.clone()),
            EditorKind::Note => project.notes.get(&self.editor.id).map(|n| n.path.clone()),
            _ => None,
        };
        if let Some(path) = path_opt.as_ref() {
            if let (Some(known), Some(now_m)) = (
                self.editor.disk_mtime_ms,
                ak_core::fsutil::mtime_millis(path),
            ) {
                if now_m > known {
                    if let Ok(disk) = std::fs::read_to_string(path) {
                        let hash = ak_core::fsutil::content_hash(disk.as_bytes());
                        if self.editor.disk_hash.is_some_and(|h| h != hash) {
                            let msg = "Fichier modifié à l’extérieur — enregistrement bloqué."
                                .to_string();
                            self.editor.save_status = SaveStatus::Failed(msg.clone());
                            self.external_conflict = Some(msg.clone());
                            return Err(msg);
                        }
                    }
                }
            }
        }

        let result = match self.editor.kind {
            EditorKind::Scene => {
                if let Some(scene) = project.manuscript.get_scene_mut(&self.editor.id) {
                    scene.body = self.editor.body.clone();
                    scene.meta.title = self.editor.title.clone();
                    scene.meta.synopsis = self.editor.synopsis.clone();
                    scene.meta.status = self.editor.scene_status;
                    let scene = scene.clone();
                    project.manuscript.save_scene(&scene).map(|_| scene.path)
                } else {
                    Err(ak_core::Error::SceneNotFound(self.editor.id.clone()))
                }
            }
            EditorKind::Note => {
                if let Some(note) = project.notes.get_mut(&self.editor.id) {
                    note.body = self.editor.body.clone();
                    note.meta.name = self.editor.title.clone();
                    let note = note.clone();
                    project.notes.save_note(&note).map(|_| note.path)
                } else {
                    Err(ak_core::Error::NoteNotFound(self.editor.id.clone()))
                }
            }
            _ => {
                self.editor.dirty = false;
                self.editor.save_status = SaveStatus::Clean;
                return Ok(());
            }
        };

        match result {
            Ok(path) => {
                self.editor.dirty = false;
                self.editor.save_status = SaveStatus::Saved;
                self.editor.last_saved_at = Some(chrono::Local::now());
                self.editor.disk_mtime_ms = ak_core::fsutil::mtime_millis(&path);
                self.editor.disk_hash =
                    Some(ak_core::fsutil::content_hash(self.editor.body.as_bytes()));
                self.external_conflict = None;
                self.maybe_snapshot_after_flush();
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                self.editor.save_status = SaveStatus::Failed(msg.clone());
                Err(msg)
            }
        }
    }

    fn handle_editor(&mut self, action: EditorAction) {
        match action {
            EditorAction::Save => {
                self.editor.dirty = true;
                match self.flush_editor() {
                    Ok(()) => self.set_toast("Enregistré."),
                    Err(e) => self.set_toast(format!("Échec d’enregistrement : {e}")),
                }
            }
            EditorAction::LinkNote(nid) => {
                let _ = self.flush_editor();
                if let (Some(project), NavSelection::Scene(sid)) =
                    (self.project.as_mut(), self.selection.clone())
                {
                    let _ = project.manuscript.link_note(&sid, &nid, &project.root);
                }
            }
            EditorAction::UnlinkNote(nid) => {
                let _ = self.flush_editor();
                if let (Some(project), NavSelection::Scene(sid)) =
                    (self.project.as_mut(), self.selection.clone())
                {
                    let _ = project.manuscript.unlink_note(&sid, &nid, &project.root);
                }
            }
            EditorAction::OpenNote(id) => {
                self.apply_selection(NavSelection::Note(id));
            }
            EditorAction::SaveStyles => {
                if let Some(p) = &self.project {
                    let _ = p.save_meta();
                    self.set_toast("Styles enregistrés.");
                }
            }
            EditorAction::ExportMarkdown => self.do_export(ExportKind::Markdown),
            EditorAction::ExportEpub => self.do_export(ExportKind::Epub),
            EditorAction::ExportDocx => self.do_export(ExportKind::Docx),
            EditorAction::Bootstrap => {
                self.bootstrap_open = true;
            }
            EditorAction::SlashContinue => self.stage_continue(ContinueMode::Continue, None),
            EditorAction::SlashSentence => self.stage_continue(ContinueMode::Sentence, None),
            EditorAction::SlashParagraph => self.stage_continue(ContinueMode::Paragraph, None),
            EditorAction::SlashGenerate { prompt } => {
                self.stage_continue(ContinueMode::Generate, Some(prompt))
            }
            EditorAction::Brush(id) => self.stage_brush(&id),
            EditorAction::AddToChat => self.stage_selection_to_chat(),
            EditorAction::GenerateSynopsis => self.run_generate_synopsis(),
        }
    }

    pub fn handle_sidebar(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::SendChat => self.send_composer(),
            SidebarAction::CancelChat => {
                self.llm_worker.cancel();
                self.sidebar.status = "Annulation…".into();
            }
            SidebarAction::SetComposerMode(mode) => {
                // Brushes require a linked selection.
                if matches!(mode, crate::chat_store::ComposerMode::Brush { .. })
                    && self.sidebar.attachment.is_none()
                {
                    self.set_toast("Ajoutez une sélection au chat pour utiliser un outil.");
                    self.sidebar.mode_menu_open = false;
                    return;
                }
                self.sidebar.composer_mode = mode;
                self.sidebar.mode_menu_open = false;
                let _ = self.persist_chat();
            }
            SidebarAction::ClearAttachment => {
                self.sidebar.attachment = None;
                if matches!(
                    self.sidebar.composer_mode,
                    crate::chat_store::ComposerMode::Brush { .. }
                ) {
                    self.sidebar.composer_mode = crate::chat_store::ComposerMode::Chat;
                }
                let _ = self.persist_chat();
            }
            SidebarAction::ClearComposer => {
                self.sidebar.clear_composer();
                let _ = self.persist_chat();
            }
            SidebarAction::NewConversation => self.new_chat_conversation(),
            SidebarAction::LoadConversation(id) => {
                if let Err(e) = self.persist_chat() {
                    self.set_toast(format!("Conversation non enregistrée : {e}"));
                }
                self.load_chat_conversation(&id);
            }
            SidebarAction::SetReviewStatus { item_id, status } => {
                if let Some(r) = self.sidebar.review.as_mut() {
                    if let Some(item) = r.items.iter_mut().find(|i| i.id == item_id) {
                        item.status = status;
                    }
                    if let Some(project) = self.project.as_ref() {
                        let copy = r.clone();
                        if let Err(e) = persist_review(&project.root, &copy) {
                            self.set_toast(format!("Revue non enregistrée : {e}"));
                        } else {
                            *r = copy;
                        }
                    }
                }
            }
            SidebarAction::CopyMessage(id) => {
                if let Some(content) = self
                    .sidebar
                    .messages
                    .iter()
                    .find(|m| m.id == id)
                    .map(|m| m.content.clone())
                {
                    self.clipboard_set(&content);
                    self.set_toast("Copié.");
                }
            }
            SidebarAction::InsertMessage(text) => {
                if !self.editor.body.is_empty() && !self.editor.body.ends_with('\n') {
                    self.editor.body.push('\n');
                }
                self.editor.body.push_str(&text);
                self.text_version += 1;
                self.editor.mark_dirty(0.0);
            }
            SidebarAction::ReplaceWithMessage(text) => {
                if self.editor.has_selection {
                    self.editor.replace_selection(&text);
                    self.text_version += 1;
                    self.editor.mark_dirty(0.0);
                } else {
                    self.set_toast(
                        "Sélectionnez un passage pour le remplacer (pas le document entier).",
                    );
                }
            }
            SidebarAction::RetryMessage(id) | SidebarAction::RegenerateMessage(id) => {
                self.run_chat_regenerate(&id);
            }
            SidebarAction::AcceptEditProposal(id) => self.accept_edit_proposal(&id, 0.0),
            SidebarAction::RejectEditProposal(id) => self.reject_edit_proposal(&id),
            SidebarAction::RegenerateEditProposal(id) => self.regenerate_edit_proposal(&id),
            SidebarAction::StopEditProposal(id) => {
                if let Some(job) = self.edit_proposal_job_id(&id) {
                    self.llm_worker.cancel_job(&job);
                }
            }
            SidebarAction::JumpToBottom => {
                self.sidebar.stick_bottom = true;
            }
            SidebarAction::RefreshIdeas => self.run_ideas(),
            SidebarAction::WriteIdea(i) => {
                if let Some(idea) = self.sidebar.ideas.get(i).cloned() {
                    self.sidebar.input =
                        format!("Écris un passage pour: {}\n\n{}", idea.title, idea.pitch);
                    self.run_chat();
                }
            }
            SidebarAction::RunReview => self.run_review_action(),
        }
    }

    fn clipboard_set(&mut self, text: &str) {
        self.pending_clipboard = Some(text.to_string());
    }

    fn current_context_id(&self) -> String {
        crate::chat_store::context_id_for(&self.selection)
    }

    fn bind_chat_to_selection(&mut self) {
        let context_id = self.current_context_id();
        let Some(project) = self.project.as_ref() else {
            self.sidebar = SidebarState::default();
            self.sidebar.context_id = context_id;
            self.refresh_chat_hints();
            return;
        };
        let conv = crate::chat_store::last_or_empty(&project.root, &context_id);
        self.sidebar = SidebarState::from_stored(conv);
        self.sidebar.tab = sidebar::SidebarTab::Chat;
        self.refresh_chat_hints();
    }

    fn ensure_chat_conversation(&mut self) {
        if !self.sidebar.conversation_id.is_empty() {
            if self.sidebar.context_id.is_empty() {
                self.sidebar.context_id = self.current_context_id();
            }
            return;
        }
        let context_id = self.current_context_id();
        let conv = crate::chat_store::new_conversation(context_id);
        self.sidebar.conversation_id = conv.id;
        self.sidebar.conversation_title = conv.title;
        self.sidebar.context_id = conv.context_id;
    }

    fn new_chat_conversation(&mut self) {
        let _ = self.persist_chat();
        let context_id = self.current_context_id();
        let conv = crate::chat_store::new_conversation(context_id);
        self.sidebar = SidebarState::from_stored(conv);
        self.sidebar.tab = sidebar::SidebarTab::Chat;
        self.refresh_chat_hints();
    }

    fn load_chat_conversation(&mut self, id: &str) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        if let Some(c) = crate::chat_store::load_conversation(&project.root, id) {
            self.sidebar = SidebarState::from_stored(c);
            self.sidebar.show_history = false;
            self.sidebar.tab = sidebar::SidebarTab::Chat;
            self.refresh_chat_hints();
        }
    }

    pub fn persist_chat(&mut self) -> Result<(), String> {
        let Some(project) = self.project.as_ref() else {
            return Ok(());
        };
        if self.sidebar.conversation_id.is_empty() {
            return Ok(());
        }
        // Skip persisting never-touched empty shells.
        if self.sidebar.messages.is_empty() && !self.sidebar.composer_dirty() {
            return Ok(());
        }
        let context_id = if self.sidebar.context_id.is_empty() {
            self.current_context_id()
        } else {
            self.sidebar.context_id.clone()
        };
        self.sidebar.context_id = context_id.clone();
        let title = if self.sidebar.title_generated {
            self.sidebar.conversation_title.clone()
        } else {
            self.sidebar
                .messages
                .iter()
                .find(|m| m.role == "user")
                .map(sidebar::intent_title)
                .unwrap_or_else(|| self.sidebar.conversation_title.clone())
        };
        self.sidebar.conversation_title = title.clone();
        let conv = crate::chat_store::StoredConversation {
            id: self.sidebar.conversation_id.clone(),
            title,
            title_generated: self.sidebar.title_generated,
            context_id,
            scene_id: None,
            updated_at: crate::chat_store::stamp_now(),
            messages: self.sidebar.to_stored_messages(),
            draft: self.sidebar.input.clone(),
            composer_mode: self.sidebar.composer_mode.clone(),
            attachment: self.sidebar.attachment.clone(),
            format_version: 2,
        };
        crate::chat_store::save_conversation(&project.root, &conv)
    }

    /// Ask the model for a short intent title once the conversation has its first user turn.
    fn request_conversation_title(&mut self) {
        if self.sidebar.title_generated
            || self.sidebar.conversation_id.is_empty()
            || self.title_pending.contains(&self.sidebar.conversation_id)
        {
            return;
        }
        let Some(first) = self.sidebar.messages.iter().find(|m| m.role == "user") else {
            return;
        };
        let request: String = first.content.chars().take(1500).collect();
        let Ok((profile, api_key)) = self.client_profile_key() else {
            return;
        };
        let system =
            "Tu nommes des conversations entre un auteur et son assistant d'écriture.".to_string();
        let user = format!(
            "Donne un titre très court (3 à 6 mots) qui résume l'intention de cette demande — \
             ce que l'auteur cherche à faire, pas une recopie du texte.\n\
             Pas de guillemets, pas de markdown, pas de ponctuation finale. Réponds uniquement par le titre.\n\n\
             Demande :\n{request}"
        );
        let conversation_id = self.sidebar.conversation_id.clone();
        self.title_pending.insert(conversation_id.clone());
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id: format!("title{}", now_stamp()),
                project_root,
                conversation_id,
                doc_id: String::new(),
                text_version: self.text_version,
                purpose: JobPurpose::Title,
            },
            kind: JobKind::Complete { system, user },
            profile,
            api_key,
        });
    }

    fn apply_generated_title(&mut self, conversation_id: &str, title: String) {
        if conversation_id == self.sidebar.conversation_id {
            self.sidebar.conversation_title = title;
            self.sidebar.title_generated = true;
            let _ = self.persist_chat();
            return;
        }
        let Some(project) = self.project.as_ref() else {
            return;
        };
        if let Some(mut conv) = crate::chat_store::load_conversation(&project.root, conversation_id)
        {
            conv.title = title;
            conv.title_generated = true;
            let _ = crate::chat_store::save_conversation(&project.root, &conv);
        }
    }

    fn refresh_chat_hints(&mut self) {
        if let Some(p) = self.llm.active() {
            self.sidebar.model_label = format!("{} / {}", p.name, p.model);
        }
        self.sidebar.context_hint = match &self.selection {
            NavSelection::Scene(id) => self
                .project
                .as_ref()
                .and_then(|p| p.manuscript.get_scene(id))
                .map(|s| {
                    let n = s.meta.links.len();
                    format!("Scène « {} » · {n} fiche(s)", s.meta.title)
                })
                .unwrap_or_default(),
            NavSelection::Note(id) => self
                .project
                .as_ref()
                .and_then(|p| p.notes.get(id))
                .map(|n| format!("Fiche « {} »", n.meta.name))
                .unwrap_or_default(),
            NavSelection::Styles => "Styles".into(),
            NavSelection::Publish => "Publication".into(),
        };
    }

    fn poll_llm_events(&mut self, ctx: &egui::Context) {
        let events = self.llm_worker.poll();
        if events.is_empty() {
            return;
        }
        ctx.request_repaint();
        let now = ctx.input(|i| i.time);
        for ev in events {
            self.handle_llm_event(ev, now);
        }
    }

    pub fn handle_llm_event(&mut self, ev: LlmEvent, _now: f64) {
        match ev {
            LlmEvent::Started { ids } => match ids.purpose {
                JobPurpose::Chat => self.sidebar.busy = true,
                JobPurpose::Blueprint => {}
                JobPurpose::Edit => {
                    if self.synopsis_job_id.as_deref() == Some(ids.job_id.as_str()) {
                        self.editor.synopsis_busy = true;
                        self.editor.synopsis_draft.clear();
                    }
                }
                JobPurpose::Ideas => self.sidebar.status = "Idées…".into(),
                JobPurpose::Review => self.sidebar.status = "Revue…".into(),
                JobPurpose::Title => {}
            },
            LlmEvent::Token { job_id, .. } if job_id.starts_with("title") => {}
            LlmEvent::Token { job_id, delta } => {
                if self.blueprint_job_id.as_deref() == Some(job_id.as_str()) {
                    return;
                }
                if self.synopsis_job_id.as_deref() == Some(job_id.as_str()) {
                    if self.editor.synopsis_busy {
                        self.editor.synopsis_draft.push_str(&delta);
                    }
                    return;
                }
                if let Some(m) = self.sidebar.messages.iter_mut().find(|m| m.id == job_id) {
                    m.status = MsgStatus::Streaming;
                    m.content.push_str(&delta);
                    if let crate::chat_store::MessagePayload::EditProposal {
                        proposal_status, ..
                    } = &mut m.payload
                    {
                        *proposal_status = crate::chat_store::EditProposalStatus::Streaming;
                    }
                    return;
                }
                if let Some(id) = &self.sidebar.streaming_id {
                    if *id == job_id {
                        if let Some(m) = self.sidebar.messages.iter_mut().find(|m| m.id == *id) {
                            m.status = MsgStatus::Streaming;
                            m.content.push_str(&delta);
                        }
                    } else {
                        self.pending_replies
                            .entry(job_id.clone())
                            .or_default()
                            .push_str(&delta);
                    }
                } else {
                    self.pending_replies
                        .entry(job_id.clone())
                        .or_default()
                        .push_str(&delta);
                }
            }
            LlmEvent::Done { ids, text } => {
                self.apply_llm_done(&ids, text, false, _now);
            }
            LlmEvent::Cancelled { ids, partial } => {
                self.apply_llm_done(&ids, partial, true, _now);
            }
            LlmEvent::Error { ids, message } => match ids.purpose {
                JobPurpose::Chat => {
                    if let Some(mid) = self.sidebar.streaming_id.take() {
                        if let Some(m) = self.sidebar.messages.iter_mut().find(|m| m.id == mid) {
                            m.status = MsgStatus::Error;
                            if m.content.is_empty() {
                                m.content = message.clone();
                            }
                        }
                    }
                    self.sidebar.busy = false;
                    self.sidebar.status = message;
                    let _ = self.persist_chat();
                }
                JobPurpose::Blueprint => {
                    self.blueprint_job_id = None;
                    self.set_toast(message);
                }
                JobPurpose::Edit => {
                    if self.synopsis_job_id.as_deref() == Some(ids.job_id.as_str()) {
                        self.synopsis_job_id = None;
                        self.editor.synopsis_busy = false;
                        self.set_toast(message);
                    } else if ids.conversation_id != self.sidebar.conversation_id {
                        self.merge_edit_reply_offscreen(&ids, "", true, Some(message));
                    } else {
                        let mut touched = false;
                        if let Some(m) = self
                            .sidebar
                            .messages
                            .iter_mut()
                            .find(|m| m.id == ids.job_id)
                        {
                            m.status = MsgStatus::Error;
                            if let crate::chat_store::MessagePayload::EditProposal {
                                proposal_status,
                                error,
                                ..
                            } = &mut m.payload
                            {
                                *proposal_status = crate::chat_store::EditProposalStatus::Error;
                                *error = Some(message.clone());
                            }
                            if m.content.is_empty() {
                                m.content = message;
                            }
                            touched = true;
                        }
                        if touched {
                            let _ = self.persist_chat();
                        }
                    }
                }
                JobPurpose::Ideas | JobPurpose::Review => {
                    self.sidebar.status = message;
                    self.pending_sidebar_job = None;
                }
                // Heuristic title stays; allow a retry on the next turn.
                JobPurpose::Title => {
                    self.title_pending.remove(&ids.conversation_id);
                }
            },
        }
    }

    fn merge_chat_reply_offscreen(&mut self, ids: &JobIds, text: &str, cancelled: bool) {
        let Some(project) = self.project.as_ref() else {
            self.sidebar.busy = false;
            return;
        };
        let Some(mut conv) =
            crate::chat_store::load_conversation(&project.root, &ids.conversation_id)
        else {
            self.sidebar.busy = false;
            return;
        };
        let body = if text.is_empty() {
            self.pending_replies.remove(&ids.job_id).unwrap_or_default()
        } else {
            text.to_string()
        };
        if let Some(m) = conv.messages.iter_mut().find(|m| m.id == ids.job_id) {
            m.content = body;
            m.status = if cancelled {
                crate::chat_store::StoredMsgStatus::Cancelled
            } else {
                crate::chat_store::StoredMsgStatus::Done
            };
        }
        let title = conv.title.clone();
        let _ = crate::chat_store::save_conversation(&project.root, &conv);
        if self.sidebar.streaming_id.as_deref() == Some(ids.job_id.as_str()) {
            self.sidebar.streaming_id = None;
        }
        self.sidebar.busy = false;
        self.set_toast(format!("Réponse enregistrée dans « {title} »."));
    }

    fn merge_edit_reply_offscreen(
        &mut self,
        ids: &JobIds,
        text: &str,
        cancelled: bool,
        error: Option<String>,
    ) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let Some(mut conv) =
            crate::chat_store::load_conversation(&project.root, &ids.conversation_id)
        else {
            return;
        };
        if let Some(m) = conv.messages.iter_mut().find(|m| m.id == ids.job_id) {
            if let Some(err) = error {
                m.status = crate::chat_store::StoredMsgStatus::Error;
                if m.content.is_empty() {
                    m.content = err.clone();
                }
                if let crate::chat_store::MessagePayload::EditProposal {
                    proposal_status,
                    error: e,
                    ..
                } = &mut m.payload
                {
                    *proposal_status = crate::chat_store::EditProposalStatus::Error;
                    *e = Some(err);
                }
            } else {
                if !text.is_empty() {
                    m.content = text.trim().to_string();
                } else {
                    m.content = m.content.trim().to_string();
                }
                if cancelled && m.content.is_empty() {
                    m.status = crate::chat_store::StoredMsgStatus::Cancelled;
                    if let crate::chat_store::MessagePayload::EditProposal {
                        proposal_status, ..
                    } = &mut m.payload
                    {
                        *proposal_status = crate::chat_store::EditProposalStatus::Rejected;
                    }
                } else {
                    m.status = crate::chat_store::StoredMsgStatus::Done;
                    if let crate::chat_store::MessagePayload::EditProposal {
                        proposal_status, ..
                    } = &mut m.payload
                    {
                        *proposal_status = crate::chat_store::EditProposalStatus::Ready;
                    }
                }
            }
        }
        let title = conv.title.clone();
        let _ = crate::chat_store::save_conversation(&project.root, &conv);
        self.set_toast(format!("Proposition IA enregistrée dans « {title} »."));
    }

    fn apply_llm_done(&mut self, ids: &JobIds, text: String, cancelled: bool, now: f64) {
        let current_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        if !ids.project_root.is_empty() && ids.project_root != current_root {
            self.set_toast("Réponse IA reçue pour un autre projet — ignorée.");
            return;
        }

        match ids.purpose {
            JobPurpose::Title => {
                self.title_pending.remove(&ids.conversation_id);
                let title = sidebar::clean_generated_title(&text);
                if cancelled || title.is_empty() {
                    return;
                }
                self.apply_generated_title(&ids.conversation_id, title);
            }
            JobPurpose::Chat => {
                if ids.conversation_id != self.sidebar.conversation_id {
                    self.merge_chat_reply_offscreen(ids, &text, cancelled);
                    return;
                }
                if let Some(mid) = self.sidebar.streaming_id.take() {
                    if let Some(m) = self.sidebar.messages.iter_mut().find(|m| m.id == mid) {
                        let merged = self.pending_replies.remove(&ids.job_id).unwrap_or_default();
                        if !text.is_empty() {
                            m.content = text;
                        } else if !merged.is_empty() {
                            m.content = merged;
                        }
                        m.status = if cancelled {
                            MsgStatus::Cancelled
                        } else {
                            MsgStatus::Done
                        };
                        m.offers_insert = looks_like_prose(&m.content);
                    }
                }
                self.sidebar.busy = false;
                self.sidebar.status.clear();
                let _ = self.persist_chat();
            }
            JobPurpose::Blueprint => {
                self.blueprint_job_id = None;
                if cancelled {
                    self.set_toast("Blueprint annulé.");
                    return;
                }
                match parse_blueprint(&text) {
                    Ok(bp) => {
                        self.blueprint_preview = Some(bp);
                        self.set_toast("Aperçu blueprint prêt.");
                    }
                    Err(e) => self.set_toast(e.to_string()),
                }
            }
            JobPurpose::Edit => {
                if self.synopsis_job_id.as_deref() == Some(ids.job_id.as_str()) {
                    self.synopsis_job_id = None;
                    self.editor.synopsis_busy = false;
                    if cancelled {
                        self.set_toast("Synopsis annulé.");
                        return;
                    }
                    if ids.doc_id != self.editor.id {
                        self.set_toast("Synopsis reçu pour une autre scène — ignoré.");
                        return;
                    }
                    let cleaned = if text.trim().is_empty() {
                        self.editor.synopsis_draft.trim().to_string()
                    } else {
                        text.trim()
                            .trim_matches(|c| c == '"' || c == '«' || c == '»')
                            .trim()
                            .to_string()
                    };
                    if !cleaned.is_empty() {
                        self.editor.synopsis = cleaned;
                        self.editor.synopsis_draft.clear();
                        self.editor.mark_dirty(now);
                    }
                    return;
                }
                if ids.conversation_id != self.sidebar.conversation_id {
                    self.merge_edit_reply_offscreen(ids, &text, cancelled, None);
                    return;
                }
                if let Some(m) = self
                    .sidebar
                    .messages
                    .iter_mut()
                    .find(|m| m.id == ids.job_id)
                {
                    if !text.is_empty() {
                        m.content = text.trim().to_string();
                    } else {
                        m.content = m.content.trim().to_string();
                    }
                    if cancelled && m.content.is_empty() {
                        m.status = MsgStatus::Cancelled;
                        if let crate::chat_store::MessagePayload::EditProposal {
                            proposal_status,
                            ..
                        } = &mut m.payload
                        {
                            *proposal_status = crate::chat_store::EditProposalStatus::Rejected;
                        }
                    } else {
                        m.status = MsgStatus::Done;
                        if let crate::chat_store::MessagePayload::EditProposal {
                            proposal_status,
                            ..
                        } = &mut m.payload
                        {
                            *proposal_status = crate::chat_store::EditProposalStatus::Ready;
                        }
                    }
                }
                if self.sidebar.streaming_id.as_deref() == Some(ids.job_id.as_str()) {
                    self.sidebar.streaming_id = None;
                }
                let _ = self.persist_chat();
            }
            JobPurpose::Ideas => {
                self.pending_sidebar_job = None;
                if cancelled {
                    self.sidebar.status = "Idées annulées.".into();
                    return;
                }
                self.sidebar.ideas = parse_ideas_json(&text);
                self.sidebar.status.clear();
                self.sidebar.tab = sidebar::SidebarTab::Ideas;
            }
            JobPurpose::Review => {
                let pending = self.pending_sidebar_job.take();
                if cancelled {
                    self.sidebar.status = "Revue annulée.".into();
                    return;
                }
                let (scene_id, draft) = match pending {
                    Some(PendingSidebarJob::Review {
                        scene_id, draft, ..
                    }) => (scene_id, draft),
                    _ => (ids.doc_id.clone(), self.editor.body.clone()),
                };
                match parse_review_response(&text, &scene_id, &draft) {
                    Ok(mut r) => {
                        if let Some(project) = self.project.as_ref() {
                            if let Err(e) = ak_coherence::save_review(&project.root, &mut r) {
                                self.set_toast(format!("Revue non enregistrée : {e}"));
                            }
                        }
                        if r.scene_id == self.editor.id {
                            self.sidebar.review = Some(r);
                            self.sidebar.tab = sidebar::SidebarTab::Reviews;
                        }
                        self.sidebar.status.clear();
                    }
                    Err(e) => self.sidebar.status = e.to_string(),
                }
            }
        }
    }

    fn client(&self) -> Result<OpenAiCompatClient, String> {
        let profile = self
            .llm
            .active()
            .cloned()
            .ok_or_else(|| "Aucun profil LLM.".to_string())?;
        let key = self.llm.get_api_key(&profile).map_err(|e| e.to_string())?;
        if profile.provider != ProviderKind::Ollama && key.is_none() {
            return Err(format!(
                "Pas de clé API pour « {} ». Ouvrez Paramètres → IA & clés.",
                profile.name
            ));
        }
        Ok(OpenAiCompatClient::new(profile, key))
    }

    fn run_chat_regenerate(&mut self, assistant_id: &str) {
        let idx = self
            .sidebar
            .messages
            .iter()
            .position(|m| m.id == assistant_id);
        let Some(ai_idx) = idx else {
            return;
        };
        let user_idx = self.sidebar.messages[..ai_idx]
            .iter()
            .rposition(|m| m.role == "user");
        let Some(u_idx) = user_idx else {
            return;
        };
        let user = self.sidebar.messages[u_idx].content.clone();
        self.sidebar.messages.truncate(u_idx + 1);
        self.sidebar
            .messages
            .push(ChatMsg::assistant_pending().with_id(assistant_id.to_string()));
        self.sidebar.streaming_id = Some(assistant_id.to_string());
        self.run_chat_with_user(user, Some(assistant_id.to_string()));
    }

    fn send_composer(&mut self) {
        if !self.sidebar.can_send() {
            return;
        }
        match self.sidebar.composer_mode.clone() {
            crate::chat_store::ComposerMode::Chat => {
                let note = self.sidebar.input.trim().to_string();
                let mut user = note.clone();
                let mut payload = crate::chat_store::MessagePayload::Chat;
                if let Some(att) = &self.sidebar.attachment {
                    let quote = att.text.trim().to_string();
                    let block = format!("Contexte :\n« {quote} »");
                    user = if user.is_empty() {
                        block
                    } else {
                        format!("{block}\n\n{user}")
                    };
                    payload = crate::chat_store::MessagePayload::UserQuote {
                        quote,
                        label: Some("Chat".into()),
                        note,
                        tool: Some("chat".into()),
                    };
                }
                if user.is_empty() {
                    return;
                }
                self.sidebar.input.clear();
                self.sidebar.attachment = None;
                self.run_chat_with_user_payload(user, None, payload);
            }
            crate::chat_store::ComposerMode::Continue { mode } => {
                let prompt = {
                    let t = self.sidebar.input.trim();
                    if t.is_empty() {
                        None
                    } else {
                        Some(t.to_string())
                    }
                };
                self.sidebar.input.clear();
                self.run_inline(Self::continue_mode_from_key(&mode), prompt);
                self.sidebar.composer_mode = crate::chat_store::ComposerMode::Chat;
                let _ = self.persist_chat();
            }
            crate::chat_store::ComposerMode::Brush { id } => {
                self.run_brush_from_attachment(&id);
                self.sidebar.composer_mode = crate::chat_store::ComposerMode::Chat;
                self.sidebar.attachment = None;
                self.sidebar.input.clear();
                let _ = self.persist_chat();
            }
        }
    }

    fn run_chat(&mut self) {
        let user = self.sidebar.input.trim().to_string();
        if user.is_empty() || self.sidebar.busy {
            return;
        }
        self.sidebar.input.clear();
        self.run_chat_with_user(user, None);
    }

    fn capture_selection_attachment(&self) -> Option<crate::chat_store::ComposerAttachment> {
        let selection = self.editor.selected_text()?.to_string();
        if selection.trim().is_empty() {
            return None;
        }
        let (a, b) = (
            self.editor.sel_start.min(self.editor.sel_end),
            self.editor.sel_start.max(self.editor.sel_end),
        );
        let (anchor_before, anchor_after) = text_anchors(&self.editor.body, a, b);
        let doc_kind = match self.editor.kind {
            EditorKind::Scene => "scene",
            EditorKind::Note => "note",
            _ => "",
        }
        .to_string();
        Some(crate::chat_store::ComposerAttachment {
            text: selection,
            range_start: a,
            range_end: b,
            doc_id: self.editor.id.clone(),
            doc_kind,
            anchor_before,
            anchor_after,
        })
    }

    fn stage_from_editor(
        &mut self,
        mode: crate::chat_store::ComposerMode,
        attach_selection: bool,
        input: Option<String>,
    ) {
        self.ensure_chat_conversation();
        self.sidebar.tab = sidebar::SidebarTab::Chat;
        self.sidebar.mode_menu_open = false;
        self.sidebar.composer_mode = mode;
        if attach_selection {
            match self.capture_selection_attachment() {
                Some(att) => self.sidebar.attachment = Some(att),
                None => {
                    self.set_toast("Sélectionnez du texte à transformer.");
                    return;
                }
            }
        }
        if let Some(text) = input {
            self.sidebar.input = text;
        }
        let _ = self.persist_chat();
        self.set_toast("Prêt dans le chat — envoyez ou effacez.");
    }

    pub fn stage_brush(&mut self, brush_id: &str) {
        if !self.brushes.iter().any(|b| b.id == brush_id) {
            return;
        }
        self.stage_from_editor(crate::chat_store::ComposerMode::brush(brush_id), true, None);
    }

    pub fn stage_continue(&mut self, mode: ContinueMode, prompt: Option<String>) {
        self.stage_from_editor(
            crate::chat_store::ComposerMode::continue_mode(Self::continue_mode_key(mode)),
            false,
            prompt,
        );
    }

    pub fn stage_selection_to_chat(&mut self) {
        self.ensure_chat_conversation();
        self.sidebar.tab = sidebar::SidebarTab::Chat;
        match self.capture_selection_attachment() {
            Some(att) => {
                self.sidebar.attachment = Some(att);
                if self.sidebar.composer_mode.is_chat() {
                    // keep Chat
                }
                let _ = self.persist_chat();
                self.set_toast("Sélection ajoutée au chat.");
            }
            None => self.set_toast("Sélectionnez du texte à ajouter au chat."),
        }
    }

    fn run_chat_with_user(&mut self, user: String, reuse_assistant_id: Option<String>) {
        self.run_chat_with_user_payload(
            user,
            reuse_assistant_id,
            crate::chat_store::MessagePayload::Chat,
        );
    }

    fn run_chat_with_user_payload(
        &mut self,
        user: String,
        reuse_assistant_id: Option<String>,
        user_payload: crate::chat_store::MessagePayload,
    ) {
        if self.sidebar.busy && reuse_assistant_id.is_none() {
            return;
        }
        let _ = self.flush_editor();
        let project_root = match self.project.as_ref() {
            Some(p) => p.root.display().to_string(),
            None => {
                self.set_toast("Ouvrez un projet.");
                return;
            }
        };
        self.ensure_chat_conversation();

        if reuse_assistant_id.is_none() {
            self.sidebar
                .messages
                .push(ChatMsg::user(&user).with_payload(user_payload));
            let assistant = ChatMsg::assistant_pending();
            let stream_id = assistant.id.clone();
            self.sidebar.messages.push(assistant);
            self.sidebar.streaming_id = Some(stream_id);
        }
        let stream_id = self
            .sidebar
            .streaming_id
            .clone()
            .unwrap_or_else(|| format!("chat{}", now_stamp()));
        self.sidebar.busy = true;
        self.sidebar.status = "Génération…".into();
        self.sidebar.stick_bottom = true;
        self.refresh_chat_hints();

        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                if let Some(m) = self.sidebar.messages.iter_mut().find(|m| m.id == stream_id) {
                    m.status = MsgStatus::Error;
                    m.content = e.clone();
                }
                self.sidebar.input = user;
                self.sidebar.busy = false;
                self.sidebar.status = e;
                self.sidebar.streaming_id = None;
                return;
            }
        };

        let prepared = self.build_request_context();
        let system = prepared.system;
        let history = prepared.history;
        let doc_id = self.editor.id.clone();
        let job = LlmJob {
            ids: JobIds {
                job_id: stream_id.clone(),
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id,
                text_version: self.text_version,
                purpose: JobPurpose::Chat,
            },
            kind: JobKind::Chat {
                system,
                history,
                user,
            },
            profile,
            api_key,
        };
        self.llm_worker.submit(job);
        let _ = self.persist_chat();
        self.request_conversation_title();
    }

    fn client_profile_key(&self) -> Result<(ak_llm::LlmProfile, Option<String>), String> {
        let profile = self
            .llm
            .active()
            .cloned()
            .ok_or_else(|| "Aucun profil LLM.".to_string())?;
        let key = self.llm.get_api_key(&profile).map_err(|e| e.to_string())?;
        if profile.provider != ProviderKind::Ollama && key.is_none() {
            return Err(format!(
                "Pas de clé API pour « {} ». Ouvrez Paramètres → IA & clés.",
                profile.name
            ));
        }
        Ok((profile, key))
    }

    fn current_system_prompt(&mut self) -> String {
        let _ = self.flush_editor();
        let Some(project) = self.project.as_ref() else {
            return String::new();
        };
        if let NavSelection::Scene(id) = &self.selection {
            if let Some(scene) = project.manuscript.get_scene(id) {
                let mut c = scene_context(project, scene);
                c.draft_body = self.editor.body.clone();
                return c.system_prompt();
            }
        }
        ak_llm::LlmContext {
            styles: project.styles.clone(),
            draft_body: self.editor.body.clone(),
            ..Default::default()
        }
        .system_prompt()
    }

    fn continue_mode_key(mode: ContinueMode) -> &'static str {
        match mode {
            ContinueMode::Sentence => "sentence",
            ContinueMode::Paragraph => "paragraph",
            ContinueMode::Continue => "continue",
            ContinueMode::Generate => "generate",
        }
    }

    fn continue_mode_from_key(key: &str) -> ContinueMode {
        match key {
            "sentence" => ContinueMode::Sentence,
            "paragraph" => ContinueMode::Paragraph,
            "generate" => ContinueMode::Generate,
            _ => ContinueMode::Continue,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_edit_proposal_messages(
        &mut self,
        job_id: &str,
        user_text: String,
        label: String,
        mode: crate::chat_store::EditApplyMode,
        range_start: usize,
        range_end: usize,
        original_slice: String,
        anchor_before: String,
        anchor_after: String,
        regen: crate::chat_store::EditRegenSpec,
        user_payload: crate::chat_store::MessagePayload,
    ) {
        self.ensure_chat_conversation();
        self.sidebar.tab = sidebar::SidebarTab::Chat;
        self.sidebar.stick_bottom = true;
        let doc_kind = match self.editor.kind {
            EditorKind::Scene => "scene",
            EditorKind::Note => "note",
            _ => "",
        }
        .to_string();
        self.sidebar
            .messages
            .push(ChatMsg::user(user_text).with_payload(user_payload));
        let payload = crate::chat_store::MessagePayload::EditProposal {
            label,
            mode,
            range_start,
            range_end,
            doc_id: self.editor.id.clone(),
            doc_kind,
            original_slice,
            anchor_before,
            anchor_after,
            proposal_status: crate::chat_store::EditProposalStatus::Streaming,
            conflict: false,
            error: None,
            job_id: job_id.to_string(),
            regen,
        };
        let mut assistant = ChatMsg::assistant_pending().with_id(job_id);
        assistant.status = MsgStatus::Streaming;
        assistant.payload = payload;
        self.sidebar.messages.push(assistant);
        self.sidebar.streaming_id = Some(job_id.to_string());
        let _ = self.persist_chat();
        self.request_conversation_title();
    }

    fn run_inline(&mut self, mode: ContinueMode, prompt: Option<String>) {
        let cursor = self.editor.sel_end.max(self.editor.sel_start);
        let tail: String = self
            .editor
            .body
            .chars()
            .take(cursor)
            .collect::<String>()
            .chars()
            .rev()
            .take(2000)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.set_toast(e);
                return;
            }
        };
        let mut system = self.current_system_prompt();
        if let Some(p) = &prompt {
            system.push_str("\n\nConsigne utilisateur: ");
            system.push_str(p);
        }
        let job_id = format!("edit{}", now_stamp());
        let (anchor_before, anchor_after) = text_anchors(&self.editor.body, cursor, cursor);
        let label = match mode {
            ContinueMode::Sentence => "Continuer (phrase)",
            ContinueMode::Paragraph => "Continuer (paragraphe)",
            ContinueMode::Generate => "Générer",
            ContinueMode::Continue => "Continuer",
        }
        .to_string();
        let user_text = if let Some(p) = &prompt {
            format!("{label} : {p}")
        } else {
            label.clone()
        };
        let user_payload = crate::chat_store::MessagePayload::UserQuote {
            quote: String::new(),
            label: Some(label.clone()),
            note: prompt.clone().unwrap_or_default(),
            tool: Some(format!("continue:{}", Self::continue_mode_key(mode))),
        };
        self.push_edit_proposal_messages(
            &job_id,
            user_text,
            label,
            crate::chat_store::EditApplyMode::InsertAtCursor,
            cursor,
            cursor,
            String::new(),
            anchor_before,
            anchor_after,
            crate::chat_store::EditRegenSpec::Continue {
                mode: Self::continue_mode_key(mode).into(),
                prompt: prompt.clone(),
            },
            user_payload,
        );
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id: self.editor.id.clone(),
                text_version: self.text_version,
                purpose: JobPurpose::Edit,
            },
            kind: JobKind::Continue { system, tail, mode },
            profile,
            api_key,
        });
    }

    fn run_brush(&mut self, brush_id: &str) {
        // Regen path: restage from current editor selection then send immediately.
        if let Some(att) = self.capture_selection_attachment() {
            self.sidebar.attachment = Some(att);
            self.run_brush_from_attachment(brush_id);
            self.sidebar.attachment = None;
        } else {
            self.set_toast("Sélectionnez du texte à transformer.");
        }
    }

    fn run_brush_from_attachment(&mut self, brush_id: &str) {
        let Some(brush) = self.brushes.iter().find(|b| b.id == brush_id).cloned() else {
            return;
        };
        let Some(att) = self.sidebar.attachment.clone() else {
            self.set_toast("Ajoutez une sélection au chat avant d'utiliser cet outil.");
            return;
        };
        if att.text.trim().is_empty() {
            self.set_toast("Sélectionnez du texte à transformer.");
            return;
        }
        if att.doc_id != self.editor.id {
            self.set_toast("La sélection appartient à un autre document — rouvrez-le.");
            return;
        }
        let _ = self.snapshot_editor("avant outil");
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.set_toast(e);
                return;
            }
        };
        let mut system = self.current_system_prompt();
        let extra = self.sidebar.input.trim();
        if !extra.is_empty() {
            system.push_str("\n\nConsigne utilisateur: ");
            system.push_str(extra);
        }
        let job_id = format!("edit{}", now_stamp());
        let excerpt: String = att.text.chars().take(280).collect();
        let user_text = format!("Outil : {}\n\n« {} »", brush.label, excerpt);
        let user_payload = crate::chat_store::MessagePayload::UserQuote {
            quote: att.text.trim().to_string(),
            label: Some(brush.label.clone()),
            note: extra.to_string(),
            tool: Some(format!("brush:{}", brush.id)),
        };
        self.push_edit_proposal_messages(
            &job_id,
            user_text,
            brush.label.clone(),
            crate::chat_store::EditApplyMode::ReplaceSelection,
            att.range_start,
            att.range_end,
            att.text.clone(),
            att.anchor_before.clone(),
            att.anchor_after.clone(),
            crate::chat_store::EditRegenSpec::Brush {
                brush_id: brush.id.clone(),
            },
            user_payload,
        );
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id: att.doc_id,
                text_version: self.text_version,
                purpose: JobPurpose::Edit,
            },
            kind: JobKind::Transform {
                system,
                instruction: brush.instruction.clone(),
                selection: att.text,
            },
            profile,
            api_key,
        });
    }

    fn edit_proposal_job_id(&self, msg_id: &str) -> Option<String> {
        self.sidebar.messages.iter().find_map(|m| {
            if m.id != msg_id {
                return None;
            }
            match &m.payload {
                crate::chat_store::MessagePayload::EditProposal { job_id, .. } => {
                    Some(job_id.clone())
                }
                _ => None,
            }
        })
    }

    fn find_edit_proposal_mut(&mut self, msg_id: &str) -> Option<&mut ChatMsg> {
        self.sidebar.messages.iter_mut().find(|m| m.id == msg_id)
    }

    fn accept_edit_proposal(&mut self, msg_id: &str, now: f64) {
        let Some(msg) = self
            .sidebar
            .messages
            .iter()
            .find(|m| m.id == msg_id)
            .cloned()
        else {
            return;
        };
        let crate::chat_store::MessagePayload::EditProposal {
            mode,
            range_start,
            range_end,
            doc_id,
            original_slice,
            anchor_before,
            anchor_after,
            ..
        } = msg.payload.clone()
        else {
            return;
        };
        if doc_id != self.editor.id {
            self.set_toast("Proposition IA pour un autre document — ouvrez-le pour appliquer.");
            return;
        }
        let _ = self.snapshot_editor("avant accept IA");
        match resolve_anchor(
            &self.editor.body,
            range_start,
            range_end,
            &original_slice,
            &anchor_before,
            &anchor_after,
            mode,
        ) {
            Ok((start, end)) => {
                let after =
                    apply_proposal_to_body(&self.editor.body, start, end, mode, msg.content.trim());
                self.editor.apply_body_change(after, now);
                self.text_version += 1;
                self.editor.text_version += 1;
                if let Some(m) = self.find_edit_proposal_mut(msg_id) {
                    if let crate::chat_store::MessagePayload::EditProposal {
                        proposal_status,
                        conflict,
                        ..
                    } = &mut m.payload
                    {
                        *proposal_status = crate::chat_store::EditProposalStatus::Accepted;
                        *conflict = false;
                    }
                    m.status = MsgStatus::Done;
                }
                let _ = self.persist_chat();
                self.set_toast("Modification IA validée (annulable).");
            }
            Err(AnchorError) => {
                if let Some(m) = self.find_edit_proposal_mut(msg_id) {
                    if let crate::chat_store::MessagePayload::EditProposal {
                        proposal_status,
                        conflict,
                        ..
                    } = &mut m.payload
                    {
                        *proposal_status = crate::chat_store::EditProposalStatus::Conflict;
                        *conflict = true;
                    }
                }
                let _ = self.persist_chat();
                self.set_toast("Le passage a changé");
            }
        }
    }

    fn reject_edit_proposal(&mut self, msg_id: &str) {
        if let Some(job) = self.edit_proposal_job_id(msg_id) {
            self.llm_worker.cancel_job(&job);
        }
        if let Some(m) = self.find_edit_proposal_mut(msg_id) {
            if let crate::chat_store::MessagePayload::EditProposal {
                proposal_status,
                conflict,
                ..
            } = &mut m.payload
            {
                *proposal_status = crate::chat_store::EditProposalStatus::Rejected;
                *conflict = false;
            }
            m.status = MsgStatus::Cancelled;
        }
        if self.sidebar.streaming_id.as_deref() == Some(msg_id) {
            self.sidebar.streaming_id = None;
        }
        let _ = self.persist_chat();
    }

    fn regenerate_edit_proposal(&mut self, msg_id: &str) {
        let Some(msg) = self
            .sidebar
            .messages
            .iter()
            .find(|m| m.id == msg_id)
            .cloned()
        else {
            return;
        };
        let crate::chat_store::MessagePayload::EditProposal { regen, job_id, .. } = msg.payload
        else {
            return;
        };
        self.llm_worker.cancel_job(&job_id);
        if let Some(m) = self.find_edit_proposal_mut(msg_id) {
            if let crate::chat_store::MessagePayload::EditProposal {
                proposal_status, ..
            } = &mut m.payload
            {
                *proposal_status = crate::chat_store::EditProposalStatus::Rejected;
            }
            m.status = MsgStatus::Cancelled;
        }
        if self.sidebar.streaming_id.as_deref() == Some(msg_id) {
            self.sidebar.streaming_id = None;
        }
        let _ = self.persist_chat();
        match regen {
            crate::chat_store::EditRegenSpec::Brush { brush_id } => self.run_brush(&brush_id),
            crate::chat_store::EditRegenSpec::Continue { mode, prompt } => {
                self.run_inline(Self::continue_mode_from_key(&mode), prompt)
            }
        }
    }

    fn run_generate_synopsis(&mut self) {
        if self.editor.kind != EditorKind::Scene {
            return;
        }
        if self.editor.synopsis_busy || self.synopsis_job_id.is_some() {
            return;
        }
        let body = self.editor.body.trim();
        if body.is_empty() {
            self.set_toast("Écrivez d'abord un peu de texte dans la scène.");
            return;
        }
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.set_toast(e);
                return;
            }
        };
        let title = self.editor.title.clone();
        let excerpt: String = body.chars().take(4000).collect();
        let system = self.current_system_prompt();
        let user = format!(
            "Rédige un synopsis court (1 à 2 phrases) pour cette scène.\n\
             Style neutre, présent ou passé selon le texte. Pas de guillemets, pas de markdown, pas de préambule — uniquement le synopsis.\n\n\
             Titre : {title}\n\n\
             Texte :\n{excerpt}"
        );
        let job_id = format!("syn{}", now_stamp());
        self.synopsis_job_id = Some(job_id.clone());
        self.editor.synopsis_busy = true;
        self.editor.synopsis_draft.clear();
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id: self.editor.id.clone(),
                text_version: self.text_version,
                purpose: JobPurpose::Edit,
            },
            kind: JobKind::Complete { system, user },
            profile,
            api_key,
        });
    }

    fn run_ideas(&mut self) {
        if self.llm_worker.is_purpose_busy(JobPurpose::Ideas) {
            return;
        }
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.sidebar.status = e;
                return;
            }
        };
        let system = self.current_system_prompt();
        let user = "Propose 5 idées concrètes pour la suite de cette scène / chapitre.\n\
             JSON strict: {\"ideas\":[{\"title\":\"...\",\"pitch\":\"...\"}]}\n\
             Sans markdown hors JSON.";
        let job_id = format!("ideas{}", now_stamp());
        self.pending_sidebar_job = Some(PendingSidebarJob::Ideas);
        self.sidebar.status = "Idées…".into();
        self.sidebar.tab = sidebar::SidebarTab::Ideas;
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id: self.editor.id.clone(),
                text_version: self.text_version,
                purpose: JobPurpose::Ideas,
            },
            kind: JobKind::Complete {
                system,
                user: user.into(),
            },
            profile,
            api_key,
        });
    }

    fn run_review_action(&mut self) {
        if self.llm_worker.is_purpose_busy(JobPurpose::Review) {
            return;
        }
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.sidebar.status = e;
                return;
            }
        };
        let system = self.current_system_prompt();
        let user = "Fais une revue de cohérence locale de la scène / draft fourni.\n\
             Cherche: contradictions avec les Notes, trous de motivation, problèmes de voix, \
             anachronismes internes, détails oubliés.\n\
             Réponds en JSON strict avec la forme:\n\
             {\"items\":[{\"severity\":\"info|warn|error\",\"title\":\"...\",\"detail\":\"...\",\
             \"suggestion\":\"correction proposée\",\"passage\":\"extrait EXACT du draft concerné (optionnel)\"}]}\n\
             Si tout est cohérent, réponds {\"items\":[]}.\n\
             Sans markdown, sans commentaire hors JSON.";
        let scene_id = self.editor.id.clone();
        let draft = self.editor.body.clone();
        let job_id = format!("rev{}", now_stamp());
        self.pending_sidebar_job = Some(PendingSidebarJob::Review {
            scene_id: scene_id.clone(),
            draft: draft.clone(),
        });
        self.sidebar.status = "Revue…".into();
        self.sidebar.tab = sidebar::SidebarTab::Reviews;
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: self.sidebar.conversation_id.clone(),
                doc_id: scene_id,
                text_version: self.text_version,
                purpose: JobPurpose::Review,
            },
            kind: JobKind::Complete {
                system,
                user: user.into(),
            },
            profile,
            api_key,
        });
    }

    fn current_scene(&self) -> Option<&ak_core::Scene> {
        let project = self.project.as_ref()?;
        match &self.selection {
            NavSelection::Scene(id) => project.manuscript.get_scene(id),
            _ => None,
        }
    }

    fn create_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Dossier du nouveau projet")
            .pick_folder()
        {
            let title = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Mon roman")
                .to_string();
            if self.project.is_some() {
                if let Err(e) = self.flush_editor() {
                    self.set_toast(format!(
                        "Impossible de créer un projet — enregistrement échoué : {e}"
                    ));
                    return;
                }
            }
            match Project::create(&path, &title, "") {
                Ok(p) => {
                    if let Some(scene) = p.manuscript.ordered_scenes().first() {
                        self.selection = NavSelection::Scene(scene.meta.id.clone());
                    }
                    self.prefs.push_recent(&p.root);
                    self.project = Some(p);
                    self.editor = EditorState::default();
                    self.sidebar = SidebarState::default();
                    self.set_toast("Projet créé.");
                }
                Err(e) => self.set_toast(format!("Erreur: {e}")),
            }
        }
    }

    fn close_project(&mut self) {
        if self.project.is_some() {
            if let Err(e) = self.flush_editor() {
                self.set_toast(format!(
                    "Impossible de fermer — enregistrement échoué : {e}"
                ));
                return;
            }
            let _ = self.persist_chat();
        }
        self.project = None;
        self.selection = NavSelection::Scene(String::new());
        self.editor = EditorState::default();
        self.sidebar = SidebarState::default();
        self.synopsis_job_id = None;
        self.pending_sidebar_job = None;
        self.blueprint_preview = None;
        self.versions_open = false;
        self.trash_open = false;
        self.search_open = false;
        self.import_open = false;
        self.read_through_open = false;
        self.styles_open = false;
        self.publish_open = false;
        self.open_diagnostics.clear();
        self.external_conflict = None;
    }

    fn open_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Ouvrir un projet AuthorKit")
            .pick_folder()
        {
            self.open_project_at(&path);
        }
    }

    fn open_project_at(&mut self, path: &std::path::Path) {
        if self.project.is_some() {
            if let Err(e) = self.flush_editor() {
                self.set_toast(format!(
                    "Impossible de changer de projet — enregistrement échoué : {e}"
                ));
                return;
            }
        }
        match Project::open_with_diagnostics(path) {
            Ok(opened) => {
                let p = opened.project;
                if let Some(scene) = p.manuscript.ordered_scenes().first() {
                    self.selection = NavSelection::Scene(scene.meta.id.clone());
                }
                self.prefs.push_recent(&p.root);
                self.open_diagnostics = opened
                    .diagnostics
                    .items
                    .iter()
                    .map(|d| d.message.clone())
                    .collect();
                if !self.open_diagnostics.is_empty() {
                    let n = self.open_diagnostics.len();
                    self.set_toast(format!(
                        "Projet ouvert avec {n} alerte(s) — voir les diagnostics."
                    ));
                }
                let context_id = crate::chat_store::context_id_for(&self.selection);
                let conv = crate::chat_store::last_or_empty(&p.root, &context_id);
                self.project = Some(p);
                self.editor = EditorState::default();
                self.sidebar = SidebarState::from_stored(conv);
                self.refresh_chat_hints();
            }
            Err(e) => self.set_toast(format!("Erreur: {e}")),
        }
    }

    fn handle_nav(&mut self, action: NavAction) {
        match action {
            NavAction::ManuscriptHeight(h) => {
                self.prefs.manuscript_height = h;
                self.prefs.save();
            }
            NavAction::Select(sel) => self.apply_selection(sel),
            NavAction::AddAct => self.add_act(),
            NavAction::AddChapter { act_id } => self.add_chapter(&act_id),
            NavAction::AddScene { act_id, chapter_id } => {
                self.create = Some(OpenModal::new((
                    PendingCreate::Scene { act_id, chapter_id },
                    String::new(),
                )));
            }
            NavAction::AddNote(kind) => {
                self.create = Some(OpenModal::new((
                    PendingCreate::Note { kind },
                    String::new(),
                )));
            }
            NavAction::RenameAct { id, title } => {
                self.rename = Some(OpenModal::new((RenameTarget::Act { id }, title)));
            }
            NavAction::RenameChapter { act_id, id, title } => {
                self.rename = Some(OpenModal::new((
                    RenameTarget::Chapter { act_id, id },
                    title,
                )));
            }
            NavAction::RenameScene { id, title } => {
                self.rename = Some(OpenModal::new((RenameTarget::Scene { id }, title)));
            }
            NavAction::RenameNote { id, name } => {
                self.rename = Some(OpenModal::new((RenameTarget::Note { id }, name)));
            }
            NavAction::DeleteAct { id } => {
                let title = self
                    .project
                    .as_ref()
                    .and_then(|p| p.manuscript.index.acts.iter().find(|a| a.id == id))
                    .map(|a| a.title.clone())
                    .unwrap_or_else(|| "Acte".into());
                self.pending_delete = Some(OpenModal::new(PendingDelete::Act { id, title }));
            }
            NavAction::DeleteChapter { act_id, id } => {
                let title = self
                    .project
                    .as_ref()
                    .and_then(|p| {
                        p.manuscript
                            .index
                            .acts
                            .iter()
                            .find(|a| a.id == act_id)
                            .and_then(|a| a.chapters.iter().find(|c| c.id == id))
                    })
                    .map(|c| c.title.clone())
                    .unwrap_or_else(|| "Chapitre".into());
                self.pending_delete =
                    Some(OpenModal::new(PendingDelete::Chapter { act_id, id, title }));
            }
            NavAction::DeleteScene { id } => {
                let title = self
                    .project
                    .as_ref()
                    .and_then(|p| p.manuscript.get_scene(&id))
                    .map(|s| s.meta.title.clone())
                    .unwrap_or_else(|| "Scène".into());
                self.pending_delete = Some(OpenModal::new(PendingDelete::Scene { id, title }));
            }
            NavAction::DeleteNote { id } => {
                let title = self
                    .project
                    .as_ref()
                    .and_then(|p| p.notes.get(&id))
                    .map(|n| n.meta.name.clone())
                    .unwrap_or_else(|| "Note".into());
                self.pending_delete = Some(OpenModal::new(PendingDelete::Note { id, title }));
            }
            NavAction::MoveSceneUp { id } => self.move_scene(&id, -1),
            NavAction::MoveSceneDown { id } => self.move_scene(&id, 1),
            NavAction::MoveChapterUp { act_id, id } => self.move_chapter(&act_id, &id, -1),
            NavAction::MoveChapterDown { act_id, id } => self.move_chapter(&act_id, &id, 1),
            NavAction::MoveActUp { id } => self.move_act(&id, -1),
            NavAction::MoveActDown { id } => self.move_act(&id, 1),
            NavAction::MoveSceneTo { id } => {
                self.move_target = Some(OpenModal::new(MoveTarget::Scene { id }));
            }
            NavAction::MoveChapterTo { act_id, id } => {
                self.move_target = Some(OpenModal::new(MoveTarget::Chapter { act_id, id }));
            }
            NavAction::ReadChapter { act_id, id } => {
                if let Some(p) = self.project.as_ref() {
                    self.read_through_text = crate::ms_tools::chapter_read_through(p, &act_id, &id);
                    self.read_through_open = true;
                }
            }
        }
    }

    fn move_scene(&mut self, scene_id: &str, delta: i32) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let loc = project
            .manuscript
            .index
            .acts
            .iter()
            .enumerate()
            .find_map(|(ai, a)| {
                a.chapters.iter().enumerate().find_map(|(ci, c)| {
                    c.scene_ids
                        .iter()
                        .position(|id| id == scene_id)
                        .map(|si| (a.id.clone(), c.id.clone(), ai, ci, si))
                })
            });
        let Some((act_id, ch_id, _ai, _ci, si)) = loc else {
            return;
        };
        let new_i = (si as i32 + delta).max(0) as usize;
        if let Err(e) =
            project
                .manuscript
                .move_scene(scene_id, &act_id, &ch_id, new_i, &project.root)
        {
            self.set_toast(e.to_string());
        }
    }

    fn move_chapter(&mut self, act_id: &str, chapter_id: &str, delta: i32) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let pos = project
            .manuscript
            .index
            .acts
            .iter()
            .find(|a| a.id == act_id)
            .and_then(|a| a.chapters.iter().position(|c| c.id == chapter_id));
        let Some(pos) = pos else {
            return;
        };
        let new_i = (pos as i32 + delta).max(0) as usize;
        if let Err(e) =
            project
                .manuscript
                .move_chapter(act_id, chapter_id, act_id, new_i, &project.root)
        {
            self.set_toast(e.to_string());
        }
    }

    fn move_act(&mut self, act_id: &str, delta: i32) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let pos = project
            .manuscript
            .index
            .acts
            .iter()
            .position(|a| a.id == act_id);
        let Some(pos) = pos else {
            return;
        };
        let new_i = (pos as i32 + delta).max(0) as usize;
        if let Err(e) = project.manuscript.move_act(act_id, new_i, &project.root) {
            self.set_toast(e.to_string());
        }
    }

    fn rename_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.rename.as_mut() else {
            return;
        };
        let mut open = modal.open;
        let (target, mut draft) = modal.data.clone();
        let mut save = false;
        let mut cancel = false;
        let lang = self.prefs.locale;
        let title = match &target {
            RenameTarget::Act { .. } => "Renommer l'acte",
            RenameTarget::Chapter { .. } => "Renommer le chapitre",
            RenameTarget::Scene { .. } => "Renommer la scène",
            RenameTarget::Note { .. } => "Renommer",
        };
        let dark = Theme::current(ctx).dark;
        widgets::FadeDialog::new("ak_rename", title)
            .width(400.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let _ = ui.add(
                    Input::new(&mut draft)
                        .hint(lang.name_hint())
                        .max_width(f32::INFINITY),
                );
                ui.add_space(12.0);
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary(ui, lang.save()).clicked() {
                        save = true;
                    }
                    if widgets::ghost(ui, lang.cancel()).clicked() {
                        cancel = true;
                    }
                });
            });
        if save {
            let name = draft.trim().to_string();
            if name.is_empty() {
                self.set_toast(lang.name_required());
            } else {
                if let Some(project) = self.project.as_mut() {
                    let result =
                        match &target {
                            RenameTarget::Act { id } => {
                                project.manuscript.rename_act(id, &name, &project.root)
                            }
                            RenameTarget::Chapter { act_id, id } => project
                                .manuscript
                                .rename_chapter(act_id, id, &name, &project.root),
                            RenameTarget::Scene { id } => {
                                let r = project.manuscript.rename_scene(id, &name);
                                if r.is_ok() && self.editor.id == *id {
                                    self.editor.title = name.clone();
                                }
                                r
                            }
                            RenameTarget::Note { id } => {
                                let r = project.notes.rename(id, &name);
                                if r.is_ok() && self.editor.id == *id {
                                    self.editor.title = name.clone();
                                }
                                r
                            }
                        };
                    self.set_toast(match result {
                        Ok(()) => "Renommé.".into(),
                        Err(e) => e.to_string(),
                    });
                }
                open = false;
            }
        } else if cancel {
            open = false;
        }
        if let Some(modal) = self.rename.as_mut() {
            modal.open = open;
            modal.data.1 = draft;
        }
        if widgets::dialog_settled(ctx, "ak_rename", open) {
            self.rename = None;
        }
    }

    fn create_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.create.as_mut() else {
            return;
        };
        let mut open = modal.open;
        let (target, mut draft) = modal.data.clone();
        let mut confirm = false;
        let mut cancel = false;
        let lang = self.prefs.locale;
        let title = match &target {
            PendingCreate::Scene { .. } => lang.new_scene_title().to_string(),
            PendingCreate::Note { kind } => lang.create_note_title(*kind),
        };
        let dark = Theme::current(ctx).dark;
        widgets::FadeDialog::new("ak_create", &title)
            .width(400.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                let resp = ui.add(
                    Input::new(&mut draft)
                        .hint(lang.name_hint())
                        .max_width(f32::INFINITY),
                );
                let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(12.0);
                widgets::modal_footer(ui, |ui| {
                    if widgets::primary(ui, lang.create()).clicked() || enter {
                        confirm = true;
                    }
                    if widgets::ghost(ui, lang.cancel()).clicked() {
                        cancel = true;
                    }
                });
            });
        if confirm {
            let name = draft.trim().to_string();
            if name.is_empty() {
                self.set_toast(lang.name_required());
            } else {
                match &target {
                    PendingCreate::Scene { act_id, chapter_id } => {
                        self.add_scene_in(act_id, chapter_id, &name)
                    }
                    PendingCreate::Note { kind } => self.add_note_named(*kind, &name),
                }
                open = false;
            }
        } else if cancel {
            open = false;
        }
        if let Some(modal) = self.create.as_mut() {
            modal.open = open;
            modal.data.1 = draft;
        }
        if widgets::dialog_settled(ctx, "ak_create", open) {
            self.create = None;
        }
    }

    fn add_act(&mut self) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let n = project.manuscript.index.acts.len() + 1;
        let act_id = project.manuscript.add_act(&format!("Acte {n}"));
        let _ = project
            .manuscript
            .add_chapter(&act_id, "Chapitre 1")
            .and_then(|ch| {
                project
                    .manuscript
                    .add_scene(&project.root, &act_id, &ch, "Nouvelle scène")
            });
        let _ = project.manuscript.save_index(&project.root);
        if let Some(scene) = project.manuscript.ordered_scenes().last() {
            self.selection = NavSelection::Scene(scene.meta.id.clone());
            self.editor = EditorState::default();
        }
        self.set_toast("Acte ajouté.");
    }

    fn add_chapter(&mut self, act_id: &str) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let n = project
            .manuscript
            .index
            .acts
            .iter()
            .find(|a| a.id == act_id)
            .map(|a| a.chapters.len() + 1)
            .unwrap_or(1);
        match project
            .manuscript
            .add_chapter(act_id, &format!("Chapitre {n}"))
        {
            Ok(ch_id) => {
                let _ = project.manuscript.save_index(&project.root);
                match project
                    .manuscript
                    .add_scene(&project.root, act_id, &ch_id, "Nouvelle scène")
                {
                    Ok(scene) => {
                        let id = scene.meta.id.clone();
                        self.selection = NavSelection::Scene(id);
                        self.editor = EditorState::default();
                        self.set_toast("Chapitre ajouté.");
                    }
                    Err(e) => self.set_toast(e.to_string()),
                }
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }

    fn add_scene_in(&mut self, act_id: &str, chapter_id: &str, title: &str) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        match project
            .manuscript
            .add_scene(&project.root, act_id, chapter_id, title)
        {
            Ok(scene) => {
                let id = scene.meta.id.clone();
                self.selection = NavSelection::Scene(id);
                self.editor = EditorState::default();
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }

    fn add_note_named(&mut self, kind: NoteKind, name: &str) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        match project.notes.create(&project.root, kind, name) {
            Ok(note) => {
                let id = note.meta.id.clone();
                self.selection = NavSelection::Note(id);
                self.editor = EditorState::default();
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }

    fn do_export(&mut self, kind: ExportKind) {
        if let Err(e) = self.flush_editor() {
            self.set_toast(format!("Enregistrement requis avant export : {e}"));
            return;
        }
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let (title, filter) = match kind {
            ExportKind::Markdown => ("Exporter Markdown", "md"),
            ExportKind::Epub => ("Exporter EPUB", "epub"),
            ExportKind::Docx => ("Exporter DOCX", "docx"),
        };
        let default_name = format!("{}.{}", slug_simple(&project.meta.title), filter);
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(&default_name)
            .save_file()
        else {
            return;
        };
        let opts = &self.export_options;
        let result = match kind {
            ExportKind::Markdown => {
                export_markdown_with(project, &path, opts).map_err(|e| e.to_string())
            }
            ExportKind::Epub => export_epub_with(project, &path, opts).map_err(|e| e.to_string()),
            ExportKind::Docx => export_docx_with(project, &path, opts).map_err(|e| e.to_string()),
        };
        self.set_toast(match result {
            Ok(()) => format!("Exporté: {}", path.display()),
            Err(e) => e,
        });
    }

    fn bootstrap_modal(&mut self, ctx: &egui::Context) {
        let mut open = self.bootstrap_open;
        let mut generate = false;
        let mut apply_new = false;
        let mut apply_existing = false;
        let mut cancel = false;
        let dark = Theme::current(ctx).dark;
        let has_preview = self.blueprint_preview.is_some();
        let title = if has_preview {
            "Aperçu du blueprint"
        } else {
            "Nouvelle idée de roman"
        };
        widgets::FadeDialog::new("ak_bootstrap", title)
            .description(if has_preview {
                "Vérifiez le plan avant application. Par défaut : nouveau projet."
            } else {
                "Décrivez personnages, premise et ton."
            })
            .width(520.0)
            .backdrop(theme::modal_backdrop(dark))
            .show(ctx, &mut open, |ui| {
                if let Some(bp) = &self.blueprint_preview {
                    ui.label(RichText::new(&bp.title).size(FONT_TITLE));
                    ui.label(RichText::new(&bp.premise).weak());
                    ui.add_space(8.0);
                    ui.label(format!("{} chapitres", bp.chapters.len()));
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for ch in &bp.chapters {
                                ui.label(format!("• {} — {}", ch.title, ch.summary));
                            }
                        });
                    ui.add_space(6.0);
                    ui.label(format!("{} personnages", bp.characters.len()));
                    for c in &bp.characters {
                        ui.label(format!("• {}", c.name));
                    }
                    ui.add_space(12.0);
                    widgets::modal_footer(ui, |ui| {
                        if widgets::primary(ui, "Créer un nouveau projet").clicked() {
                            apply_new = true;
                        }
                        if self.project.is_some()
                            && widgets::ghost(ui, "Appliquer à l’existant").clicked()
                        {
                            apply_existing = true;
                        }
                        if widgets::ghost(ui, "Annuler").clicked() {
                            cancel = true;
                        }
                    });
                } else {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.bootstrap_idea)
                            .desired_rows(8)
                            .desired_width(f32::INFINITY),
                    );
                    if self.blueprint_job_id.is_some() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label("Génération du blueprint…");
                        });
                    }
                    ui.add_space(12.0);
                    widgets::modal_footer(ui, |ui| {
                        let busy = self.blueprint_job_id.is_some();
                        if !busy && widgets::primary(ui, "Générer l’aperçu").clicked() {
                            generate = true;
                        }
                        if busy && widgets::ghost(ui, "Annuler").clicked() {
                            if let Some(jid) = self.blueprint_job_id.take() {
                                self.llm_worker.cancel_job(&jid);
                            }
                        }
                        if widgets::ghost(ui, "Annuler").clicked() {
                            cancel = true;
                        }
                    });
                }
            });
        if generate {
            self.run_bootstrap_generate();
        } else if apply_new {
            self.blueprint_apply_existing = false;
            self.run_bootstrap_apply();
            open = false;
        } else if apply_existing {
            self.blueprint_apply_existing = true;
            self.run_bootstrap_apply();
            open = false;
        } else if cancel {
            self.blueprint_preview = None;
            open = false;
        }
        self.bootstrap_open = open;
    }

    fn run_bootstrap_generate(&mut self) {
        let idea = self.bootstrap_idea.clone();
        if idea.trim().is_empty() {
            self.set_toast("Décrivez d'abord l'idée.");
            return;
        }
        if self.blueprint_job_id.is_some() {
            return;
        }
        let (profile, api_key) = match self.client_profile_key() {
            Ok(v) => v,
            Err(e) => {
                self.set_toast(e);
                return;
            }
        };
        let (system, user) = blueprint_prompts(&idea);
        let job_id = format!("bp{}", now_stamp());
        self.blueprint_job_id = Some(job_id.clone());
        let project_root = self
            .project
            .as_ref()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        self.llm_worker.submit(LlmJob {
            ids: JobIds {
                job_id,
                project_root,
                conversation_id: String::new(),
                doc_id: String::new(),
                text_version: 0,
                purpose: JobPurpose::Blueprint,
            },
            kind: JobKind::Complete { system, user },
            profile,
            api_key,
        });
        self.set_toast("Génération du blueprint…");
    }

    fn run_bootstrap_apply(&mut self) {
        let Some(bp) = self.blueprint_preview.clone() else {
            return;
        };
        if !self.blueprint_apply_existing {
            // Default: new project folder
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Dossier pour le nouveau roman")
                .pick_folder()
            {
                if self.project.is_some() {
                    if let Err(e) = self.flush_editor() {
                        self.set_toast(format!("Enregistrement échoué : {e}"));
                        return;
                    }
                }
                match Project::create(&path, &bp.title, "") {
                    Ok(mut p) => match apply_blueprint(&mut p, &bp) {
                        Ok(msg) => {
                            if let Some(scene) = p.manuscript.ordered_scenes().first() {
                                self.selection = NavSelection::Scene(scene.meta.id.clone());
                            }
                            self.prefs.push_recent(&p.root);
                            self.project = Some(p);
                            self.editor = EditorState::default();
                            self.blueprint_preview = None;
                            self.bootstrap_idea.clear();
                            self.set_toast(msg);
                        }
                        Err(e) => self.set_toast(e.to_string()),
                    },
                    Err(e) => self.set_toast(format!("Création refusée : {e}")),
                }
            }
            return;
        }
        // Explicit apply to existing — manuscript acts go to trash first.
        if let Err(e) = self.flush_editor() {
            self.set_toast(format!("Enregistrement échoué : {e}"));
            return;
        }
        let _ = self.snapshot_editor("avant blueprint");
        let Some(project) = self.project.as_mut() else {
            self.set_toast("Aucun projet ouvert.");
            return;
        };
        match apply_blueprint(project, &bp) {
            Ok(msg) => {
                if let Some(scene) = project.manuscript.ordered_scenes().first() {
                    self.selection = NavSelection::Scene(scene.meta.id.clone());
                }
                self.editor = EditorState::default();
                self.blueprint_preview = None;
                self.bootstrap_idea.clear();
                self.set_toast(format!("{msg} (ancien manuscrit en corbeille)"));
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }
}

enum ExportKind {
    Markdown,
    Epub,
    Docx,
}

fn now_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Mask a secret for display: keep a short prefix + last 4 chars.
fn mask_api_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let n = chars.len();
    if n == 0 {
        return "••••".into();
    }
    if n <= 8 {
        return format!("{}…", "•".repeat(n.min(6)));
    }
    let prefix: String = chars.iter().take(3).collect();
    let suffix: String = chars[n - 4..].iter().collect();
    format!("{prefix}…{suffix}")
}

fn text_anchors(body: &str, start: usize, end: usize) -> (String, String) {
    let chars: Vec<char> = body.chars().collect();
    let before: String = chars
        .iter()
        .take(start.min(chars.len()))
        .rev()
        .take(40)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let after: String = chars.iter().skip(end.min(chars.len())).take(40).collect();
    (before, after)
}

/// Anchor could not be uniquely resolved in the current document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorError;

/// Public for integration tests — char-indexed anchor resolution.
pub fn resolve_anchor(
    current: &str,
    range_start: usize,
    range_end: usize,
    original_slice: &str,
    before: &str,
    after: &str,
    mode: AiApplyMode,
) -> Result<(usize, usize), AnchorError> {
    let slice_at = |body: &str, a: usize, b: usize| -> String {
        body.chars().skip(a).take(b.saturating_sub(a)).collect()
    };
    if slice_at(current, range_start, range_end) == original_slice {
        let ctx_ok = current
            .chars()
            .take(range_start)
            .collect::<String>()
            .ends_with(before)
            && current
                .chars()
                .skip(range_end)
                .collect::<String>()
                .starts_with(after);
        if ctx_ok || (before.is_empty() && after.is_empty()) {
            return Ok((range_start, range_end));
        }
    }
    let needle = match mode {
        AiApplyMode::InsertAtCursor => before.to_string(),
        _ => format!("{before}{original_slice}{after}"),
    };
    if needle.is_empty() {
        return Err(AnchorError);
    }
    // Search on the char string so indices stay char-based (Unicode-safe).
    let chars: Vec<char> = current.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut matches = Vec::new();
    let mut i = 0usize;
    while i + needle_chars.len() <= chars.len() {
        if chars[i..i + needle_chars.len()] == needle_chars[..] {
            matches.push(i);
            i += needle_chars.len().max(1);
        } else {
            i += 1;
        }
    }
    match matches.len() {
        1 => {
            let start = matches[0] + before.chars().count();
            let end = start + original_slice.chars().count();
            Ok((start, end))
        }
        _ => Err(AnchorError),
    }
}

fn apply_proposal_to_body(
    before: &str,
    range_start: usize,
    range_end: usize,
    mode: AiApplyMode,
    proposal: &str,
) -> String {
    let mut chars: Vec<char> = before.chars().collect();
    let a = range_start.min(chars.len());
    let b = range_end.min(chars.len()).max(a);
    match mode {
        AiApplyMode::ReplaceSelection => {
            chars.splice(a..b, proposal.chars());
        }
        AiApplyMode::InsertAtCursor | AiApplyMode::AppendEnd => {
            let mut piece = String::new();
            if a > 0 && !chars[a - 1].is_whitespace() && !proposal.starts_with(char::is_whitespace)
            {
                piece.push(' ');
            }
            piece.push_str(proposal);
            let at = if matches!(mode, AiApplyMode::AppendEnd) {
                chars.len()
            } else {
                a
            };
            chars.splice(at..at, piece.chars());
        }
    }
    chars.into_iter().collect()
}

fn parse_ideas_json(raw: &str) -> Vec<ContentIdea> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    #[derive(serde::Deserialize)]
    struct Wrap {
        ideas: Vec<ContentIdea>,
    }
    if let Ok(w) = serde_json::from_str::<Wrap>(cleaned) {
        return w.ideas;
    }
    // try extract outer object
    if let (Some(a), Some(b)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if let Ok(w) = serde_json::from_str::<Wrap>(&cleaned[a..=b]) {
            return w.ideas;
        }
    }
    vec![ContentIdea {
        title: "Suite".into(),
        pitch: raw.to_string(),
    }]
}

fn looks_like_prose(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 80 {
        return false;
    }
    // Conversational answers often start with meta phrases.
    let lower = t.to_lowercase();
    if lower.starts_with("bien sûr")
        || lower.starts_with("voici")
        || lower.starts_with("je ")
        || lower.starts_with("oui")
        || lower.contains("?\n")
    {
        return false;
    }
    t.contains('\n') || t.chars().count() > 200
}

fn scene_breadcrumb(project: &Project, scene_id: &str) -> String {
    let Some(scene) = project.manuscript.get_scene(scene_id) else {
        return "Scène".into();
    };
    for act in &project.manuscript.index.acts {
        for ch in &act.chapters {
            if ch.scene_ids.iter().any(|id| id == scene_id) {
                return format!("{} · {} · {}", act.title, ch.title, scene.meta.title);
            }
        }
    }
    scene.meta.title.clone()
}

fn slug_simple(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "livre".into()
    } else {
        s
    }
}
