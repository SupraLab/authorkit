use ak_coherence::{ContentIdea, ReviewReport};
use ak_core::{Project, Scene};
use ak_llm::{Brush, ChatMessage};
use egui::{RichText, ScrollArea, TextEdit, Ui};
use shadcn_egui::components::button::{Button, ButtonSize, ButtonVariant};
use shadcn_egui::components::tabs::{Tabs, TabsVariant};
use shadcn_egui::Theme;

use crate::chat_store::{ComposerAttachment, ComposerMode};
use crate::theme::{FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Chat,
    Context,
    Ideas,
    Reviews,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgStatus {
    Pending,
    Streaming,
    Done,
    Cancelled,
    Error,
}

#[derive(Debug, Clone)]
pub struct ChatMsg {
    pub id: String,
    pub role: String,
    pub content: String,
    pub status: MsgStatus,
    /// True when this assistant reply looks like insertable prose.
    pub offers_insert: bool,
    pub payload: crate::chat_store::MessagePayload,
}

impl ChatMsg {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: "user".into(),
            content: content.into(),
            status: MsgStatus::Done,
            offers_insert: false,
            payload: crate::chat_store::MessagePayload::Chat,
        }
    }

    pub fn assistant_pending() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: "assistant".into(),
            content: String::new(),
            status: MsgStatus::Pending,
            offers_insert: false,
            payload: crate::chat_store::MessagePayload::Chat,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_payload(mut self, payload: crate::chat_store::MessagePayload) -> Self {
        self.payload = payload;
        self
    }
}

pub struct SidebarState {
    pub tab: SidebarTab,
    pub input: String,
    pub messages: Vec<ChatMsg>,
    pub conversation_id: String,
    pub conversation_title: String,
    pub title_generated: bool,
    /// Bound left-nav context (`scene:…`, `note:…`, …).
    pub context_id: String,
    pub show_history: bool,
    pub busy: bool,
    pub status: String,
    pub ideas: Vec<ContentIdea>,
    pub review: Option<ReviewReport>,
    /// Stick scroll to bottom only when user is already there.
    pub stick_bottom: bool,
    pub show_jump_bottom: bool,
    /// Active model label for footer hint.
    pub model_label: String,
    pub context_hint: String,
    /// Streaming assistant message id.
    pub streaming_id: Option<String>,
    /// Assembled context pieces (Context tab).
    pub context_pieces: Vec<ak_llm::ContextPiece>,
    pub context_omitted_history: usize,
    pub context_doc_chars: usize,
    pub context_budget_chars: usize,
    pub composer_mode: ComposerMode,
    pub attachment: Option<ComposerAttachment>,
    pub mode_menu_open: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            tab: SidebarTab::Chat,
            input: String::new(),
            messages: Vec::new(),
            conversation_id: String::new(),
            conversation_title: "Nouvelle conversation".into(),
            title_generated: false,
            context_id: String::new(),
            show_history: false,
            busy: false,
            status: String::new(),
            ideas: Vec::new(),
            review: None,
            stick_bottom: true,
            show_jump_bottom: false,
            model_label: String::new(),
            context_hint: String::new(),
            streaming_id: None,
            context_pieces: Vec::new(),
            context_omitted_history: 0,
            context_doc_chars: 0,
            context_budget_chars: 0,
            composer_mode: ComposerMode::Chat,
            attachment: None,
            mode_menu_open: false,
        }
    }
}

impl SidebarState {
    pub fn to_llm_history(&self) -> Vec<ChatMessage> {
        self.messages
            .iter()
            .filter(|m| !m.payload.is_edit_proposal())
            .filter(|m| m.status == MsgStatus::Done || m.status == MsgStatus::Streaming)
            .filter(|m| !m.content.is_empty())
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect()
    }

    pub fn from_stored(conv: crate::chat_store::StoredConversation) -> Self {
        let messages = conv
            .messages
            .into_iter()
            .map(|m| {
                let status = match m.status {
                    crate::chat_store::StoredMsgStatus::Pending => MsgStatus::Pending,
                    crate::chat_store::StoredMsgStatus::Streaming => MsgStatus::Streaming,
                    crate::chat_store::StoredMsgStatus::Done => MsgStatus::Done,
                    crate::chat_store::StoredMsgStatus::Cancelled => MsgStatus::Cancelled,
                    crate::chat_store::StoredMsgStatus::Error => MsgStatus::Error,
                };
                let offers_insert = m.role == "assistant"
                    && status == MsgStatus::Done
                    && !m.payload.is_edit_proposal()
                    && !m.content.trim().is_empty();
                ChatMsg {
                    id: if m.id.is_empty() {
                        uuid::Uuid::new_v4().to_string()
                    } else {
                        m.id
                    },
                    role: m.role,
                    content: m.content,
                    status,
                    offers_insert,
                    payload: m.payload,
                }
            })
            .collect();
        let mut s = Self {
            conversation_id: conv.id,
            conversation_title: conv.title,
            title_generated: conv.title_generated,
            context_id: conv.context_id,
            input: conv.draft,
            composer_mode: conv.composer_mode,
            attachment: conv.attachment,
            messages,
            ..Default::default()
        };
        if !s.title_generated {
            if let Some(first) = s.messages.iter().find(|m| m.role == "user") {
                s.conversation_title = intent_title(first);
            }
        }
        s
    }

    pub fn clear_composer(&mut self) {
        self.input.clear();
        self.composer_mode = ComposerMode::Chat;
        self.attachment = None;
        self.mode_menu_open = false;
    }

    pub fn composer_dirty(&self) -> bool {
        !self.input.trim().is_empty() || self.attachment.is_some() || !self.composer_mode.is_chat()
    }

    pub fn can_send(&self) -> bool {
        if self.busy {
            return false;
        }
        match &self.composer_mode {
            ComposerMode::Chat => !self.input.trim().is_empty() || self.attachment.is_some(),
            ComposerMode::Continue { .. } => true,
            ComposerMode::Brush { .. } => self.attachment.is_some(),
        }
    }

    pub fn to_stored_messages(&self) -> Vec<crate::chat_store::StoredMessage> {
        self.messages
            .iter()
            .map(|m| crate::chat_store::StoredMessage {
                id: m.id.clone(),
                role: m.role.clone(),
                content: m.content.clone(),
                status: match m.status {
                    MsgStatus::Pending => crate::chat_store::StoredMsgStatus::Pending,
                    MsgStatus::Streaming => crate::chat_store::StoredMsgStatus::Streaming,
                    MsgStatus::Done => crate::chat_store::StoredMsgStatus::Done,
                    MsgStatus::Cancelled => crate::chat_store::StoredMsgStatus::Cancelled,
                    MsgStatus::Error => crate::chat_store::StoredMsgStatus::Error,
                },
                created_at: crate::chat_store::stamp_now(),
                payload: m.payload.clone(),
            })
            .collect()
    }
}

pub enum SidebarAction {
    SendChat,
    CancelChat,
    NewConversation,
    LoadConversation(String),
    CopyMessage(String),
    InsertMessage(String),
    ReplaceWithMessage(String),
    RetryMessage(String),
    RegenerateMessage(String),
    AcceptEditProposal(String),
    RejectEditProposal(String),
    RegenerateEditProposal(String),
    StopEditProposal(String),
    SetComposerMode(ComposerMode),
    ClearAttachment,
    ClearComposer,
    JumpToBottom,
    RefreshIdeas,
    WriteIdea(usize),
    RunReview,
    SetReviewStatus {
        item_id: String,
        status: ak_coherence::ReviewStatus,
    },
}

pub fn show(
    ui: &mut Ui,
    project: &Project,
    scene: Option<&Scene>,
    state: &mut SidebarState,
    brushes: &[Brush],
) -> Vec<SidebarAction> {
    let mut actions = Vec::new();
    let mut idx = match state.tab {
        SidebarTab::Chat => 0,
        SidebarTab::Context => 1,
        SidebarTab::Ideas => 2,
        SidebarTab::Reviews => 3,
    };
    if ui
        .add(
            Tabs::new(&mut idx, &["Chat", "Contexte", "Idées", "Révisions"])
                .variant(TabsVariant::Line)
                .width(f32::INFINITY),
        )
        .changed()
    {
        state.tab = match idx {
            1 => SidebarTab::Context,
            2 => SidebarTab::Ideas,
            3 => SidebarTab::Reviews,
            _ => SidebarTab::Chat,
        };
    }
    ui.add_space(Tabs::content_gap(ui));

    match state.tab {
        SidebarTab::Chat => chat_panel(ui, project, scene, state, brushes, &mut actions),
        SidebarTab::Context => context_panel(ui, project, scene, state),
        SidebarTab::Ideas => ideas_panel(ui, state, &mut actions),
        SidebarTab::Reviews => reviews_panel(ui, state, &mut actions),
    }

    if !state.status.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new(&state.status).size(FONT_UI_SM).weak());
    }
    actions
}

fn chat_panel(
    ui: &mut Ui,
    project: &Project,
    scene: Option<&Scene>,
    state: &mut SidebarState,
    brushes: &[Brush],
    actions: &mut Vec<SidebarAction>,
) {
    let pal = Theme::current(ui.ctx()).palette;
    // Explicit top-down — do NOT inherit bottom_up from parents.
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_max_width(ui.available_width());

        // 1. Header
        let mut history_btn_rect = egui::Rect::NOTHING;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&state.conversation_title)
                    .size(FONT_UI)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let hist = widgets::icon_btn(ui, &lucide::HISTORY, "Historique");
                history_btn_rect = hist.rect;
                if hist.clicked() {
                    state.show_history = !state.show_history;
                }
                if widgets::icon_btn(ui, &lucide::PLUS, "Nouvelle conversation").clicked() {
                    actions.push(SidebarAction::NewConversation);
                }
            });
        });
        ui.add_space(4.0);

        if state.show_history {
            let ctx = ui.ctx().clone();
            let popup_pos = egui::pos2(history_btn_rect.right(), history_btn_rect.bottom() + 4.0);
            let convs = if state.context_id.is_empty() {
                crate::chat_store::list_conversations(&project.root)
            } else {
                crate::chat_store::list_for_context(&project.root, &state.context_id)
            };
            let popup = egui::Area::new(egui::Id::new("ak_chat_history"))
                .order(egui::Order::Foreground)
                .fixed_pos(popup_pos)
                .pivot(egui::Align2::RIGHT_TOP)
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
                            ui.set_width(260.0);
                            ui.label(
                                RichText::new("Historique")
                                    .size(FONT_UI)
                                    .strong()
                                    .color(pal.foreground),
                            );
                            ui.add_space(6.0);
                            if convs.is_empty() {
                                ui.label(
                                    RichText::new("Aucune conversation")
                                        .size(FONT_UI_SM)
                                        .color(pal.muted_foreground),
                                );
                            } else {
                                let list_w = ui.available_width();
                                ScrollArea::vertical()
                                    .id_salt("ak_chat_history_scroll")
                                    .max_height(280.0)
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.set_width(list_w);
                                        ui.spacing_mut().item_spacing.y = 6.0;
                                        for c in &convs {
                                            let selected = c.id == state.conversation_id;
                                            let when =
                                                crate::chat_store::format_stamp(&c.updated_at);
                                            let fill = if selected {
                                                pal.accent.gamma_multiply(0.22)
                                            } else {
                                                pal.muted.gamma_multiply(0.45)
                                            };
                                            let stroke = if selected {
                                                egui::Stroke::new(1.0_f32, pal.accent)
                                            } else {
                                                egui::Stroke::new(1.0_f32, pal.border)
                                            };
                                            let resp = egui::Frame::new()
                                                .fill(fill)
                                                .stroke(stroke)
                                                .corner_radius(8.0)
                                                .inner_margin(egui::Margin::symmetric(10, 8))
                                                .show(ui, |ui| {
                                                    ui.set_width(list_w - 4.0);
                                                    ui.label(
                                                        RichText::new(&c.title)
                                                            .size(FONT_UI)
                                                            .strong()
                                                            .color(pal.foreground),
                                                    );
                                                    if when != "—" && !when.is_empty() {
                                                        ui.label(
                                                            RichText::new(&when)
                                                                .size(FONT_UI_SM)
                                                                .color(pal.muted_foreground),
                                                        );
                                                    }
                                                })
                                                .response
                                                .on_hover_cursor(
                                                    egui::CursorIcon::PointingHand,
                                                )
                                                .interact(egui::Sense::click());
                                            if resp.hovered() && !selected {
                                                ui.painter().rect(
                                                    resp.rect,
                                                    8.0,
                                                    egui::Color32::TRANSPARENT,
                                                    egui::Stroke::new(
                                                        1.0_f32,
                                                        pal.foreground.gamma_multiply(0.35),
                                                    ),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                            if resp.clicked() {
                                                actions.push(SidebarAction::LoadConversation(
                                                    c.id.clone(),
                                                ));
                                                state.show_history = false;
                                            }
                                        }
                                    });
                            }
                        });
                });

            let pointer = ctx.pointer_latest_pos().unwrap_or(egui::Pos2::ZERO);
            let over_btn = history_btn_rect.contains(pointer);
            let over_popup = popup.response.contains_pointer() || popup.response.hovered();
            if ctx.input(|i| i.pointer.any_click()) && !over_btn && !over_popup {
                state.show_history = false;
            }
        }

        // Context / model hint
        if !state.model_label.is_empty() || !state.context_hint.is_empty() {
            ui.label(
                RichText::new(format!(
                    "{}{}",
                    if state.model_label.is_empty() {
                        String::new()
                    } else {
                        format!("{} · ", state.model_label)
                    },
                    state.context_hint
                ))
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
            );
            ui.add_space(2.0);
        }

        // Pin composer to the bottom with a fixed pad. Growth (attachment, wrap)
        // expands upward into the message list — bottom margin never moves.
        // Panel frame already pads 12px at the bottom; +2 matches the 14px sides.
        const BOTTOM_PAD: f32 = 2.0;
        const GAP_ABOVE: f32 = 6.0;
        let area = ui.available_rect_before_wrap();
        let height_id = egui::Id::new("ak_composer_body_h");
        let composer_body: f32 = ui.ctx().data(|d| {
            d.get_temp(height_id).unwrap_or(if state.attachment.is_some() {
                96.0
            } else {
                72.0
            })
        });
        let jump_h = if state.show_jump_bottom { 28.0 } else { 0.0 };
        let strip_h = jump_h + GAP_ABOVE + composer_body + BOTTOM_PAD;
        let strip = egui::Rect::from_min_max(
            egui::pos2(area.left(), (area.bottom() - strip_h).max(area.top())),
            area.max,
        );
        let messages_rect =
            egui::Rect::from_min_max(area.min, egui::pos2(area.right(), strip.top()));

        let scroll = ui
            .scope_builder(egui::UiBuilder::new().max_rect(messages_rect), |ui| {
                ScrollArea::vertical()
                    .id_salt("ak_chat_scroll")
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .stick_to_bottom(state.stick_bottom)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                            if state.messages.is_empty() {
                                ui.add_space(24.0);
                                ui.vertical_centered(|ui| {
                                    let _ = ui.add(
                                        shadcn_egui::components::icon::IconView::new(
                                            &lucide::SPARKLES,
                                        )
                                        .size(20.0)
                                        .color(pal.muted_foreground),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Assistant d'écriture")
                                            .size(FONT_UI)
                                            .strong(),
                                    );
                                    let hint = if let Some(s) = scene {
                                        format!(
                                            "Scène : {}\nAucune conversation pour ce contexte — + pour démarrer.",
                                            s.meta.title
                                        )
                                    } else if !state.context_hint.is_empty() {
                                        format!(
                                            "{}\nAucune conversation pour ce contexte — + pour démarrer.",
                                            state.context_hint
                                        )
                                    } else {
                                        "Aucune conversation pour ce contexte — + pour démarrer."
                                            .into()
                                    };
                                    ui.label(
                                        RichText::new(hint)
                                            .size(FONT_UI_SM)
                                            .color(pal.muted_foreground),
                                    );
                                });
                            } else {
                                for (i, msg) in state.messages.iter().enumerate() {
                                    ui.add_space(8.0);
                                    if msg.payload.is_edit_proposal() {
                                        edit_proposal_card(ui, msg, actions);
                                        continue;
                                    }
                                    let is_user = msg.role == "user";
                                    if is_user {
                                        let avail = ui.available_width();
                                        ui.horizontal_top(|ui| {
                                            ui.add_space(
                                                (avail - bubble_outer_width(avail)).max(0.0),
                                            );
                                            // Older turns lack a tool key — borrow it from
                                            // the proposal card answering them.
                                            let hint = state
                                                .messages
                                                .get(i + 1)
                                                .and_then(proposal_tool_key);
                                            chat_bubble(ui, msg, true, avail, hint, actions);
                                        });
                                    } else {
                                        let row_w = ui.available_width();
                                        chat_bubble(ui, msg, false, row_w, None, actions);
                                    }
                                }
                                ui.add_space(8.0);
                            }
                        });
                    })
            })
            .inner;

        // Detect if user scrolled away from bottom
        let at_bottom = scroll.state.offset.y
            >= (scroll.content_size.y - scroll.inner_rect.height() - 24.0).max(0.0);
        if state.busy {
            if at_bottom {
                state.stick_bottom = true;
                state.show_jump_bottom = false;
            } else {
                state.stick_bottom = false;
                state.show_jump_bottom = true;
            }
        } else if at_bottom {
            state.show_jump_bottom = false;
            state.stick_bottom = true;
        }

        ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
            if state.show_jump_bottom {
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            Button::new("Aller aux derniers messages")
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Sm),
                        )
                        .clicked()
                    {
                        state.stick_bottom = true;
                        state.show_jump_bottom = false;
                        actions.push(SidebarAction::JumpToBottom);
                    }
                });
            }
            ui.add_space(GAP_ABOVE);
            let top = ui.cursor().top();
            composer_panel(ui, state, brushes, actions);
            let measured = (ui.min_rect().bottom() - top).max(48.0);
            ui.ctx()
                .data_mut(|d| d.insert_temp(height_id, measured));
            // BOTTOM_PAD is the unused band below the composer inside `strip`.
        });
        ui.advance_cursor_after_rect(area);
    });
}

fn composer_panel(
    ui: &mut Ui,
    state: &mut SidebarState,
    brushes: &[Brush],
    actions: &mut Vec<SidebarAction>,
) {
    let pal = Theme::current(ui.ctx()).palette;
    let mut mode_btn_rect = egui::Rect::NOTHING;
    // Brushes are only meaningful with a linked selection.
    if state.attachment.is_none() && matches!(state.composer_mode, ComposerMode::Brush { .. }) {
        state.composer_mode = ComposerMode::Chat;
    }

    egui::Frame::new()
        .fill(pal.muted.gamma_multiply(0.4))
        .stroke(egui::Stroke::new(1.0_f32, pal.border))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Attachment chip only when present — no empty reserved band.
            if let Some(att) = state.attachment.as_ref() {
                let plain = plain_excerpt(&att.text);
                let excerpt: String = plain.chars().take(40).collect();
                let label = if plain.chars().count() > 40 {
                    format!("« {excerpt}… »")
                } else {
                    format!("« {excerpt} »")
                };
                if attachment_chip(ui, &label) {
                    actions.push(SidebarAction::ClearAttachment);
                }
                ui.add_space(4.0);
            }

            let hint = match &state.composer_mode {
                ComposerMode::Chat => "Message…",
                ComposerMode::Continue { mode } if mode == "generate" => "Consigne de génération…",
                ComposerMode::Continue { .. } => "Consigne optionnelle…",
                ComposerMode::Brush { .. } => "Consigne optionnelle…",
            };
            let edit = ui.add(
                TextEdit::multiline(&mut state.input)
                    .id_salt("ak_chat_input")
                    .desired_rows(1)
                    .desired_width(f32::INFINITY)
                    .margin(egui::Margin::symmetric(0, 2))
                    .frame(egui::Frame::NONE)
                    .hint_text(hint),
            );
            let chat_focused = edit.has_focus();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let mode_label = state.composer_mode.label(brushes).to_string();
                let mode_icon = composer_mode_icon(&state.composer_mode);
                let mode_btn = mode_chip(ui, mode_icon, &mode_label)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Choisir une action");
                mode_btn_rect = mode_btn.rect;
                if mode_btn.clicked() {
                    state.mode_menu_open = !state.mode_menu_open;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if state.busy {
                        if ui
                            .add(
                                Button::new("")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::IconSm)
                                    .icon(&lucide::SQUARE),
                            )
                            .on_hover_text("Arrêter")
                            .clicked()
                        {
                            actions.push(SidebarAction::CancelChat);
                        }
                    } else {
                        let ready = state.can_send();
                        if ui
                            .add_enabled(
                                ready,
                                Button::new("")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::IconSm)
                                    .icon(&lucide::SEND),
                            )
                            .on_hover_text("Envoyer")
                            .clicked()
                        {
                            actions.push(SidebarAction::SendChat);
                        }
                        // Always allocate the clear control so the send icon does not shift.
                        let dirty = state.composer_dirty();
                        let clear = ui
                            .add_enabled(
                                dirty,
                                Button::new("")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::IconSm)
                                    .icon(&lucide::ERASER),
                            )
                            .on_hover_text("Tout effacer");
                        if dirty && clear.clicked() {
                            actions.push(SidebarAction::ClearComposer);
                        }
                    }
                });
            });

            if chat_focused {
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let shift = ui.input(|i| i.modifiers.shift);
                let cmd = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                if enter && state.can_send() && (!shift || cmd) {
                    actions.push(SidebarAction::SendChat);
                }
            }
        });

    if state.mode_menu_open {
        let ctx = ui.ctx().clone();
        let popup_pos = egui::pos2(mode_btn_rect.left(), mode_btn_rect.top() - 4.0);
        let current = state.composer_mode.clone();
        let popup = egui::Area::new(egui::Id::new("ak_composer_mode_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .interactable(true)
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(pal.popover)
                    .stroke(egui::Stroke::new(1.0_f32, pal.border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(6, 6))
                    .shadow(egui::Shadow {
                        offset: [0, 2],
                        blur: 10,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(28),
                    })
                    .show(ui, |ui| {
                        ui.set_width(180.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut pick = |mode: ComposerMode| {
                            actions.push(SidebarAction::SetComposerMode(mode));
                            state.mode_menu_open = false;
                        };
                        composer_mode_row(
                            ui,
                            composer_mode_icon(&ComposerMode::Chat),
                            "Chat",
                            matches!(current, ComposerMode::Chat),
                            || pick(ComposerMode::Chat),
                        );
                        composer_mode_row(
                            ui,
                            composer_mode_icon(&ComposerMode::continue_mode("continue")),
                            "Continuer",
                            matches!(
                                current,
                                ComposerMode::Continue { ref mode } if mode == "continue"
                            ),
                            || pick(ComposerMode::continue_mode("continue")),
                        );
                        if state.attachment.is_some() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Outils")
                                    .size(FONT_UI_SM)
                                    .color(pal.muted_foreground),
                            );
                            for b in brushes {
                                let selected = matches!(
                                    &current,
                                    ComposerMode::Brush { id } if id == &b.id
                                );
                                let mode = ComposerMode::brush(b.id.clone());
                                composer_mode_row(
                                    ui,
                                    composer_mode_icon(&mode),
                                    &b.label,
                                    selected,
                                    || pick(mode),
                                );
                            }
                        }
                    });
            });

        let pointer = ctx.pointer_latest_pos().unwrap_or(egui::Pos2::ZERO);
        let over_btn = mode_btn_rect.contains(pointer);
        let over_popup = popup.response.contains_pointer() || popup.response.hovered();
        if ctx.input(|i| i.pointer.any_click()) && !over_btn && !over_popup {
            state.mode_menu_open = false;
        }
    }
}

/// Linked-text pill with its remove ✕ inside (like editor link chips).
/// Returns true when the ✕ was clicked.
fn attachment_chip(ui: &mut Ui, label: &str) -> bool {
    let pal = Theme::current(ui.ctx()).palette;
    let h = 28.0;
    let icon_px = 14.0;
    let x_px = 12.0;
    let pad_x = 10.0;
    let gap = 6.0;
    let max_text_w = (ui.available_width() - (pad_x * 2.0 + icon_px + x_px + gap * 2.0)).max(40.0);
    let job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: max_text_w,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..egui::text::LayoutJob::simple_singleline(
            label.to_string(),
            egui::FontId::proportional(FONT_UI_SM),
            pal.foreground,
        )
    };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let w = pad_x + icon_px + gap + galley.size().x + gap + x_px + pad_x;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let x_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - pad_x - x_px / 2.0, rect.center().y),
        egui::Vec2::splat(x_px),
    );
    let on_x = resp
        .hover_pos()
        .is_some_and(|p| x_rect.expand(5.0).contains(p));
    ui.painter().rect(
        rect,
        h / 2.0,
        pal.foreground.gamma_multiply(0.10),
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + pad_x + icon_px / 2.0, rect.center().y),
        egui::Vec2::splat(icon_px),
    );
    shadcn_egui::components::icon::paint_icon(
        ui.painter(),
        icon_rect,
        &lucide::TEXT_QUOTE,
        pal.muted_foreground,
        2.0,
    );
    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + gap,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        pal.foreground,
    );
    let x_color = if on_x {
        pal.foreground
    } else {
        pal.muted_foreground
    };
    shadcn_egui::components::icon::paint_icon(ui.painter(), x_rect, &lucide::X, x_color, 2.0);
    if on_x {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        resp.clone().on_hover_text("Retirer la sélection");
    }
    resp.clicked() && on_x
}

/// Pill-shaped mode picker, sized like the composer's icon buttons (28px).
fn mode_chip(ui: &mut Ui, icon: &shadcn_egui::Icon, label: &str) -> egui::Response {
    let pal = Theme::current(ui.ctx()).palette;
    let h = 28.0;
    let icon_px = 14.0;
    let pad_x = 10.0;
    let gap = 6.0;
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(FONT_UI),
        pal.foreground,
    );
    let size = egui::vec2(pad_x * 2.0 + icon_px + gap + galley.size().x, h);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    // Foreground tint → lighter than the composer in dark, darker in light.
    let fill = pal
        .foreground
        .gamma_multiply(if resp.hovered() { 0.16 } else { 0.10 });
    ui.painter().rect(
        rect,
        h / 2.0,
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad_x, rect.center().y - icon_px / 2.0),
        egui::vec2(icon_px, icon_px),
    );
    let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(icon_rect));
    let _ = icon_ui.add(
        shadcn_egui::components::icon::IconView::new(icon)
            .size(icon_px)
            .color(pal.foreground),
    );
    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + gap,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        pal.foreground,
    );
    resp
}

fn composer_mode_icon(mode: &ComposerMode) -> &'static shadcn_egui::Icon {
    match mode {
        ComposerMode::Chat => &lucide::MESSAGE_SQUARE,
        ComposerMode::Continue { mode } => match mode.as_str() {
            "sentence" => &lucide::PEN_LINE,
            "paragraph" => &lucide::FILE_TEXT,
            "generate" => &lucide::WAND_SPARKLES,
            _ => &lucide::SPARKLES,
        },
        ComposerMode::Brush { id } => brush_icon(id),
    }
}

fn brush_icon(id: &str) -> &'static shadcn_egui::Icon {
    match id {
        "improve" => &lucide::SPARKLES,
        "shorten" => &lucide::SHRINK,
        "lengthen" => &lucide::EXPAND,
        "grammar" => &lucide::SPELL_CHECK_2,
        "simplify" => &lucide::A_LARGE_SMALL,
        "readable" => &lucide::SCAN_TEXT,
        _ => &lucide::WAND_SPARKLES,
    }
}

fn composer_mode_row(
    ui: &mut Ui,
    icon: &shadcn_egui::Icon,
    label: &str,
    selected: bool,
    on_click: impl FnOnce(),
) {
    let pal = Theme::current(ui.ctx()).palette;
    let fill = if selected {
        pal.accent.gamma_multiply(0.22)
    } else {
        egui::Color32::TRANSPARENT
    };
    let row_h = 28.0;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_h),
        egui::Sense::click(),
    );
    ui.painter().rect(
        rect,
        6.0,
        fill,
        egui::Stroke::NONE,
        egui::StrokeKind::Inside,
    );
    let icon_px = 14.0;
    let icon_pos = egui::pos2(rect.left() + 8.0, rect.center().y - icon_px / 2.0);
    let mut icon_ui = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
        icon_pos,
        egui::vec2(icon_px, icon_px),
    )));
    let _ = icon_ui.add(
        shadcn_egui::components::icon::IconView::new(icon)
            .size(icon_px)
            .color(pal.foreground),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 28.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(FONT_UI),
        pal.foreground,
    );
    if selected {
        let check_pos = egui::pos2(rect.right() - 18.0, rect.center().y - icon_px / 2.0);
        let mut check_ui = ui.new_child(egui::UiBuilder::new().max_rect(
            egui::Rect::from_min_size(check_pos, egui::vec2(icon_px, icon_px)),
        ));
        let _ = check_ui.add(
            shadcn_egui::components::icon::IconView::new(&lucide::CHECK)
                .size(icon_px)
                .color(pal.foreground),
        );
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        on_click();
    }
}

fn edit_proposal_card(ui: &mut Ui, msg: &ChatMsg, actions: &mut Vec<SidebarAction>) {
    use crate::chat_store::{EditApplyMode, EditProposalStatus, MessagePayload};

    let MessagePayload::EditProposal {
        label,
        mode,
        proposal_status,
        conflict,
        error,
        regen,
        ..
    } = &msg.payload
    else {
        return;
    };
    let pal = Theme::current(ui.ctx()).palette;
    let dark = Theme::current(ui.ctx()).dark;
    let fill = crate::theme::ai_bubble_fill(dark);
    let content_w = bubble_content_width(ui.available_width());
    let icon = tool_icon(Some(&regen_tool_key(regen)));

    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, pal.border))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(content_w);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let _ = ui.add(
                    shadcn_egui::components::icon::IconView::new(icon)
                        .size(14.0)
                        .color(pal.foreground),
                );
                ui.label(
                    RichText::new(label)
                        .size(FONT_UI)
                        .strong()
                        .color(pal.foreground),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = match proposal_status {
                        EditProposalStatus::Streaming => "Génération…",
                        EditProposalStatus::Ready => match mode {
                            EditApplyMode::ReplaceSelection => "Remplacer la sél.",
                            EditApplyMode::InsertAtCursor => "Insérer au curseur",
                            EditApplyMode::AppendEnd => "Ajouter à la fin",
                        },
                        EditProposalStatus::Accepted => "Validée",
                        EditProposalStatus::Rejected => "Ignorée",
                        EditProposalStatus::Conflict => "Conflit",
                        EditProposalStatus::Error => "Erreur",
                    };
                    ui.label(
                        RichText::new(status)
                            .size(FONT_UI_SM)
                            .color(pal.muted_foreground),
                    );
                });
            });
            if *conflict || matches!(proposal_status, EditProposalStatus::Conflict) {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Le passage a changé — régénérez ou ignorez.")
                        .size(FONT_UI_SM)
                        .color(egui::Color32::from_rgb(200, 120, 60)),
                );
            }
            ui.add_space(4.0);
            let body = if msg.content.is_empty() {
                "…"
            } else {
                msg.content.as_str()
            };
            egui::ScrollArea::vertical()
                .id_salt(format!("ak_edit_prop_{}", msg.id))
                .max_height(160.0)
                .show(ui, |ui| {
                    crate::chat_md::show(ui, body, pal.foreground);
                    if let Some(e) = error {
                        if !e.is_empty() {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(e)
                                    .size(FONT_UI_SM)
                                    .color(egui::Color32::from_rgb(200, 80, 80)),
                            );
                        }
                    }
                });

            let actionable = matches!(
                proposal_status,
                EditProposalStatus::Streaming
                    | EditProposalStatus::Ready
                    | EditProposalStatus::Conflict
                    | EditProposalStatus::Error
            );
            if actionable {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if matches!(proposal_status, EditProposalStatus::Streaming) {
                        if widgets::ghost(ui, "Arrêter").clicked() {
                            actions.push(SidebarAction::StopEditProposal(msg.id.clone()));
                        }
                    } else {
                        let can_accept = matches!(proposal_status, EditProposalStatus::Ready)
                            && !msg.content.trim().is_empty()
                            && !*conflict;
                        if can_accept && widgets::primary(ui, "Valider").clicked() {
                            actions.push(SidebarAction::AcceptEditProposal(msg.id.clone()));
                        }
                        if widgets::ghost(ui, "Régénérer").clicked() {
                            actions.push(SidebarAction::RegenerateEditProposal(msg.id.clone()));
                        }
                        if widgets::ghost(ui, "Ignorer").clicked() {
                            actions.push(SidebarAction::RejectEditProposal(msg.id.clone()));
                        }
                    }
                });
            }
        });
}

/// Immediate (pre-model) conversation title: the intent of the first user turn,
/// not its raw text.
pub fn intent_title(msg: &ChatMsg) -> String {
    let v = user_view(msg);
    let quote = clip_words(&plain_excerpt(&v.quote), 24);
    let note = first_sentence(&plain_excerpt(&v.note));
    let title = match (v.label.as_deref(), quote.is_empty(), note.is_empty()) {
        (Some("Chat") | None, false, true) => format!("À propos de « {quote} »"),
        (Some("Chat") | None, _, false) => note,
        (Some(label), false, _) => format!("{label} : « {quote} »"),
        (Some(label), true, false) => format!("{label} : {note}"),
        (Some(label), true, true) => label.to_string(),
        (None, true, true) => String::new(),
    };
    let t = clip_words(&title, 48);
    if t.is_empty() {
        "Conversation".into()
    } else {
        t
    }
}

/// Clean a model-produced title: one line, no quotes/markdown/trailing punctuation.
pub fn clean_generated_title(raw: &str) -> String {
    let line = raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let line = line
        .trim()
        .trim_start_matches(|c: char| c == '#' || c == '-' || c.is_whitespace())
        .trim_start_matches("Titre :")
        .trim_start_matches("Titre:");
    let plain = plain_excerpt(line);
    let t = plain.trim_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\'' | '«' | '»' | '“' | '”' | '.' | '!' | ':' | ';' | ','
            )
    });
    clip_words(t, 60)
}

fn first_sentence(text: &str) -> String {
    let t = text.trim();
    let end = t
        .char_indices()
        .find(|(_, c)| matches!(c, '.' | '?' | '!' | '\n'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(t.len());
    t[..end].trim().trim_end_matches('.').trim().to_string()
}

/// Cut at a word boundary to at most `max` chars, with an ellipsis when cut.
fn clip_words(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    let cut: String = t.chars().take(max).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > max / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end_matches([' ', ',', ';', ':']))
}

/// Display-only: drop markdown markers (quote/heading/list prefixes, emphasis, code ticks).
pub fn plain_excerpt(md: &str) -> String {
    let lines: Vec<String> = md
        .lines()
        .map(|line| {
            let cleaned = line
                .replace("**", "")
                .replace("__", "")
                .replace(['`', '*'], "");
            let mut l = cleaned.trim_start();
            loop {
                let next = l
                    .strip_prefix('>')
                    .or_else(|| l.strip_prefix('#'))
                    .or_else(|| l.strip_prefix("- "));
                match next {
                    Some(rest) => l = rest.trim_start(),
                    None => break,
                }
            }
            l.to_string()
        })
        .filter(|l| !l.trim().is_empty())
        .collect();
    lines.join(" ").trim().to_string()
}

fn user_quote_body(
    ui: &mut Ui,
    quote: &str,
    label: Option<&str>,
    tool: Option<&str>,
    note: &str,
    fg: egui::Color32,
) {
    let pal = Theme::current(ui.ctx()).palette;
    ui.spacing_mut().item_spacing.y = 6.0;
    if let Some(label) = label {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let _ = ui.add(
                shadcn_egui::components::icon::IconView::new(tool_icon(tool))
                    .size(14.0)
                    .color(fg),
            );
            ui.label(RichText::new(label).size(FONT_UI).strong().color(fg));
        });
    }
    let quote = plain_excerpt(quote);
    if quote.is_empty() {
        if !note.trim().is_empty() {
            crate::chat_md::show(ui, note, fg);
        }
        return;
    }
    let shown: String = quote.chars().take(280).collect();
    let shown = if quote.chars().count() > 280 {
        format!("{shown}…")
    } else {
        shown
    };
    let resp = egui::Frame::new()
        .fill(pal.background.gamma_multiply(0.35))
        .corner_radius(4.0)
        .inner_margin(egui::Margin {
            left: 10,
            right: 8,
            top: 4,
            bottom: 4,
        })
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(shown)
                        .size(FONT_UI_SM)
                        .italics()
                        .color(fg.gamma_multiply(0.8)),
                )
                .wrap()
                .selectable(true),
            );
        })
        .response;
    let bar = egui::Rect::from_min_size(resp.rect.min, egui::vec2(3.0, resp.rect.height()));
    ui.painter().rect_filled(bar, 1.5, pal.border);
    if !note.trim().is_empty() {
        crate::chat_md::show(ui, note, fg);
    }
}

/// What a user bubble shows: tool intent (icon + label), linked text, typed message.
#[derive(Debug, PartialEq)]
struct UserView {
    label: Option<String>,
    tool: Option<String>,
    quote: String,
    note: String,
}

fn user_view(msg: &ChatMsg) -> UserView {
    use crate::chat_store::MessagePayload;
    if let MessagePayload::UserQuote {
        quote,
        label,
        note,
        tool,
    } = &msg.payload
    {
        return UserView {
            label: label.clone(),
            tool: tool.clone(),
            quote: quote.clone(),
            note: note.clone(),
        };
    }
    legacy_user_view(&msg.content)
}

/// Older turns stored only the prompt text — recover intent / quote / note from it.
fn legacy_user_view(content: &str) -> UserView {
    let c = content.trim();
    let unquote = |s: &str| -> String {
        s.trim()
            .trim_start_matches('«')
            .trim_end_matches('»')
            .trim()
            .to_string()
    };
    if let Some(rest) = c
        .strip_prefix("Outil :")
        .or_else(|| c.strip_prefix("Brush :"))
    {
        let (label, quote) = rest.split_once("\n\n").unwrap_or((rest, ""));
        return UserView {
            label: Some(label.trim().to_string()),
            tool: None,
            quote: unquote(quote),
            note: String::new(),
        };
    }
    if let Some(rest) = c.strip_prefix("Contexte :") {
        let rest = rest.trim_start();
        let (quote, note) = match rest.find('»') {
            Some(i) => (&rest[..i + '»'.len_utf8()], &rest[i + '»'.len_utf8()..]),
            None => (rest, ""),
        };
        return UserView {
            label: Some("Chat".into()),
            tool: Some("chat".into()),
            quote: unquote(quote),
            note: note.trim().to_string(),
        };
    }
    for (prefix, mode) in [
        ("Continuer (phrase)", "sentence"),
        ("Continuer (paragraphe)", "paragraph"),
        ("Continuer", "continue"),
        ("Générer", "generate"),
    ] {
        if let Some(rest) = c.strip_prefix(prefix) {
            let rest = rest.trim_start();
            if rest.is_empty() || rest.starts_with(':') {
                return UserView {
                    label: Some(prefix.to_string()),
                    tool: Some(format!("continue:{mode}")),
                    quote: String::new(),
                    note: rest.trim_start_matches(':').trim().to_string(),
                };
            }
        }
    }
    UserView {
        label: None,
        tool: None,
        quote: String::new(),
        note: c.to_string(),
    }
}

/// Tool key (`brush:<id>` / `continue:<mode>`) of a proposal — same key user turns store.
fn regen_tool_key(regen: &crate::chat_store::EditRegenSpec) -> String {
    match regen {
        crate::chat_store::EditRegenSpec::Brush { brush_id } => format!("brush:{brush_id}"),
        crate::chat_store::EditRegenSpec::Continue { mode, .. } => format!("continue:{mode}"),
    }
}

fn proposal_tool_key(msg: &ChatMsg) -> Option<String> {
    match &msg.payload {
        crate::chat_store::MessagePayload::EditProposal { regen, .. } => {
            Some(regen_tool_key(regen))
        }
        _ => None,
    }
}

fn tool_icon(tool: Option<&str>) -> &'static shadcn_egui::Icon {
    match tool {
        Some("chat") => &lucide::MESSAGE_SQUARE,
        Some(t) if t.starts_with("brush:") => brush_icon(&t["brush:".len()..]),
        Some(t) if t.starts_with("continue:") => {
            composer_mode_icon(&ComposerMode::continue_mode(&t["continue:".len()..]))
        }
        _ => &lucide::WAND_SPARKLES,
    }
}

/// Every bubble / proposal card has this share of the message column — one width for all.
const BUBBLE_FRAC: f32 = 0.88;
/// Bubble frame horizontal margins (2 × 10) + stroke.
const BUBBLE_CHROME_X: f32 = 22.0;

fn bubble_content_width(row_w: f32) -> f32 {
    (row_w * BUBBLE_FRAC - BUBBLE_CHROME_X).max(100.0)
}

fn bubble_outer_width(row_w: f32) -> f32 {
    bubble_content_width(row_w) + BUBBLE_CHROME_X
}

/// Draws a bubble; `row_w` is the full message column width.
fn chat_bubble(
    ui: &mut Ui,
    msg: &ChatMsg,
    is_user: bool,
    row_w: f32,
    tool_hint: Option<String>,
    actions: &mut Vec<SidebarAction>,
) {
    let pal = Theme::current(ui.ctx()).palette;
    let dark = Theme::current(ui.ctx()).dark;
    let (fill, stroke, fg) = if is_user {
        (
            pal.secondary,
            egui::Stroke::new(1.0_f32, pal.border),
            pal.secondary_foreground,
        )
    } else {
        (
            crate::theme::ai_bubble_fill(dark),
            egui::Stroke::new(1.0_f32, pal.border),
            pal.foreground,
        )
    };
    let content_w = bubble_content_width(row_w);
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(egui::CornerRadius {
            nw: 12,
            ne: 12,
            sw: if is_user { 12 } else { 4 },
            se: if is_user { 4 } else { 12 },
        })
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.set_width(content_w);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                if !is_user {
                    let label = match msg.status {
                        MsgStatus::Pending => "AuthorKit · en attente…",
                        MsgStatus::Streaming => "AuthorKit · génération…",
                        MsgStatus::Cancelled => "AuthorKit · annulé",
                        MsgStatus::Error => "AuthorKit · erreur",
                        MsgStatus::Done => "AuthorKit",
                    };
                    ui.label(
                        RichText::new(label)
                            .size(FONT_UI_SM)
                            .strong()
                            .color(pal.muted_foreground),
                    );
                }
                if msg.content.is_empty()
                    && matches!(msg.status, MsgStatus::Pending | MsgStatus::Streaming)
                {
                    ui.label(RichText::new("…").size(FONT_UI).color(fg));
                } else if is_user {
                    let mut v = user_view(msg);
                    if v.tool.is_none() && v.label.is_some() {
                        v.tool = tool_hint;
                    }
                    user_quote_body(
                        ui,
                        &v.quote,
                        v.label.as_deref(),
                        v.tool.as_deref(),
                        &v.note,
                        fg,
                    );
                } else {
                    crate::chat_md::show(ui, &msg.content, fg);
                }

                if !is_user
                    && matches!(
                        msg.status,
                        MsgStatus::Done | MsgStatus::Cancelled | MsgStatus::Error
                    )
                {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        if widgets::icon_ghost_xs(ui, &lucide::COPY, "Copier").clicked() {
                            actions.push(SidebarAction::CopyMessage(msg.id.clone()));
                        }
                        if msg.status == MsgStatus::Error
                            && widgets::icon_ghost_xs(ui, &lucide::REFRESH_CCW, "Réessayer")
                                .clicked()
                        {
                            actions.push(SidebarAction::RetryMessage(msg.id.clone()));
                        }
                        if msg.status == MsgStatus::Done {
                            if widgets::icon_ghost_xs(ui, &lucide::REFRESH_CCW, "Régénérer")
                                .clicked()
                            {
                                actions.push(SidebarAction::RegenerateMessage(msg.id.clone()));
                            }
                            if msg.offers_insert {
                                if widgets::icon_ghost_xs(
                                    ui,
                                    &lucide::CORNER_DOWN_LEFT,
                                    "Insérer au curseur",
                                )
                                .clicked()
                                {
                                    actions.push(SidebarAction::InsertMessage(msg.content.clone()));
                                }
                                if widgets::icon_ghost_xs(
                                    ui,
                                    &lucide::REPLACE,
                                    "Remplacer la sélection",
                                )
                                .clicked()
                                {
                                    actions.push(SidebarAction::ReplaceWithMessage(
                                        msg.content.clone(),
                                    ));
                                }
                            }
                        }
                    });
                }
            });
        });
}

fn context_panel(ui: &mut Ui, project: &Project, scene: Option<&Scene>, state: &SidebarState) {
    let pal = Theme::current(ui.ctx()).palette;
    let Some(scene) = scene else {
        ui.label(
            RichText::new("Ouvrez une scène pour voir le contexte lié.")
                .size(FONT_UI)
                .color(pal.muted_foreground),
        );
        return;
    };
    use ak_llm::PieceKind;

    widgets::eyebrow(ui, "Ce que l'IA reçoit");
    ui.add_space(2.0);
    ui.label(
        RichText::new(format!(
            "Joint à chaque message du chat pour « {} ».",
            scene.meta.title
        ))
        .size(FONT_UI_SM)
        .color(pal.muted_foreground),
    );
    ui.add_space(8.0);
    budget_meter(ui, state.context_doc_chars, state.context_budget_chars);
    if state.context_omitted_history > 0 {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!(
                "{} ancien(s) message(s) du chat ne sont plus envoyés (trop long).",
                state.context_omitted_history
            ))
            .size(FONT_UI_SM)
            .color(pal.muted_foreground),
        );
    }
    ui.add_space(10.0);

    let of = |k: PieceKind| state.context_pieces.iter().filter(move |p| p.kind == k);

    ScrollArea::vertical()
        .id_salt("ak_context_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            context_section(ui, "Scène");
            for p in of(PieceKind::Scene) {
                context_row(
                    ui,
                    p,
                    &lucide::FILE_TEXT,
                    "Texte de la scène",
                    "La scène est vide.",
                );
            }
            for p in of(PieceKind::Passage) {
                context_row(
                    ui,
                    p,
                    &lucide::TEXT_QUOTE,
                    "Passage sélectionné",
                    "Sélectionnez du texte dans l'éditeur pour l'envoyer en priorité.",
                );
            }

            context_section(ui, "Fiches liées");
            let notes: Vec<_> = of(PieceKind::Note).collect();
            if notes.is_empty() {
                context_hint(
                    ui,
                    "Aucune fiche liée — utilisez « + Relier » sous le titre.",
                );
            }
            for p in notes {
                let note = project.notes.get(&p.id);
                let icon = note
                    .map(|n| widgets::note_kind_icon(n.meta.kind))
                    .unwrap_or(&lucide::BOOK);
                let name = note
                    .map(|n| n.meta.name.clone())
                    .unwrap_or_else(|| p.label.clone());
                context_row(ui, p, icon, &name, "Fiche vide.");
            }

            context_section(ui, "Histoire");
            for p in of(PieceKind::Premise) {
                context_row(
                    ui,
                    p,
                    &lucide::BOOK_OPEN,
                    "Prémisse",
                    "Pas de prémisse définie.",
                );
            }
            let summaries: Vec<_> = of(PieceKind::Summary).collect();
            let sent = summaries.iter().filter(|p| p.is_included()).count();
            if summaries.is_empty() {
                context_hint(ui, "Aucun synopsis d'autres scènes.");
            } else {
                context_group_row(
                    ui,
                    &lucide::LIST,
                    "Synopsis des autres scènes",
                    &format!("{sent} / {}", summaries.len()),
                    &summaries,
                );
            }

            context_section(ui, "Consignes");
            for p in of(PieceKind::Styles) {
                context_row(
                    ui,
                    p,
                    &lucide::PALETTE,
                    "Styles d'écriture",
                    "Aucun style défini (section Styles).",
                );
            }
        });
}

fn fmt_chars(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(c);
    }
    out
}

fn budget_meter(ui: &mut Ui, used: usize, max: usize) {
    let pal = Theme::current(ui.ctx()).palette;
    let frac = if max == 0 {
        0.0
    } else {
        (used as f32 / max as f32).clamp(0.0, 1.0)
    };
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 3.0, pal.foreground.gamma_multiply(0.10));
    let mut fill = rect;
    fill.set_width(rect.width() * frac);
    let color = if frac > 0.9 {
        egui::Color32::from_rgb(200, 120, 60)
    } else {
        pal.primary
    };
    ui.painter().rect_filled(fill, 3.0, color);
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "{} / {} caractères de contexte utilisés",
            fmt_chars(used),
            fmt_chars(max)
        ))
        .size(FONT_UI_SM)
        .color(pal.muted_foreground),
    );
}

fn context_section(ui: &mut Ui, title: &str) {
    ui.add_space(10.0);
    widgets::eyebrow(ui, title);
    ui.add_space(4.0);
}

fn context_hint(ui: &mut Ui, text: &str) {
    let pal = Theme::current(ui.ctx()).palette;
    ui.label(
        RichText::new(text)
            .size(FONT_UI_SM)
            .color(pal.muted_foreground),
    );
}

/// Status of a piece in plain words, plus whether it reached the model.
fn piece_state(p: &ak_llm::ContextPiece) -> (&'static str, bool) {
    match p.status {
        ak_llm::PieceStatus::Included => ("Envoyé", true),
        ak_llm::PieceStatus::Truncated { .. } => ("Raccourci", true),
        ak_llm::PieceStatus::Omitted(ak_llm::OmitReason::Empty) => ("Vide", false),
        ak_llm::PieceStatus::Omitted(ak_llm::OmitReason::Budget) => ("Pas la place", false),
    }
}

/// Clickable row: icon · name · status / size. Click shows what was sent.
fn context_row(
    ui: &mut Ui,
    p: &ak_llm::ContextPiece,
    icon: &shadcn_egui::Icon,
    name: &str,
    empty_hint: &str,
) {
    let (status, sent) = piece_state(p);
    let detail = if sent {
        format!("{status} · {} car.", fmt_chars(p.included_chars))
    } else {
        status.to_string()
    };
    let open_id = egui::Id::new(("ak_ctx_open", p.kind as u8, &p.id));
    let open = context_row_header(ui, open_id, icon, name, &detail, sent);
    if open {
        context_detail(ui, |ui| {
            if !sent {
                let why = if matches!(
                    p.status,
                    ak_llm::PieceStatus::Omitted(ak_llm::OmitReason::Budget)
                ) {
                    "Le contexte est plein : cet élément n'a pas été envoyé."
                } else {
                    empty_hint
                };
                context_hint(ui, why);
                return;
            }
            if let ak_llm::PieceStatus::Truncated { omitted_chars } = p.status {
                context_hint(
                    ui,
                    &format!(
                        "Trop long : {} caractères ont été retirés (début et fin conservés).",
                        fmt_chars(omitted_chars)
                    ),
                );
                ui.add_space(4.0);
            }
            context_preview(ui, &p.text);
        });
    }
}

/// One row for a group of pieces (e.g. synopses); opens the per-item list.
fn context_group_row(
    ui: &mut Ui,
    icon: &shadcn_egui::Icon,
    name: &str,
    detail: &str,
    items: &[&ak_llm::ContextPiece],
) {
    let pal = Theme::current(ui.ctx()).palette;
    let any_sent = items.iter().any(|p| p.is_included());
    let open_id = egui::Id::new(("ak_ctx_open_group", name));
    let open = context_row_header(
        ui,
        open_id,
        icon,
        name,
        &format!("{detail} envoyés"),
        any_sent,
    );
    if open {
        context_detail(ui, |ui| {
            for p in items {
                let (status, sent) = piece_state(p);
                ui.label(
                    RichText::new(format!("{} — {status}", p.label))
                        .size(FONT_UI_SM)
                        .color(if sent {
                            pal.foreground
                        } else {
                            pal.muted_foreground
                        }),
                );
            }
        });
    }
}

fn context_row_header(
    ui: &mut Ui,
    open_id: egui::Id,
    icon: &shadcn_egui::Icon,
    name: &str,
    detail: &str,
    sent: bool,
) -> bool {
    let pal = Theme::current(ui.ctx()).palette;
    let mut open: bool = ui.ctx().data(|d| d.get_temp(open_id)).unwrap_or(false);
    let h = 30.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 6.0, pal.foreground.gamma_multiply(0.06));
    }
    let fg = if sent {
        pal.foreground
    } else {
        pal.muted_foreground
    };
    let chevron = if open {
        &lucide::CHEVRON_DOWN
    } else {
        &lucide::CHEVRON_RIGHT
    };
    let mut x = rect.left() + 4.0;
    for (ic, color) in [(chevron, pal.muted_foreground), (icon, fg)] {
        let r = egui::Rect::from_center_size(
            egui::pos2(x + 7.0, rect.center().y),
            egui::Vec2::splat(14.0),
        );
        shadcn_egui::components::icon::paint_icon(ui.painter(), r, ic, color, 2.0);
        x += 20.0;
    }
    let detail_galley = ui.painter().layout_no_wrap(
        detail.to_string(),
        egui::FontId::proportional(FONT_UI_SM),
        pal.muted_foreground,
    );
    let detail_w = detail_galley.size().x;
    let name_max = (rect.right() - 8.0 - detail_w - 8.0 - x).max(20.0);
    let job = egui::text::LayoutJob {
        wrap: egui::text::TextWrapping {
            max_width: name_max,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..egui::text::LayoutJob::simple_singleline(
            name.to_string(),
            egui::FontId::proportional(FONT_UI),
            fg,
        )
    };
    let name_galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(
        egui::pos2(x, rect.center().y - name_galley.size().y / 2.0),
        name_galley,
        fg,
    );
    ui.painter().galley(
        egui::pos2(
            rect.right() - 8.0 - detail_w,
            rect.center().y - detail_galley.size().y / 2.0,
        ),
        detail_galley,
        pal.muted_foreground,
    );
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        open = !open;
        ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
    }
    open
}

fn context_detail(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.add_space(44.0);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 8.0);
            add(ui);
            ui.add_space(6.0);
        });
    });
}

fn context_preview(ui: &mut Ui, text: &str) {
    let pal = Theme::current(ui.ctx()).palette;
    let plain = plain_excerpt(text);
    let shown: String = plain.chars().take(600).collect();
    let shown = if plain.chars().count() > 600 {
        format!("{shown}…")
    } else {
        shown
    };
    if shown.trim().is_empty() {
        context_hint(ui, "Seulement des titres de section, sans contenu.");
        return;
    }
    let resp = egui::Frame::new()
        .fill(pal.foreground.gamma_multiply(0.05))
        .corner_radius(4.0)
        .inner_margin(egui::Margin {
            left: 10,
            right: 8,
            top: 6,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(shown)
                    .size(FONT_UI_SM)
                    .color(pal.foreground.gamma_multiply(0.85)),
            );
        })
        .response;
    let bar = egui::Rect::from_min_size(resp.rect.min, egui::vec2(3.0, resp.rect.height()));
    ui.painter().rect_filled(bar, 1.5, pal.border);
}

fn ideas_panel(ui: &mut Ui, state: &mut SidebarState, actions: &mut Vec<SidebarAction>) {
    if widgets::outline(ui, "Rafraîchir les idées").clicked() {
        actions.push(SidebarAction::RefreshIdeas);
    }
    ui.add_space(8.0);
    if state.ideas.is_empty() {
        ui.label(
            RichText::new("Suggestions pour la suite de votre draft.")
                .size(FONT_UI)
                .weak(),
        );
    }
    ScrollArea::vertical().show(ui, |ui| {
        for (i, idea) in state.ideas.iter().enumerate() {
            ui.label(RichText::new(&idea.title).size(FONT_UI).strong());
            ui.label(RichText::new(&idea.pitch).size(FONT_UI_SM).weak());
            if ui
                .add(
                    Button::new("Écrire pour moi")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .icon(&lucide::SPARKLES),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                actions.push(SidebarAction::WriteIdea(i));
            }
            ui.add_space(8.0);
        }
    });
}

fn reviews_panel(ui: &mut Ui, state: &mut SidebarState, actions: &mut Vec<SidebarAction>) {
    let pal = Theme::current(ui.ctx()).palette;
    if widgets::primary(ui, "Lancer une revue").clicked() {
        actions.push(SidebarAction::RunReview);
    }
    ui.add_space(8.0);
    let Some(report) = &mut state.review else {
        ui.label(
            RichText::new("Vérifie cohérence, voix et trous à partir du draft + Univers.")
                .size(FONT_UI)
                .weak(),
        );
        return;
    };
    if report.items.iter().any(|i| i.stale) {
        ui.label(
            RichText::new("Cette revue est ancienne — le texte a changé.")
                .size(FONT_UI_SM)
                .color(pal.destructive),
        );
        ui.add_space(4.0);
    }
    if report.items.is_empty() {
        ui.label(RichText::new("Aucune remarque.").size(FONT_UI).weak());
        return;
    }
    ScrollArea::vertical().show(ui, |ui| {
        for item in &mut report.items {
            let color = match item.severity.as_str() {
                "error" => pal.destructive,
                "warn" => pal.chart[1],
                _ => pal.chart[2],
            };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&item.title)
                        .size(FONT_UI)
                        .strong()
                        .color(color),
                );
                if item.stale {
                    ui.label(RichText::new("périmée").size(FONT_UI_SM).weak());
                }
            });
            ui.label(RichText::new(&item.detail).size(FONT_UI_SM));
            if !item.suggestion.is_empty() {
                ui.label(
                    RichText::new(format!("Suggestion : {}", item.suggestion))
                        .size(FONT_UI_SM)
                        .italics(),
                );
            }
            if let Some(p) = &item.passage {
                ui.label(
                    RichText::new(format!("« {} »", p.chars().take(80).collect::<String>()))
                        .size(FONT_UI_SM)
                        .weak(),
                );
            }
            ui.horizontal(|ui| {
                for (label, st) in [
                    ("À traiter", ak_coherence::ReviewStatus::ToDo),
                    ("Traitée", ak_coherence::ReviewStatus::Done),
                    ("Ignorée", ak_coherence::ReviewStatus::Ignored),
                ] {
                    if ui
                        .selectable_label(item.status == st, RichText::new(label).size(FONT_UI_SM))
                        .clicked()
                    {
                        item.status = st;
                        actions.push(SidebarAction::SetReviewStatus {
                            item_id: item.id.clone(),
                            status: st,
                        });
                    }
                }
            });
            ui.add_space(8.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        brush_icon, bubble_content_width, bubble_outer_width, clean_generated_title,
        composer_mode_icon, fmt_chars, intent_title, legacy_user_view, piece_state, plain_excerpt,
        tool_icon, user_view, ChatMsg, BUBBLE_CHROME_X,
    };
    use crate::chat_store::{ComposerMode, MessagePayload};
    use crate::widgets::lucide;
    use ak_llm::{ContextPiece, ContextRole, OmitReason, PieceKind, PieceStatus};

    fn piece(status: PieceStatus) -> ContextPiece {
        ContextPiece {
            kind: PieceKind::Scene,
            role: ContextRole::Documentary,
            id: "s1".into(),
            label: "Scène".into(),
            status,
            original_chars: 10,
            included_chars: 10,
            text: String::new(),
        }
    }

    #[test]
    fn intent_title_uses_action_not_raw_text() {
        let brush = ChatMsg::user("Brush : Améliorer\n\n« Ici tout commença »").with_payload(
            MessagePayload::UserQuote {
                quote: "**Ici** tout commença par une longue nuit d'hiver sans lune".into(),
                label: Some("Améliorer".into()),
                note: String::new(),
                tool: Some("brush:improve".into()),
            },
        );
        let t = intent_title(&brush);
        assert!(t.starts_with("Améliorer : « Ici tout commença"), "{t}");
        assert!(!t.contains("**"));

        let chat = ChatMsg::user("Peux-tu m'aider à rendre ce dialogue plus tendu ? Merci.");
        assert_eq!(
            intent_title(&chat),
            "Peux-tu m'aider à rendre ce dialogue plus tendu…"
        );
    }

    #[test]
    fn intent_title_empty_and_label_without_quote() {
        let empty = ChatMsg::user("");
        assert_eq!(intent_title(&empty), "Conversation");

        let labeled = ChatMsg::user("").with_payload(MessagePayload::UserQuote {
            quote: String::new(),
            label: Some("Améliorer".into()),
            note: String::new(),
            tool: Some("brush:improve".into()),
        });
        assert_eq!(intent_title(&labeled), "Améliorer");

        let labeled_note = ChatMsg::user("").with_payload(MessagePayload::UserQuote {
            quote: String::new(),
            label: Some("Continuer".into()),
            note: "Poursuis le dialogue.".into(),
            tool: Some("continue:continue".into()),
        });
        assert_eq!(
            intent_title(&labeled_note),
            "Continuer : Poursuis le dialogue"
        );
    }

    #[test]
    fn legacy_user_turns_recover_intent_quote_and_note() {
        let v = legacy_user_view("Brush : Grammaire\n\n« > Ici **tout** commença ... »");
        assert_eq!(v.label.as_deref(), Some("Grammaire"));
        assert_eq!(v.quote, "> Ici **tout** commença ...");
        assert!(v.note.is_empty());

        let v = legacy_user_view("Contexte :\n« Ici tout »\n\nC'est trop plat ?");
        assert_eq!(v.tool.as_deref(), Some("chat"));
        assert_eq!(v.quote, "Ici tout");
        assert_eq!(v.note, "C'est trop plat ?");

        let v = legacy_user_view("Générer : une scène de pluie");
        assert_eq!(v.tool.as_deref(), Some("continue:generate"));
        assert_eq!(v.note, "une scène de pluie");

        let v = legacy_user_view("Bonjour");
        assert_eq!(v.label, None);
        assert_eq!(v.note, "Bonjour");
    }

    #[test]
    fn clean_generated_title_strips_noise() {
        assert_eq!(
            clean_generated_title("Titre : « Tension du dialogue ».\nautre"),
            "Tension du dialogue"
        );
        assert_eq!(
            clean_generated_title("**Améliorer l'ouverture**"),
            "Améliorer l'ouverture"
        );
    }

    #[test]
    fn plain_excerpt_strips_quote_and_emphasis() {
        assert_eq!(
            plain_excerpt("**> **Ici** tout **commença** ...**"),
            "Ici tout commença ..."
        );
        assert_eq!(plain_excerpt("# Titre\n\n- item `code`"), "Titre item code");
    }

    #[test]
    fn fmt_chars_thin_space_thousands() {
        assert_eq!(fmt_chars(0), "0");
        assert_eq!(fmt_chars(999), "999");
        assert_eq!(fmt_chars(1000), format!("1{}000", '\u{202F}'));
        assert_eq!(
            fmt_chars(1_234_567),
            format!("1{}234{}567", '\u{202F}', '\u{202F}')
        );
    }

    #[test]
    fn piece_state_covers_statuses() {
        assert_eq!(piece_state(&piece(PieceStatus::Included)), ("Envoyé", true));
        assert_eq!(
            piece_state(&piece(PieceStatus::Truncated { omitted_chars: 12 })),
            ("Raccourci", true)
        );
        assert_eq!(
            piece_state(&piece(PieceStatus::Omitted(OmitReason::Empty))),
            ("Vide", false)
        );
        assert_eq!(
            piece_state(&piece(PieceStatus::Omitted(OmitReason::Budget))),
            ("Pas la place", false)
        );
    }

    #[test]
    fn bubble_widths_floor_and_chrome() {
        assert_eq!(BUBBLE_CHROME_X, 22.0);
        assert_eq!(bubble_content_width(0.0), 100.0);
        assert_eq!(bubble_outer_width(0.0), 122.0);
        let row = 500.0;
        let content = row * 0.88 - BUBBLE_CHROME_X;
        assert_eq!(bubble_content_width(row), content);
        assert_eq!(bubble_outer_width(row), content + BUBBLE_CHROME_X);
    }

    #[test]
    fn user_view_quote_vs_plain() {
        let quoted = ChatMsg::user("ignored").with_payload(MessagePayload::UserQuote {
            quote: "passage".into(),
            label: Some("Chat".into()),
            note: "note".into(),
            tool: Some("chat".into()),
        });
        let v = user_view(&quoted);
        assert_eq!(v.label.as_deref(), Some("Chat"));
        assert_eq!(v.quote, "passage");
        assert_eq!(v.note, "note");
        assert_eq!(v.tool.as_deref(), Some("chat"));

        let plain = ChatMsg::user("Bonjour le monde");
        let v = user_view(&plain);
        assert_eq!(v.label, None);
        assert!(v.quote.is_empty());
        assert_eq!(v.note, "Bonjour le monde");
    }

    #[test]
    fn tool_and_brush_icons_are_distinct() {
        // Icon consts are not address-stable; compare by value / name.
        assert_eq!(*tool_icon(Some("chat")), lucide::MESSAGE_SQUARE);
        assert_eq!(*brush_icon("improve"), lucide::SPARKLES);
        assert_eq!(*brush_icon("shorten"), lucide::SHRINK);
        assert_eq!(
            *composer_mode_icon(&ComposerMode::continue_mode("sentence")),
            lucide::PEN_LINE
        );
        assert_ne!(
            tool_icon(Some("brush:improve")).name,
            tool_icon(Some("continue:sentence")).name
        );
        assert_eq!(*tool_icon(None), lucide::WAND_SPARKLES);
    }
}
