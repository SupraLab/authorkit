use ak_core::{Note, NoteKind, Project, Scene};
use ak_llm::Brush;
use egui::{RichText, ScrollArea, TextEdit, Ui};
use egui_commonmark::CommonMarkCache;
use shadcn_egui::components::button::{Button, ButtonSize, ButtonVariant};
use shadcn_egui::components::card::{card_content, Card};
use shadcn_egui::components::input::Input;
use shadcn_egui::components::separator::separator;
use shadcn_egui::Theme;

use crate::i18n::Lang;
use crate::theme::{FONT_TITLE, FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    Scene,
    Note,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    Clean,
    Unsaved,
    Saving,
    Saved,
    Failed(String),
}

impl SaveStatus {
    /// Status-bar label: only when the user needs to act or wait — never a sticky "Saved".
    pub fn label_fr(&self) -> String {
        match self {
            Self::Clean | Self::Saved => String::new(),
            Self::Unsaved => "Modifications non enregistrées".into(),
            Self::Saving => "Enregistrement…".into(),
            Self::Failed(msg) => format!("Échec : {msg}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Typing / continuous edits — may coalesce into one undo step.
    Typing,
    /// Formatting, AI, restore — always a discrete undo step.
    Discrete,
}

pub struct EditorState {
    pub kind: EditorKind,
    pub id: String,
    pub title: String,
    pub body: String,
    pub dirty: bool,
    pub show_slash: bool,
    pub slash_filter: String,
    pub slash_index: usize,
    /// Character range of the current selection (if any).
    pub sel_start: usize,
    pub sel_end: usize,
    pub has_selection: bool,
    pub selection_anchor: Option<egui::Pos2>,
    /// Selection to reinject into TextEditState next frame (after toggles).
    pub pending_selection: Option<(usize, usize)>,
    /// Bumps on every discrete / typing mutation (stale LLM checks).
    pub text_version: u64,
    /// `ctx.input.time` when last edited — for autosave debounce.
    pub dirty_at: Option<f64>,
    pub save_status: SaveStatus,
    /// Wall-clock of last successful save (status bar).
    pub last_saved_at: Option<chrono::DateTime<chrono::Local>>,
    /// When Saving started (keep spinner ≥ 300 ms).
    pub saving_since: Option<f64>,
    /// Disk fingerprint when document was loaded / last successfully saved.
    pub disk_mtime_ms: Option<u128>,
    pub disk_hash: Option<u64>,
    /// Keep selection toolbar alive across frames (hover / open menu).
    pub toolbar_keep: bool,
    pub toolbar_menu_open: bool,
    /// Heading level menu open on the selection toolbar.
    pub heading_menu_open: bool,
    /// "+ Link" context note picker.
    pub link_menu_open: bool,
    pub link_filter: String,
    /// Scene synopsis (persisted on flush).
    pub synopsis: String,
    /// Streaming draft while generating a synopsis (does not replace `synopsis` until Done).
    pub synopsis_draft: String,
    /// True while an LLM synopsis generation is in flight.
    pub synopsis_busy: bool,
    /// Synopsis editor modal open.
    pub synopsis_open: bool,
    pub scene_status: ak_core::SceneStatus,
    /// Unified undo/redo for typing, formatting, and AI applies.
    pub undo_stack: Vec<EditorSnapshot>,
    pub redo_stack: Vec<EditorSnapshot>,
    /// Last time an undo snapshot was pushed (typing coalesce).
    pub last_undo_at: Option<f64>,
    /// Stable id salt for the draft TextEdit (focus / selection reinjection).
    pub draft_focus_id: Option<egui::Id>,
    /// Quote ranges from last visual layout (for painted bars).
    pub quote_ranges: Vec<(usize, usize)>,
    /// Body before the current typing burst (for undo grouping).
    pub typing_anchor_body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EditorSnapshot {
    pub body: String,
    pub title: String,
    pub synopsis: String,
    pub scene_status: ak_core::SceneStatus,
    pub sel_start: usize,
    pub sel_end: usize,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            kind: EditorKind::Empty,
            id: String::new(),
            title: String::new(),
            body: String::new(),
            dirty: false,
            show_slash: false,
            slash_filter: String::new(),
            slash_index: 0,
            sel_start: 0,
            sel_end: 0,
            has_selection: false,
            selection_anchor: None,
            pending_selection: None,
            text_version: 0,
            dirty_at: None,
            save_status: SaveStatus::Clean,
            last_saved_at: None,
            saving_since: None,
            disk_mtime_ms: None,
            disk_hash: None,
            toolbar_keep: false,
            toolbar_menu_open: false,
            heading_menu_open: false,
            link_menu_open: false,
            link_filter: String::new(),
            synopsis: String::new(),
            synopsis_draft: String::new(),
            synopsis_busy: false,
            synopsis_open: false,
            scene_status: ak_core::SceneStatus::ToWrite,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_undo_at: None,
            draft_focus_id: None,
            quote_ranges: Vec::new(),
            typing_anchor_body: None,
        }
    }
}

impl EditorState {
    pub fn selected_text(&self) -> Option<&str> {
        if !self.has_selection || self.sel_start == self.sel_end {
            return None;
        }
        let (a, b) = (
            self.sel_start.min(self.sel_end),
            self.sel_start.max(self.sel_end),
        );
        Some(self.body_char_slice(a, b))
    }

    fn body_char_slice(&self, start: usize, end: usize) -> &str {
        let mut it = self.body.char_indices();
        let byte_start = it.nth(start).map(|(i, _)| i).unwrap_or(self.body.len());
        let byte_end = if end <= start {
            byte_start
        } else {
            self.body
                .char_indices()
                .nth(end)
                .map(|(i, _)| i)
                .unwrap_or(self.body.len())
        };
        &self.body[byte_start..byte_end]
    }

    /// Unified mutation marker for typing, title, formatting, and AI inserts.
    pub fn mark_dirty(&mut self, now: f64) {
        self.dirty = true;
        self.dirty_at = Some(now);
        self.save_status = SaveStatus::Unsaved;
    }

    fn take_snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            body: self.body.clone(),
            title: self.title.clone(),
            synopsis: self.synopsis.clone(),
            scene_status: self.scene_status,
            sel_start: self.sel_start,
            sel_end: self.sel_end,
        }
    }

    fn apply_snapshot(&mut self, snap: EditorSnapshot) {
        self.body = snap.body;
        self.title = snap.title;
        self.synopsis = snap.synopsis;
        self.scene_status = snap.scene_status;
        self.sel_start = snap.sel_start;
        self.sel_end = snap.sel_end;
        self.has_selection = self.sel_start != self.sel_end;
        self.pending_selection = Some((self.sel_start, self.sel_end));
    }

    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.take_snapshot());
        if self.undo_stack.len() > 80 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Single mutation entry — handles undo grouping, dirty clock, text_version.
    pub fn edit<F>(&mut self, kind: EditKind, now: f64, f: F)
    where
        F: FnOnce(&mut Self),
    {
        let should_push = match kind {
            EditKind::Discrete => true,
            EditKind::Typing => {
                let gap = self.last_undo_at.map(|t| now - t).unwrap_or(f64::INFINITY);
                gap > 1.0 || self.undo_stack.is_empty()
            }
        };
        if should_push {
            self.push_undo();
            self.last_undo_at = Some(now);
        }
        f(self);
        self.text_version = self.text_version.saturating_add(1);
        self.mark_dirty(now);
        if self.sel_start != self.sel_end {
            self.pending_selection = Some((self.sel_start, self.sel_end));
            self.has_selection = true;
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.take_snapshot());
        self.apply_snapshot(prev);
        self.text_version = self.text_version.saturating_add(1);
        self.mark_dirty(self.dirty_at.unwrap_or(0.0).max(0.001));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.take_snapshot());
        self.apply_snapshot(next);
        self.text_version = self.text_version.saturating_add(1);
        self.mark_dirty(self.dirty_at.unwrap_or(0.0).max(0.001));
        true
    }

    /// Apply an AI (or bulk) edit as a single undo step.
    pub fn apply_body_change(&mut self, new_body: String, now: f64) {
        self.edit(EditKind::Discrete, now, |s| {
            s.body = new_body;
        });
    }

    pub fn replace_selection(&mut self, replacement: &str) {
        self.replace_selection_at(replacement, None);
    }

    /// Replace selection; `now` when provided sets dirty_at correctly.
    pub fn replace_selection_at(&mut self, replacement: &str, now: Option<f64>) {
        if !self.has_selection {
            return;
        }
        let now = now.unwrap_or(0.001);
        let replacement = replacement.to_string();
        self.edit(EditKind::Discrete, now, |s| {
            let (a, b) = (s.sel_start.min(s.sel_end), s.sel_start.max(s.sel_end));
            let mut chars: Vec<char> = s.body.chars().collect();
            if a > chars.len() || b > chars.len() || a > b {
                return;
            }
            chars.splice(a..b, replacement.chars());
            s.body = chars.into_iter().collect();
            s.sel_end = a + replacement.chars().count();
            s.sel_start = s.sel_end;
            s.has_selection = false;
        });
    }

    pub fn wrap_selection(&mut self, prefix: &str, suffix: &str) {
        self.wrap_selection_at(prefix, suffix, 0.001);
    }

    pub fn wrap_selection_at(&mut self, prefix: &str, suffix: &str, now: f64) {
        if let Some(sel) = self.selected_text().map(|s| s.to_string()) {
            let a0 = self.sel_start.min(self.sel_end);
            let wrapped = format!("{prefix}{sel}{suffix}");
            self.replace_selection_at(&wrapped, Some(now));
            let inner_len = sel.chars().count();
            self.sel_start = a0 + prefix.chars().count();
            self.sel_end = self.sel_start + inner_len;
            self.has_selection = true;
            self.pending_selection = Some((self.sel_start, self.sel_end));
        } else {
            self.edit(EditKind::Discrete, now, |s| {
                widgets::format_wrap(&mut s.body, prefix, suffix);
            });
        }
    }

    /// True when selection is already wrapped (markers inside or just outside).
    pub fn selection_has_wrap(&self, prefix: &str, suffix: &str) -> bool {
        let Some(sel) = self.selected_text() else {
            return false;
        };
        let plen = prefix.chars().count();
        let slen = suffix.chars().count();
        if sel.chars().count() >= plen + slen
            && sel.starts_with(prefix)
            && sel.ends_with(suffix)
            && !(prefix == "*" && sel.starts_with("**"))
        {
            return true;
        }
        let (a, b) = (
            self.sel_start.min(self.sel_end),
            self.sel_start.max(self.sel_end),
        );
        if a < plen {
            return false;
        }
        let before = self.body_char_slice(a - plen, a);
        let after = self.body_char_slice(b, b + slen);
        if before != prefix || after != suffix {
            return false;
        }
        // Don't treat `**` as italic `*`.
        if prefix == "*" {
            let wider = if a >= 2 {
                self.body_char_slice(a - 2, a)
            } else {
                before
            };
            if wider == "**" {
                return false;
            }
            let wider_after = self.body_char_slice(b, b + 2);
            if wider_after.starts_with("**") {
                return false;
            }
        }
        true
    }

    pub fn is_bold(&self) -> bool {
        self.selection_has_wrap("**", "**") || self.selection_has_wrap("__", "__")
    }

    pub fn is_italic(&self) -> bool {
        if self.is_bold() {
            return false;
        }
        self.selection_has_wrap("*", "*") || self.selection_has_wrap("_", "_")
    }

    pub fn is_heading(&self) -> bool {
        self.heading_level().is_some()
    }

    /// Current heading level (1–6) of the selection / line, if any.
    pub fn heading_level(&self) -> Option<u8> {
        let line = self.selected_or_line_head();
        let bytes = line.as_bytes();
        let mut n = 0usize;
        while n < bytes.len() && bytes[n] == b'#' {
            n += 1;
        }
        if (1..=6).contains(&n) && bytes.get(n) == Some(&b' ') {
            Some(n as u8)
        } else {
            None
        }
    }

    pub fn is_quote(&self) -> bool {
        self.selected_or_line_head().starts_with("> ")
    }

    fn selected_or_line_head(&self) -> String {
        if let Some(sel) = self.selected_text() {
            return sel.lines().next().unwrap_or(sel).to_string();
        }
        let a = self.sel_start.min(self.sel_end);
        let line_start = self.char_line_start(a);
        self.body_char_slice(line_start, (line_start + 8).min(self.body.chars().count()))
            .to_string()
    }

    fn char_line_start(&self, char_idx: usize) -> usize {
        let mut last_nl = 0usize;
        for (count, c) in self.body.chars().enumerate() {
            if count >= char_idx {
                break;
            }
            if c == '\n' {
                last_nl = count + 1;
            }
        }
        last_nl
    }

    pub fn toggle_wrap(&mut self, prefix: &str, suffix: &str) {
        self.toggle_wrap_at(prefix, suffix, 0.001);
    }

    pub fn toggle_wrap_at(&mut self, prefix: &str, suffix: &str, now: f64) {
        let Some(sel) = self.selected_text().map(|s| s.to_string()) else {
            return;
        };
        let plen = prefix.chars().count();
        let slen = suffix.chars().count();
        let (a, b) = (
            self.sel_start.min(self.sel_end),
            self.sel_start.max(self.sel_end),
        );

        if sel.starts_with(prefix)
            && sel.ends_with(suffix)
            && sel.chars().count() >= plen + slen
            && !(prefix == "*" && sel.starts_with("**"))
        {
            let inner: String = sel
                .chars()
                .skip(plen)
                .take(sel.chars().count() - plen - slen)
                .collect();
            self.replace_selection_at(&inner, Some(now));
            self.has_selection = true;
            self.sel_start = a;
            self.sel_end = a + inner.chars().count();
            self.pending_selection = Some((self.sel_start, self.sel_end));
            return;
        }

        if a >= plen {
            let before = self.body_char_slice(a - plen, a).to_string();
            let after = self.body_char_slice(b, b + slen).to_string();
            let ok = before == prefix
                && after == suffix
                && !(prefix == "*" && a >= 2 && self.body_char_slice(a - 2, a) == "**");
            if ok {
                self.sel_start = a - plen;
                self.sel_end = b + slen;
                self.has_selection = true;
                self.replace_selection_at(&sel, Some(now));
                self.has_selection = true;
                self.sel_start = a - plen;
                self.sel_end = a - plen + sel.chars().count();
                self.pending_selection = Some((self.sel_start, self.sel_end));
                return;
            }
        }

        self.wrap_selection_at(prefix, suffix, now);
    }

    pub fn toggle_line_prefix(&mut self, prefix: &str) {
        self.toggle_line_prefix_at(prefix, 0.001);
    }

    pub fn toggle_line_prefix_at(&mut self, prefix: &str, now: f64) {
        let Some(sel) = self.selected_text().map(|s| s.to_string()) else {
            // Expand to whole line when no selection.
            let a = self.sel_start.min(self.sel_end);
            let start = self.char_line_start(a);
            let end = {
                let total = self.body.chars().count();
                let mut i = a;
                let chars: Vec<char> = self.body.chars().collect();
                while i < total && chars[i] != '\n' {
                    i += 1;
                }
                i
            };
            if start == end {
                return;
            }
            self.sel_start = start;
            self.sel_end = end;
            self.has_selection = true;
            self.toggle_line_prefix_at(prefix, now);
            return;
        };
        let a0 = self.sel_start.min(self.sel_end);
        if sel.starts_with(prefix) {
            let stripped = sel.strip_prefix(prefix).unwrap_or(&sel).to_string();
            self.replace_selection_at(&stripped, Some(now));
            self.sel_start = a0;
            self.sel_end = a0 + stripped.chars().count();
            self.has_selection = true;
            self.pending_selection = Some((self.sel_start, self.sel_end));
            return;
        }
        let stripped = strip_heading_prefix(&sel);
        if prefix.starts_with('#')
            && self.heading_level().is_some()
            && prefix == format!("{} ", "#".repeat(self.heading_level().unwrap() as usize))
        {
            self.replace_selection_at(stripped, Some(now));
            self.sel_start = a0;
            self.sel_end = a0 + stripped.chars().count();
            self.has_selection = true;
            self.pending_selection = Some((self.sel_start, self.sel_end));
            return;
        }
        let next = format!("{prefix}{stripped}");
        self.replace_selection_at(&next, Some(now));
        self.sel_start = a0;
        self.sel_end = a0 + next.chars().count();
        self.has_selection = true;
        self.pending_selection = Some((self.sel_start, self.sel_end));
    }

    /// Set heading level (1–6) or `None` to strip to paragraph.
    pub fn set_heading(&mut self, level: Option<u8>, now: f64) {
        if !self.has_selection || self.sel_start == self.sel_end {
            let a = self.sel_start.min(self.sel_end);
            let start = self.char_line_start(a);
            let chars: Vec<char> = self.body.chars().collect();
            let mut end = a.min(chars.len());
            while end < chars.len() && chars[end] != '\n' {
                end += 1;
            }
            if start >= end {
                return;
            }
            self.sel_start = start;
            self.sel_end = end;
            self.has_selection = true;
        }
        let a0 = self.sel_start.min(self.sel_end);
        let current = self.heading_level();
        let Some(sel) = self.selected_text().map(|s| s.to_string()) else {
            return;
        };
        let stripped = strip_heading_prefix(&sel).to_string();
        let next = match level {
            Some(l) if (1..=6).contains(&l) => {
                if current == Some(l) {
                    stripped
                } else {
                    format!("{} {stripped}", "#".repeat(l as usize))
                }
            }
            _ => stripped,
        };
        self.replace_selection_at(&next, Some(now));
        self.sel_start = a0;
        self.sel_end = a0 + next.chars().count();
        self.has_selection = true;
        self.pending_selection = Some((self.sel_start, self.sel_end));
    }
}

fn strip_heading_prefix(sel: &str) -> &str {
    let bytes = sel.as_bytes();
    let mut n = 0usize;
    while n < bytes.len() && bytes[n] == b'#' {
        n += 1;
    }
    if (1..=6).contains(&n) && bytes.get(n) == Some(&b' ') {
        &sel[n + 1..]
    } else {
        sel
    }
}

pub enum EditorAction {
    Save,
    SlashContinue,
    SlashSentence,
    SlashParagraph,
    SlashGenerate { prompt: String },
    Brush(String),
    AddToChat,
    LinkNote(String),
    UnlinkNote(String),
    OpenNote(String),
    ExportMarkdown,
    ExportEpub,
    ExportDocx,
    SaveStyles,
    Bootstrap,
    GenerateSynopsis,
}

pub struct EditorOutcome {
    pub actions: Vec<EditorAction>,
}

pub fn show_scene(
    ui: &mut Ui,
    project: &Project,
    scene: &Scene,
    state: &mut EditorState,
    brushes: &[Brush],
    code_mode: &mut bool,
    md_cache: &mut CommonMarkCache,
    lang: Lang,
) -> EditorOutcome {
    let mut out = EditorOutcome {
        actions: Vec::new(),
    };
    if state.id != scene.meta.id || state.kind != EditorKind::Scene {
        let disk_mtime_ms = ak_core::fsutil::mtime_millis(&scene.path);
        let disk_hash = Some(ak_core::fsutil::content_hash(scene.body.as_bytes()));
        *state = EditorState {
            kind: EditorKind::Scene,
            id: scene.meta.id.clone(),
            title: scene.meta.title.clone(),
            body: scene.body.clone(),
            synopsis: scene.meta.synopsis.clone(),
            scene_status: scene.meta.status,
            disk_mtime_ms,
            disk_hash,
            ..EditorState::default()
        };
    }

    header(ui, state, code_mode, lang, &mut out);
    context_chips(ui, project, scene, state, &mut out, lang);
    ui.add_space(4.0);
    draft_area(ui, state, *code_mode, md_cache, lang);
    selection_toolbar(ui.ctx(), state, brushes, &mut out, lang);
    out
}

fn context_chips(
    ui: &mut Ui,
    project: &Project,
    scene: &Scene,
    state: &mut EditorState,
    out: &mut EditorOutcome,
    lang: Lang,
) {
    let pal = Theme::current(ui.ctx()).palette;
    let ctx = ui.ctx().clone();
    let mut btn_rect = egui::Rect::NOTHING;

    // Sm = 24px — chips, ✕ and Relier share that height.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            widgets::eyebrow(ui, lang.context());
            for link in &scene.meta.links {
                if let Some(note) = project.notes.get(link) {
                    let dark = Theme::current(ui.ctx()).dark;
                    let fg = crate::theme::chip_fg(&pal, dark);
                    let hit = link_chip(
                        ui,
                        &note.meta.name,
                        widgets::note_kind_icon(note.meta.kind),
                        fg,
                        crate::theme::chip_fill(&pal, dark),
                        pal.border,
                    );
                    if hit.open {
                        out.actions
                            .push(EditorAction::OpenNote(note.meta.id.clone()));
                    }
                    if hit.remove {
                        out.actions
                            .push(EditorAction::UnlinkNote(note.meta.id.clone()));
                    }
                }
            }
            let btn = ui
                .add(
                    Button::new(lang.link())
                        .variant(if state.link_menu_open {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Outline
                        })
                        .size(ButtonSize::Sm)
                        .icon(&lucide::LINK),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(lang.link_tip());
            btn_rect = btn.rect;
            if btn.clicked() {
                state.link_menu_open = !state.link_menu_open;
                if state.link_menu_open {
                    state.link_filter.clear();
                }
            }
        });
    });

    if !state.link_menu_open {
        return;
    }

    let popup_pos = egui::pos2(btn_rect.left(), btn_rect.bottom() + 4.0);
    let popup = egui::Area::new(egui::Id::new("ak_link_menu"))
        .order(egui::Order::Foreground)
        .fixed_pos(popup_pos)
        .pivot(egui::Align2::LEFT_TOP)
        .interactable(true)
        .show(&ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(pal.popover)
                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(10, 10))
                .shadow(egui::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(28),
                })
                .show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.label(
                        RichText::new(lang.link_title())
                            .size(FONT_UI)
                            .strong()
                            .color(pal.foreground),
                    );
                    ui.add_space(6.0);
                    let _ = ui.add(
                        Input::new(&mut state.link_filter)
                            .hint(lang.filter_hint())
                            .max_width(f32::INFINITY),
                    );
                    ui.add_space(8.0);

                    let filter = state.link_filter.to_lowercase();
                    let mut any = false;
                    ScrollArea::vertical()
                        .id_salt("ak_link_scroll")
                        .max_height(320.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_width(280.0);
                            for kind in NoteKind::all() {
                                let notes: Vec<_> = project
                                    .notes
                                    .by_kind(kind)
                                    .filter(|n| !scene.meta.links.contains(&n.meta.id))
                                    .filter(|n| {
                                        filter.is_empty()
                                            || n.meta.name.to_lowercase().contains(&filter)
                                    })
                                    .collect();
                                if notes.is_empty() {
                                    continue;
                                }
                                any = true;
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 5.0;
                                    widgets::icon_colored(
                                        ui,
                                        widgets::note_kind_icon(kind),
                                        12.0,
                                        pal.muted_foreground,
                                    );
                                    ui.label(
                                        RichText::new(lang.note_kind_plural(kind))
                                            .size(FONT_UI_SM)
                                            .strong()
                                            .color(pal.muted_foreground),
                                    );
                                });
                                ui.add_space(2.0);
                                for note in notes {
                                    if ui
                                        .add(
                                            Button::new(note.meta.name.as_str())
                                                .variant(ButtonVariant::Ghost)
                                                .size(ButtonSize::Sm)
                                                .icon(widgets::note_kind_icon(kind)),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        out.actions
                                            .push(EditorAction::LinkNote(note.meta.id.clone()));
                                        state.link_menu_open = false;
                                        state.link_filter.clear();
                                    }
                                }
                                ui.add_space(8.0);
                            }
                            if !any {
                                ui.label(
                                    RichText::new(if filter.is_empty() {
                                        lang.no_notes_link()
                                    } else {
                                        lang.no_results()
                                    })
                                    .size(FONT_UI_SM)
                                    .color(pal.muted_foreground),
                                );
                            }
                        });
                });
        });

    let over_btn = btn_rect.contains(ctx.pointer_latest_pos().unwrap_or(egui::Pos2::ZERO));
    let over_popup = popup.response.contains_pointer() || popup.response.hovered();
    if ctx.input(|i| i.pointer.any_click()) && !over_btn && !over_popup {
        state.link_menu_open = false;
        state.link_filter.clear();
    }
}

fn draft_area(
    ui: &mut Ui,
    state: &mut EditorState,
    code_mode: bool,
    md_cache: &mut CommonMarkCache,
    lang: Lang,
) {
    let _ = md_cache;
    let now = ui.ctx().input(|i| i.time);
    let keep_sel = state.toolbar_keep
        || state.toolbar_menu_open
        || state.heading_menu_open
        || state.show_slash;
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width((ui.available_width() - 40.0).clamp(380.0, 720.0));

            let hint = if code_mode {
                lang.md_hint()
            } else {
                lang.write_hint()
            };
            let rows = if code_mode { 32 } else { 28 };

            let mut quote_ranges: Vec<(usize, usize)> = Vec::new();
            let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                if code_mode {
                    let job = egui::text::LayoutJob::simple(
                        buf.as_str().into(),
                        egui::FontId::monospace(FONT_UI),
                        Theme::current(ui.ctx()).palette.foreground,
                        wrap_width,
                    );
                    ui.fonts_mut(|f| f.layout_job(job))
                } else {
                    let (job, extras) = crate::md_layout::highlight_markdown_ex(
                        buf.as_str(),
                        ui,
                        wrap_width,
                        FONT_UI + 4.0,
                    );
                    quote_ranges = extras.quote_ranges;
                    ui.fonts_mut(|f| f.layout_job(job))
                }
            };

            // id includes mode so switching Visuel/Code busts the cached galley
            // (otherwise the switch looks broken — UI flips, text stays raw).
            let te_id = ui.make_persistent_id(("ak_draft", state.id.as_str(), code_mode));
            state.draft_focus_id = Some(te_id);

            // Reinject selection after style toggles so is_bold / re-click work.
            let pending = state.pending_selection.take();
            if let Some((a, b)) = pending {
                use egui::text::{CCursor, CCursorRange};
                use egui::text_edit::TextEditState;
                let mut te_state = TextEditState::load(ui.ctx(), te_id).unwrap_or_default();
                te_state
                    .cursor
                    .set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
                te_state.store(ui.ctx(), te_id);
                ui.memory_mut(|m| m.request_focus(te_id));
                state.sel_start = a;
                state.sel_end = b;
                state.has_selection = a != b;
            }

            // Swallow native TextEdit undo while draft is focused — we own the stack.
            if ui.memory(|m| m.has_focus(te_id)) {
                ui.input_mut(|i| {
                    if (i.modifiers.command || i.modifiers.ctrl)
                        && (i.key_pressed(egui::Key::Z) || i.key_pressed(egui::Key::Y))
                    {
                        i.consume_key(
                            if i.modifiers.shift {
                                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
                            } else {
                                egui::Modifiers::COMMAND
                            },
                            if i.key_pressed(egui::Key::Y) {
                                egui::Key::Y
                            } else {
                                egui::Key::Z
                            },
                        );
                    }
                });
            }

            let output = TextEdit::multiline(&mut state.body)
                .id(te_id)
                .desired_width(f32::INFINITY)
                .desired_rows(rows)
                .frame(egui::Frame::NONE)
                .hint_text(hint)
                .layouter(&mut layouter)
                .show(ui);

            state.quote_ranges = quote_ranges;

            // Paint quote gutter: soft background + left border (under the text layer).
            if !code_mode && !state.quote_ranges.is_empty() {
                let pal = Theme::current(ui.ctx()).palette;
                let painter = ui.painter();
                let gutter = crate::md_layout::QUOTE_GUTTER;
                for (a, b) in crate::md_layout::merge_quote_blocks(&state.quote_ranges) {
                    let r0 = output.galley.pos_from_cursor(egui::text::CCursor::new(a));
                    let end_idx = b.saturating_sub(1).max(a);
                    let r1 = output
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(end_idx));
                    let top = output.galley_pos.y + r0.min.y;
                    let bottom = (output.galley_pos.y + r1.max.y).max(top + r0.height());
                    let left = output.galley_pos.x;
                    let width = output.galley.rect.width().max(gutter + 8.0);
                    let bg = egui::Rect::from_min_max(
                        egui::pos2(left, top),
                        egui::pos2(left + width, bottom),
                    );
                    let wash = egui::Color32::from_rgba_unmultiplied(
                        pal.muted.r(),
                        pal.muted.g(),
                        pal.muted.b(),
                        48,
                    );
                    painter.rect_filled(bg, 3.0, wash);
                    // Vertical border in the leading gutter.
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(left + 3.0, top + 2.0),
                        egui::pos2(left + 6.0, bottom - 2.0),
                    );
                    painter.rect_filled(bar, 1.5, pal.primary);
                }
            }

            // Keep selection highlight visible when focus is stolen by toolbar / AI menu.
            let paint_sel = keep_sel
                && state.has_selection
                && state.sel_start != state.sel_end
                && !output.response.has_focus();
            if paint_sel {
                paint_selection_overlay(
                    ui,
                    &output.galley,
                    output.galley_pos,
                    state.sel_start,
                    state.sel_end,
                );
            }

            if output.response.changed() {
                let gap = state.last_undo_at.map(|t| now - t).unwrap_or(f64::INFINITY);
                if gap > 1.0 {
                    if let Some(prev) = state.typing_anchor_body.take() {
                        state.undo_stack.push(EditorSnapshot {
                            body: prev,
                            title: state.title.clone(),
                            synopsis: state.synopsis.clone(),
                            scene_status: state.scene_status,
                            sel_start: state.sel_start,
                            sel_end: state.sel_end,
                        });
                        if state.undo_stack.len() > 80 {
                            state.undo_stack.remove(0);
                        }
                        state.redo_stack.clear();
                    }
                    state.last_undo_at = Some(now);
                }
                state.typing_anchor_body = Some(state.body.clone());
                state.text_version = state.text_version.saturating_add(1);
                state.mark_dirty(now);
                if let Some(range) = output.cursor_range {
                    try_consume_slash_command(state, range.primary.index);
                }
            } else if state.typing_anchor_body.is_none() {
                state.typing_anchor_body = Some(state.body.clone());
            }

            if let Some(range) = output.cursor_range {
                let [min, max] = range.sorted_cursors();
                state.sel_start = min.index;
                state.sel_end = max.index;
                state.has_selection = min.index != max.index;
                if state.has_selection {
                    let rect = output.galley.pos_from_cursor(max);
                    state.selection_anchor =
                        Some(output.galley_pos + egui::vec2(rect.center().x, rect.bottom()));
                } else if !keep_sel {
                    state.selection_anchor = None;
                }
            } else if !output.response.has_focus() && !keep_sel {
                state.has_selection = false;
                state.selection_anchor = None;
                state.toolbar_menu_open = false;
                state.heading_menu_open = false;
            }
        });
}

fn paint_selection_overlay(
    ui: &mut Ui,
    galley: &std::sync::Arc<egui::Galley>,
    galley_pos: egui::Pos2,
    sel_start: usize,
    sel_end: usize,
) {
    let (a, b) = (sel_start.min(sel_end), sel_start.max(sel_end));
    if a == b {
        return;
    }
    let fill = ui.visuals().selection.bg_fill;
    let range =
        egui::text::CCursorRange::two(egui::text::CCursor::new(a), egui::text::CCursor::new(b));
    let [min, max] = range.sorted_cursors();
    let r_min = galley.pos_from_cursor(min);
    let r_max = galley.pos_from_cursor(max);
    // Single-line fast path; multi-line paints a union rect (good enough to keep selection visible).
    if (r_min.min.y - r_max.min.y).abs() < 1.0 {
        let rect = egui::Rect::from_min_max(
            galley_pos + egui::vec2(r_min.min.x, r_min.min.y),
            galley_pos + egui::vec2(r_max.max.x, r_min.max.y),
        );
        ui.painter().rect_filled(rect, 0.0, fill);
        return;
    }
    let mut y = r_min.min.y;
    let bottom = r_max.max.y;
    let row_h = (r_min.height()).max(1.0);
    while y < bottom + 0.5 {
        let left = if (y - r_min.min.y).abs() < 0.5 {
            r_min.min.x
        } else {
            0.0
        };
        let right = if y + row_h >= bottom - 0.5 {
            r_max.max.x
        } else {
            galley.rect.width()
        };
        let rect = egui::Rect::from_min_max(
            galley_pos + egui::vec2(left, y),
            galley_pos + egui::vec2(right, y + row_h),
        );
        ui.painter().rect_filled(rect, 0.0, fill);
        y += row_h;
    }
}

/// If the char before `cursor` is a slash at a word boundary, remove it and open the palette.
fn try_consume_slash_command(state: &mut EditorState, cursor: usize) {
    if cursor == 0 || state.show_slash {
        return;
    }
    let mut chars: Vec<char> = state.body.chars().collect();
    if cursor > chars.len() {
        return;
    }
    if chars[cursor - 1] != '/' {
        return;
    }
    let prev = cursor.checked_sub(2).and_then(|i| chars.get(i).copied());
    let at_boundary = prev.map(|c| c.is_whitespace()).unwrap_or(true);
    if !at_boundary {
        return;
    }
    chars.remove(cursor - 1);
    state.body = chars.into_iter().collect();
    state.sel_start = cursor - 1;
    state.sel_end = cursor - 1;
    state.has_selection = false;
    state.show_slash = true;
    state.slash_filter.clear();
    state.slash_index = 0;
}

fn selection_toolbar(
    ctx: &egui::Context,
    state: &mut EditorState,
    brushes: &[Brush],
    out: &mut EditorOutcome,
    lang: Lang,
) {
    if !state.has_selection && !state.toolbar_menu_open && !state.heading_menu_open {
        state.toolbar_keep = false;
        return;
    }
    let Some(pos) = state.selection_anchor else {
        state.toolbar_keep = false;
        return;
    };
    let pal = Theme::current(ctx).palette;
    let mut menu_btn_rect = egui::Rect::NOTHING;
    let mut heading_btn_rect = egui::Rect::NOTHING;
    let now = ctx.input(|i| i.time);

    let area = egui::Area::new(egui::Id::new("ak_sel_toolbar"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos + egui::vec2(0.0, 6.0))
        .pivot(egui::Align2::CENTER_TOP)
        .interactable(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(pal.popover)
                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(4, 3))
                .shadow(egui::Shadow {
                    offset: [0, 1],
                    blur: 6,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(18),
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.horizontal(|ui| {
                        ui.set_min_height(28.0);
                        let bold = state.is_bold();
                        let italic = state.is_italic();
                        let heading = state.is_heading();
                        let quote = state.is_quote();
                        if widgets::icon_toggle(ui, &lucide::BOLD, lang.bold(), bold).clicked() {
                            state.toggle_wrap_at("**", "**", now);
                        }
                        if widgets::icon_toggle(ui, &lucide::ITALIC, lang.italic(), italic)
                            .clicked()
                        {
                            state.toggle_wrap_at("*", "*", now);
                        }
                        let heading_btn = widgets::icon_toggle(
                            ui,
                            &lucide::HEADING_1,
                            lang.heading(),
                            heading || state.heading_menu_open,
                        );
                        heading_btn_rect = heading_btn.rect;
                        if heading_btn.clicked() {
                            state.heading_menu_open = !state.heading_menu_open;
                            state.toolbar_menu_open = false;
                        }
                        if widgets::icon_toggle(ui, &lucide::QUOTE, lang.quote(), quote).clicked() {
                            state.toggle_line_prefix_at("> ", now);
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        if ui
                            .add(
                                Button::new("Au chat")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .icon(&lucide::MESSAGE_SQUARE_PLUS),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Ajouter la sélection au chat")
                            .clicked()
                        {
                            out.actions.push(EditorAction::AddToChat);
                        }
                        for b in brushes.iter().take(4) {
                            if ui
                                .add(
                                    Button::new(b.label.as_str())
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(&b.label)
                                .clicked()
                            {
                                out.actions.push(EditorAction::Brush(b.id.clone()));
                            }
                        }
                        let more = widgets::icon_toggle(
                            ui,
                            &lucide::ELLIPSIS,
                            lang.more_brushes(),
                            state.toolbar_menu_open,
                        );
                        menu_btn_rect = more.rect;
                        if more.clicked() {
                            state.toolbar_menu_open = !state.toolbar_menu_open;
                            state.heading_menu_open = false;
                        }
                    });
                });
        });

    let toolbar_hot = area.response.contains_pointer() || area.response.hovered();
    let mut menu_hot = false;
    let mut heading_hot = false;

    if state.heading_menu_open {
        let menu_pos = egui::pos2(heading_btn_rect.center().x, heading_btn_rect.bottom() + 4.0);
        let current = state.heading_level();
        let menu = egui::Area::new(egui::Id::new("ak_sel_heading"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu_pos)
            .pivot(egui::Align2::CENTER_TOP)
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(pal.popover)
                    .stroke(egui::Stroke::new(1.0_f32, pal.border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(6, 6))
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(24),
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(140.0);
                        let para_active = current.is_none();
                        if ui
                            .add(
                                Button::new("Paragraphe")
                                    .variant(if para_active {
                                        ButtonVariant::Secondary
                                    } else {
                                        ButtonVariant::Ghost
                                    })
                                    .size(ButtonSize::Sm),
                            )
                            .clicked()
                        {
                            state.set_heading(None, now);
                            state.heading_menu_open = false;
                        }
                        for level in 1u8..=6 {
                            let label = format!("Titre {level}");
                            let active = current == Some(level);
                            if ui
                                .add(
                                    Button::new(label)
                                        .variant(if active {
                                            ButtonVariant::Secondary
                                        } else {
                                            ButtonVariant::Ghost
                                        })
                                        .size(ButtonSize::Sm),
                                )
                                .clicked()
                            {
                                state.set_heading(Some(level), now);
                                state.heading_menu_open = false;
                            }
                        }
                    });
            });
        heading_hot = menu.response.contains_pointer() || menu.response.hovered();
        if ctx.input(|i| i.pointer.any_click()) && !toolbar_hot && !heading_hot {
            state.heading_menu_open = false;
        }
    }

    if state.toolbar_menu_open {
        let menu_pos = egui::pos2(menu_btn_rect.center().x, menu_btn_rect.bottom() + 4.0);
        let menu = egui::Area::new(egui::Id::new("ak_sel_more"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu_pos)
            .pivot(egui::Align2::CENTER_TOP)
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(pal.popover)
                    .stroke(egui::Stroke::new(1.0_f32, pal.border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(6, 6))
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(24),
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(160.0);
                        ui.label(
                            RichText::new(lang.brushes())
                                .size(FONT_UI_SM)
                                .color(pal.muted_foreground),
                        );
                        ui.add_space(4.0);
                        for b in brushes {
                            if ui
                                .add(
                                    Button::new(b.label.as_str())
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Sm),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                out.actions.push(EditorAction::Brush(b.id.clone()));
                                state.toolbar_menu_open = false;
                            }
                        }
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        if ui
                            .add(
                                Button::new(lang.ai_commands())
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Sm)
                                    .icon(&lucide::SPARKLES),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            state.show_slash = true;
                            state.toolbar_menu_open = false;
                        }
                    });
            });
        menu_hot = menu.response.contains_pointer() || menu.response.hovered();

        if ctx.input(|i| i.pointer.any_click()) && !toolbar_hot && !menu_hot {
            state.toolbar_menu_open = false;
        }
    }

    state.toolbar_keep = toolbar_hot
        || menu_hot
        || heading_hot
        || state.toolbar_menu_open
        || state.heading_menu_open;
}

pub fn show_slash_palette(ctx: &egui::Context, state: &mut EditorState) -> Option<EditorAction> {
    let commands = [
        (&lucide::SPARKLES, "Continuer d'écrire", "continue"),
        (&lucide::PEN_LINE, "Écrire une phrase", "sentence"),
        (&lucide::FILE_TEXT, "Écrire un paragraphe", "paragraph"),
        (&lucide::WAND_SPARKLES, "Générer…", "generate"),
    ];

    let mut chosen: Option<&str> = None;
    let mut open = state.show_slash;
    let mut close = false;
    let dark = Theme::current(ctx).dark;
    crate::widgets::FadeDialog::new("ak_slash", "Commandes IA")
        .description("Aide d'écriture au curseur")
        .width(420.0)
        .backdrop(crate::theme::modal_backdrop(dark))
        .show(ctx, &mut open, |ui| {
            let _ = ui.add(
                Input::new(&mut state.slash_filter)
                    .hint("Filtrer ou décrire la génération…")
                    .max_width(f32::INFINITY),
            );
            ui.add_space(10.0);
            let filter = state.slash_filter.to_lowercase();
            for (i, (icon, label, id)) in commands.iter().enumerate() {
                if !filter.is_empty()
                    && *id != "generate"
                    && !label.to_lowercase().contains(&filter)
                {
                    continue;
                }
                let variant = if state.slash_index == i {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                };
                if ui
                    .add(
                        Button::new(*label)
                            .variant(variant)
                            .icon(icon)
                            .size(ButtonSize::Lg),
                    )
                    .clicked()
                {
                    chosen = Some(*id);
                    state.slash_index = i;
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Échap · Entrée").size(11.0).weak());
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Some((_, _, id)) = commands.get(state.slash_index) {
                    chosen = Some(*id);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                state.slash_index = (state.slash_index + 1).min(commands.len() - 1);
            }
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                state.slash_index = state.slash_index.saturating_sub(1);
            }
        });
    if close || chosen.is_some() {
        open = false;
    }
    state.show_slash = open;

    match chosen {
        Some("continue") => Some(EditorAction::SlashContinue),
        Some("sentence") => Some(EditorAction::SlashSentence),
        Some("paragraph") => Some(EditorAction::SlashParagraph),
        Some("generate") => {
            let prompt = std::mem::take(&mut state.slash_filter);
            Some(EditorAction::SlashGenerate { prompt })
        }
        _ => None,
    }
}

/// Synopsis editor modal — open from the status row button.
pub fn show_synopsis_modal(ctx: &egui::Context, state: &mut EditorState) -> Option<EditorAction> {
    if state.kind != EditorKind::Scene && !state.synopsis_open {
        return None;
    }
    // Keep drawing through the exit fade even after scene switch edge cases.
    if !state.synopsis_open
        && ctx.animate_bool_responsive(egui::Id::new("ak_synopsis").with("ak_fade"), false) <= 0.001
    {
        return None;
    }

    let mut open = state.synopsis_open;
    let mut generate = false;
    let mut close = false;
    let dark = Theme::current(ctx).dark;
    let busy = state.synopsis_busy;
    let body_empty = state.body.trim().is_empty();

    widgets::FadeDialog::new("ak_synopsis", "Synopsis")
        .description("Résumé court de la scène — utilisé pour la navigation et le contexte IA.")
        .width(480.0)
        .min_height(240.0)
        .backdrop(crate::theme::modal_backdrop(dark))
        .show(ctx, &mut open, |ui| {
            if busy && !state.synopsis_draft.is_empty() {
                ui.label(RichText::new(&state.synopsis_draft).size(FONT_UI_SM).weak());
                ui.add_space(6.0);
            }
            let syn = ui.add(
                TextEdit::multiline(&mut state.synopsis)
                    .desired_width(ui.available_width())
                    .desired_rows(6)
                    .hint_text("Résumé court de la scène…")
                    .font(egui::FontId::proportional(FONT_UI)),
            );
            if syn.changed() {
                state.mark_dirty(ui.ctx().input(|i| i.time));
            }
            ui.add_space(12.0);
            widgets::modal_footer(ui, |ui| {
                let tip = if busy {
                    "Génération…"
                } else if body_empty {
                    "Écrivez d'abord la scène"
                } else {
                    "Générer avec l'IA"
                };
                let gen = ui
                    .add_enabled(
                        !busy && !body_empty,
                        Button::new(if busy { "Génération…" } else { "Générer" }).icon(if busy {
                            &lucide::LOADER_CIRCLE
                        } else {
                            &lucide::SPARKLES
                        }),
                    )
                    .on_hover_text(tip);
                if gen.clicked() {
                    generate = true;
                }
                if ui
                    .add(Button::new("Fermer").variant(ButtonVariant::Outline))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    close = true;
                }
            });
        });

    if close {
        open = false;
    }
    state.synopsis_open = open;
    if generate {
        Some(EditorAction::GenerateSynopsis)
    } else {
        None
    }
}

pub fn show_note(
    ui: &mut Ui,
    note: &Note,
    state: &mut EditorState,
    brushes: &[Brush],
    code_mode: &mut bool,
    md_cache: &mut CommonMarkCache,
    lang: Lang,
) -> EditorOutcome {
    let mut out = EditorOutcome {
        actions: Vec::new(),
    };
    if state.id != note.meta.id || state.kind != EditorKind::Note {
        let disk_mtime_ms = ak_core::fsutil::mtime_millis(&note.path);
        let disk_hash = Some(ak_core::fsutil::content_hash(note.body.as_bytes()));
        *state = EditorState {
            kind: EditorKind::Note,
            id: note.meta.id.clone(),
            title: note.meta.name.clone(),
            body: note.body.clone(),
            disk_mtime_ms,
            disk_hash,
            ..EditorState::default()
        };
    }
    ui.label(
        RichText::new(lang.note_kind_singular(note.meta.kind))
            .size(FONT_UI_SM)
            .color(Theme::current(ui.ctx()).palette.muted_foreground),
    );
    header(ui, state, code_mode, lang, &mut out);
    draft_area(ui, state, *code_mode, md_cache, lang);
    selection_toolbar(ui.ctx(), state, brushes, &mut out, lang);
    out
}

pub fn show_styles(ui: &mut Ui, project: &mut Project) -> EditorOutcome {
    let mut out = EditorOutcome {
        actions: Vec::new(),
    };
    Card::new().show(ui, |ui| {
        card_content(ui, |ui| {
            ui.label(RichText::new("Styles d'écriture").size(FONT_TITLE).strong());
            ui.label(
                RichText::new("Injectés dans chaque appel IA.")
                    .size(FONT_UI_SM)
                    .weak(),
            );
            ui.add_space(8.0);
            ui.label("Règles");
            ui.add(
                TextEdit::multiline(&mut project.styles.rules)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            ui.label("Voix");
            ui.add(
                TextEdit::multiline(&mut project.styles.voice_notes)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.label("À éviter");
            ui.add(
                TextEdit::multiline(&mut project.styles.avoid)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            if widgets::primary(ui, "Enregistrer").clicked() {
                out.actions.push(EditorAction::SaveStyles);
            }
        });
    });
    out
}

pub fn show_publish(ui: &mut Ui, project: &Project) -> EditorOutcome {
    let mut out = EditorOutcome {
        actions: Vec::new(),
    };
    Card::new().show(ui, |ui| {
        card_content(ui, |ui| {
            ui.label(RichText::new("Publier").size(FONT_TITLE).strong());
            ui.label(
                RichText::new(format!(
                    "{} — {} mots",
                    project.meta.title,
                    project.manuscript.total_words()
                ))
                .size(FONT_UI_SM)
                .weak(),
            );
            ui.add_space(8.0);
            if ui
                .add(
                    Button::new("Exporter Markdown…")
                        .variant(ButtonVariant::Outline)
                        .icon(&lucide::FILE_TEXT),
                )
                .clicked()
            {
                out.actions.push(EditorAction::ExportMarkdown);
            }
            if ui
                .add(
                    Button::new("Exporter EPUB…")
                        .variant(ButtonVariant::Outline)
                        .icon(&lucide::BOOK),
                )
                .clicked()
            {
                out.actions.push(EditorAction::ExportEpub);
            }
            if ui
                .add(
                    Button::new("Exporter DOCX…")
                        .variant(ButtonVariant::Outline)
                        .icon(&lucide::DOWNLOAD),
                )
                .clicked()
            {
                out.actions.push(EditorAction::ExportDocx);
            }
            ui.add_space(12.0);
            if ui
                .add(Button::new("Blueprint roman (IA)…").icon(&lucide::SPARKLES))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                out.actions.push(EditorAction::Bootstrap);
            }
        });
    });
    out
}

/// Vega `ButtonSize::Sm` is 32px — status, synopsis, Visuel/Code, links and Relier share it.
const HEADER_BTN_H: f32 = 32.0;

enum PillStyle {
    /// Transparent until hover.
    Ghost,
    /// Filled primary — active segment (Visuel / Code / Synopsis).
    Solid,
    /// Status color, active item only.
    Tint(egui::Color32),
}

/// Distance from a galley's top to the visual middle of its capitals, measured on
/// the rasterized « H ». Center text on this, not on the line box.
fn cap_midline(ui: &Ui, font: &egui::FontId) -> f32 {
    let probe = ui
        .painter()
        .layout_no_wrap("H".to_owned(), font.clone(), egui::Color32::WHITE);
    probe
        .rows
        .first()
        .and_then(|r| {
            r.row
                .glyphs
                .first()
                .map(|g| r.pos.y + g.pos.y + g.uv_rect.offset.y + g.uv_rect.size.y * 0.5)
        })
        .unwrap_or(probe.size().y * 0.5)
}

/// Fixed-height labeled icon. Nothing inside can grow the row.
fn pill(ui: &mut Ui, label: &str, icon: &shadcn_egui::Icon, style: PillStyle) -> egui::Response {
    let pal = Theme::current(ui.ctx()).palette;
    let (fg, fill, stroke) = match style {
        PillStyle::Ghost => (
            pal.foreground,
            egui::Color32::TRANSPARENT,
            egui::Stroke::NONE,
        ),
        PillStyle::Solid => (pal.primary_foreground, pal.primary, egui::Stroke::NONE),
        PillStyle::Tint(color) => (
            color,
            color.gamma_multiply(0.28),
            egui::Stroke::new(1.0_f32, color),
        ),
    };
    let galley = ui.painter().layout(
        label.to_owned(),
        egui::FontId::proportional(FONT_UI_SM),
        fg,
        f32::INFINITY,
    );
    let icon_sz = 14.0_f32;
    // Vega Sm: 6px leading pad with icon, 4px gap, 10px trailing pad.
    let w = 6.0 + icon_sz + 4.0 + galley.size().x + 10.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HEADER_BTN_H), egui::Sense::click());
    let resp = widgets::pointer(resp);
    let fill = if matches!(style, PillStyle::Ghost) && resp.hovered() {
        pal.muted
    } else {
        fill
    };
    if fill != egui::Color32::TRANSPARENT || stroke != egui::Stroke::NONE {
        ui.painter()
            .rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
    }
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 6.0 + icon_sz * 0.5, rect.center().y),
        egui::Vec2::splat(icon_sz),
    );
    shadcn_egui::components::icon::paint_icon(ui.painter(), icon_rect, icon, fg, 2.0);
    let text_pos = egui::pos2(
        icon_rect.right() + 4.0,
        rect.center().y - cap_midline(ui, &egui::FontId::proportional(FONT_UI_SM)),
    );
    ui.painter().galley(text_pos, galley, fg);
    resp
}

fn header_cartouche(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    let pal = Theme::current(ui.ctx()).palette;
    egui::Frame::new()
        .fill(pal.muted)
        .stroke(egui::Stroke::new(1.0_f32, pal.border))
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                ui.set_min_height(HEADER_BTN_H);
                add_contents(ui);
            });
        });
}

fn mode_switch(ui: &mut Ui, code_mode: &mut bool, lang: Lang) {
    let visual_on = !*code_mode;
    header_cartouche(ui, |ui| {
        if pill(
            ui,
            lang.visual(),
            &lucide::EYE,
            if visual_on {
                PillStyle::Solid
            } else {
                PillStyle::Ghost
            },
        )
        .on_hover_text(lang.visual_tip())
        .clicked()
            && *code_mode
        {
            *code_mode = false;
        }
        if pill(
            ui,
            lang.code(),
            &lucide::CODE,
            if *code_mode {
                PillStyle::Solid
            } else {
                PillStyle::Ghost
            },
        )
        .on_hover_text(lang.code_tip())
        .clicked()
            && !*code_mode
        {
            *code_mode = true;
        }
    });
}

fn status_switch(ui: &mut Ui, state: &mut EditorState) {
    let dark = Theme::current(ui.ctx()).dark;
    header_cartouche(ui, |ui| {
        for st in ak_core::SceneStatus::all() {
            let on = state.scene_status == st;
            let label = st.label_fr();
            let style = if on {
                PillStyle::Tint(crate::theme::scene_status_color(st, dark))
            } else {
                PillStyle::Ghost
            };
            if pill(ui, label, widgets::scene_status_icon(st), style)
                .on_hover_text(label)
                .clicked()
                && !on
            {
                state.scene_status = st;
                state.mark_dirty(ui.ctx().input(|i| i.time));
            }
        }
    });
}

fn synopsis_button(ui: &mut Ui, state: &mut EditorState) {
    let has_syn = !state.synopsis.trim().is_empty();
    let tip = if has_syn {
        "Éditer le synopsis"
    } else {
        "Ajouter un synopsis"
    };
    header_cartouche(ui, |ui| {
        if pill(
            ui,
            "Synopsis",
            &lucide::BOOK_TEXT,
            if has_syn {
                PillStyle::Solid
            } else {
                PillStyle::Ghost
            },
        )
        .on_hover_text(tip)
        .clicked()
        {
            state.synopsis_open = true;
        }
    });
}

struct LinkHit {
    open: bool,
    remove: bool,
}

/// Link chip at exactly [`HEADER_BTN_H`] — same as the Relier button (`ButtonSize::Sm`).
fn link_chip(
    ui: &mut Ui,
    name: &str,
    icon: &shadcn_egui::Icon,
    fg: egui::Color32,
    fill: egui::Color32,
    border: egui::Color32,
) -> LinkHit {
    let galley = ui.painter().layout(
        name.to_owned(),
        egui::FontId::proportional(FONT_UI_SM),
        fg,
        f32::INFINITY,
    );
    let icon_sz = 14.0_f32;
    let x_sz = 14.0_f32;
    let w = 6.0 + icon_sz + 4.0 + galley.size().x + 6.0 + x_sz + 8.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, HEADER_BTN_H), egui::Sense::click());
    let resp = widgets::pointer(resp);
    ui.painter().rect(
        rect,
        8.0,
        fill,
        egui::Stroke::new(1.0_f32, border),
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 6.0 + icon_sz * 0.5, rect.center().y),
        egui::Vec2::splat(icon_sz),
    );
    shadcn_egui::components::icon::paint_icon(ui.painter(), icon_rect, icon, fg, 2.0);
    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + 4.0,
            rect.center().y - cap_midline(ui, &egui::FontId::proportional(FONT_UI_SM)),
        ),
        galley,
        fg,
    );
    let x_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 8.0 - x_sz * 0.5, rect.center().y),
        egui::Vec2::splat(x_sz),
    );
    shadcn_egui::components::icon::paint_icon(ui.painter(), x_rect, &lucide::X, fg, 2.0);
    let pointer = resp
        .interact_pointer_pos()
        .or_else(|| ui.input(|i| i.pointer.interact_pos()));
    let on_x = pointer.is_some_and(|p| x_rect.expand(4.0).contains(p));
    if on_x {
        resp.clone().on_hover_text("Retirer");
    }
    LinkHit {
        open: resp.clicked() && !on_x,
        remove: resp.clicked() && on_x,
    }
}

/// Editable title + status / synopsis / Visual·Code. Word count / save live in the status bar.
fn header(
    ui: &mut Ui,
    state: &mut EditorState,
    code_mode: &mut bool,
    lang: Lang,
    _out: &mut EditorOutcome,
) {
    let pal = Theme::current(ui.ctx()).palette;
    // Explicit geometry: the row is exactly as tall as the cartouches (measured last
    // frame), controls are pinned right, and the title is placed by its cap-height
    // midline — egui centers the line box, whose descender space makes text sit high.
    let size_id = ui.id().with("ak_header_controls_size");
    let stored: egui::Vec2 = ui
        .ctx()
        .data(|d| d.get_temp(size_id))
        .unwrap_or(egui::vec2(720.0, HEADER_BTN_H + 8.0));
    let (row, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), stored.y),
        egui::Sense::hover(),
    );

    let controls_rect = egui::Rect::from_min_max(
        egui::pos2((row.right() - stored.x).max(row.left()), row.top()),
        row.max,
    );
    let controls = ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(controls_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            if state.kind == EditorKind::Scene {
                status_switch(ui, state);
                synopsis_button(ui, state);
            }
            mode_switch(ui, code_mode, lang);
        },
    );
    let measured = controls.response.rect.size();
    if (measured - stored).length() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(size_id, measured));
        ui.ctx().request_discard("editor header measured");
    }

    let font = egui::FontId::proportional(FONT_TITLE + 2.0);
    let line_h = ui.fonts_mut(|f| f.row_height(&font));
    let title_rect = egui::Rect::from_min_max(
        egui::pos2(row.left(), row.center().y - cap_midline(ui, &font)),
        egui::pos2(
            (controls_rect.left() - 12.0).max(row.left() + 48.0),
            row.center().y - cap_midline(ui, &font) + line_h,
        ),
    );
    let title_edit = ui
        .put(
            title_rect,
            TextEdit::singleline(&mut state.title)
                .font(font)
                .desired_width(title_rect.width())
                .frame(egui::Frame::NONE)
                .text_color(pal.foreground)
                .hint_text(lang.title_hint()),
        )
        .on_hover_cursor(egui::CursorIcon::Text);
    if title_edit.changed() {
        state.mark_dirty(ui.ctx().input(|i| i.time));
    }
    ui.add_space(10.0);
    separator(ui);
    ui.add_space(4.0);
}
