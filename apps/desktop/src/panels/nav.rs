use ak_core::{NoteKind, Project};
use egui::{RichText, ScrollArea, Sense, Stroke, Ui};
use shadcn_egui::Theme;

use crate::i18n::Lang;
use crate::theme::{FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavSelection {
    Scene(String),
    Note(String),
    Styles,
    Publish,
}

#[derive(Debug, Clone)]
pub enum NavAction {
    Select(NavSelection),
    AddAct,
    AddChapter {
        act_id: String,
    },
    AddScene {
        act_id: String,
        chapter_id: String,
    },
    AddNote(NoteKind),
    RenameAct {
        id: String,
        title: String,
    },
    RenameChapter {
        act_id: String,
        id: String,
        title: String,
    },
    RenameScene {
        id: String,
        title: String,
    },
    RenameNote {
        id: String,
        name: String,
    },
    DeleteAct {
        id: String,
    },
    DeleteChapter {
        act_id: String,
        id: String,
    },
    DeleteScene {
        id: String,
    },
    DeleteNote {
        id: String,
    },
    ManuscriptHeight(f32),
    MoveSceneUp {
        id: String,
    },
    MoveSceneDown {
        id: String,
    },
    MoveChapterUp {
        act_id: String,
        id: String,
    },
    MoveChapterDown {
        act_id: String,
        id: String,
    },
    MoveActUp {
        id: String,
    },
    MoveActDown {
        id: String,
    },
    MoveSceneTo {
        id: String,
    },
    MoveChapterTo {
        act_id: String,
        id: String,
    },
    ReadChapter {
        act_id: String,
        id: String,
    },
}

pub struct NavOutcome {
    pub actions: Vec<NavAction>,
}

pub fn show(
    ui: &mut Ui,
    project: &Project,
    selection: &NavSelection,
    manuscript_height: f32,
    lang: Lang,
) -> NavOutcome {
    let mut out = NavOutcome {
        actions: Vec::new(),
    };
    let pal = Theme::current(ui.ctx()).palette;
    let avail = ui.available_height();
    let handle_h = 8.0;
    let styles_reserve = 44.0;
    let max_ms = (avail - handle_h - styles_reserve - 120.0).max(120.0);
    let mut ms_h = manuscript_height.clamp(120.0, max_ms);

    // —— Manuscript (fixed height, no corner resize) ——
    row_header(ui, lang.manuscript(), Some(lang.new_act()), || {
        out.actions.push(NavAction::AddAct);
    });
    ui.add_space(4.0);
    ScrollArea::vertical()
        .id_salt("nav_ms")
        .max_height(ms_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(ms_h - 4.0);
            if project.manuscript.index.acts.is_empty() {
                ui.label(
                    RichText::new(lang.no_acts())
                        .size(FONT_UI_SM)
                        .color(pal.muted_foreground),
                );
            }
            for act in &project.manuscript.index.acts {
                act_block(ui, project, act, selection, &mut out, lang);
            }
        });

    // —— Drag separator between Manuscrit and Univers ——
    let sep_w = ui.available_width();
    let (sep_rect, sep_resp) =
        ui.allocate_exact_size(egui::vec2(sep_w, handle_h), Sense::click_and_drag());
    let mid_y = sep_rect.center().y;
    ui.painter()
        .hline(sep_rect.x_range(), mid_y, Stroke::new(1.0_f32, pal.border));
    // Grab cue — short centered bar
    let cue_w = 28.0_f32;
    ui.painter().hline(
        (sep_rect.center().x - cue_w * 0.5)..=(sep_rect.center().x + cue_w * 0.5),
        mid_y,
        Stroke::new(2.5_f32, pal.muted_foreground.gamma_multiply(0.55)),
    );
    if sep_resp.hovered() || sep_resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if sep_resp.dragged() {
        ms_h = (ms_h + sep_resp.drag_delta().y).clamp(120.0, max_ms);
        out.actions.push(NavAction::ManuscriptHeight(ms_h));
    }

    // —— Universe fills the rest ——
    row_header(ui, lang.universe(), None, || {});
    ui.add_space(4.0);
    ScrollArea::vertical()
        .id_salt("nav_univers")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for kind in NoteKind::all() {
                note_group(ui, project, kind, selection, &mut out, lang);
            }
        });

    out
}

fn row_header(ui: &mut Ui, title: &str, add_tip: Option<&str>, mut on_add: impl FnMut()) {
    ui.horizontal(|ui| {
        widgets::eyebrow(ui, title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(tip) = add_tip {
                if widgets::icon_ghost_xs(ui, &lucide::PLUS, tip).clicked() {
                    on_add();
                }
            }
        });
    });
}

fn act_block(
    ui: &mut Ui,
    project: &Project,
    act: &ak_core::Act,
    selection: &NavSelection,
    out: &mut NavOutcome,
    lang: Lang,
) {
    let pal = Theme::current(ui.ctx()).palette;

    let _ = tree_row(
        ui,
        0,
        false,
        |ui| {
            ui.label(
                RichText::new(&act.title)
                    .size(FONT_UI)
                    .strong()
                    .color(pal.foreground),
            );
        },
        |ui| {
            if widgets::icon_ghost_xs(ui, &lucide::PLUS, lang.chapter()).clicked() {
                out.actions.push(NavAction::AddChapter {
                    act_id: act.id.clone(),
                });
            }
            if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_UP, "Monter").clicked() {
                out.actions
                    .push(NavAction::MoveActUp { id: act.id.clone() });
            }
            if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_DOWN, "Descendre").clicked() {
                out.actions
                    .push(NavAction::MoveActDown { id: act.id.clone() });
            }
            if widgets::icon_ghost_xs(ui, &lucide::PENCIL, lang.rename()).clicked() {
                out.actions.push(NavAction::RenameAct {
                    id: act.id.clone(),
                    title: act.title.clone(),
                });
            }
            if widgets::icon_ghost_xs(ui, &lucide::TRASH_2, lang.delete()).clicked() {
                out.actions
                    .push(NavAction::DeleteAct { id: act.id.clone() });
            }
        },
    );

    for ch in &act.chapters {
        let _ = tree_row(
            ui,
            1,
            false,
            |ui| {
                ui.label(
                    RichText::new(&ch.title)
                        .size(FONT_UI)
                        .color(pal.muted_foreground),
                );
            },
            |ui| {
                if widgets::icon_ghost_xs(ui, &lucide::BOOK_OPEN, "Lecture continue").clicked() {
                    out.actions.push(NavAction::ReadChapter {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_UP, "Monter").clicked() {
                    out.actions.push(NavAction::MoveChapterUp {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_DOWN, "Descendre").clicked() {
                    out.actions.push(NavAction::MoveChapterDown {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::FOLDER_OPEN, "Déplacer vers…").clicked() {
                    out.actions.push(NavAction::MoveChapterTo {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::PLUS, lang.scene()).clicked() {
                    out.actions.push(NavAction::AddScene {
                        act_id: act.id.clone(),
                        chapter_id: ch.id.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::PENCIL, lang.rename()).clicked() {
                    out.actions.push(NavAction::RenameChapter {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                        title: ch.title.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::TRASH_2, lang.delete()).clicked() {
                    out.actions.push(NavAction::DeleteChapter {
                        act_id: act.id.clone(),
                        id: ch.id.clone(),
                    });
                }
            },
        );

        for sid in &ch.scene_ids {
            let Some(scene) = project.manuscript.get_scene(sid) else {
                continue;
            };
            let selected = matches!(selection, NavSelection::Scene(id) if id == sid);
            let title = scene.meta.title.clone();
            let status = scene.meta.status;
            let sid = sid.clone();
            let dark = Theme::current(ui.ctx()).dark;
            let status_color = crate::theme::scene_status_color(status, dark);

            let tip = status.label_fr();
            let clicked = tree_row(
                ui,
                2,
                selected,
                |ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    widgets::icon_colored(
                        ui,
                        widgets::scene_status_icon(status),
                        12.0,
                        status_color,
                    )
                    .on_hover_text(tip);
                    ui.add(
                        egui::Label::new(RichText::new(&title).size(FONT_UI).color(if selected {
                            pal.accent_foreground
                        } else {
                            pal.foreground
                        }))
                        .selectable(false)
                        .sense(Sense::hover()),
                    )
                    .on_hover_text(tip);
                },
                |ui| {
                    if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_UP, "Monter").clicked() {
                        out.actions.push(NavAction::MoveSceneUp { id: sid.clone() });
                    }
                    if widgets::icon_ghost_xs(ui, &lucide::CHEVRON_DOWN, "Descendre").clicked() {
                        out.actions
                            .push(NavAction::MoveSceneDown { id: sid.clone() });
                    }
                    if widgets::icon_ghost_xs(ui, &lucide::FOLDER_OPEN, "Déplacer vers…").clicked()
                    {
                        out.actions.push(NavAction::MoveSceneTo { id: sid.clone() });
                    }
                    if widgets::icon_ghost_xs(ui, &lucide::PENCIL, lang.rename()).clicked() {
                        out.actions.push(NavAction::RenameScene {
                            id: sid.clone(),
                            title: title.clone(),
                        });
                    }
                    if widgets::icon_ghost_xs(ui, &lucide::TRASH_2, lang.delete()).clicked() {
                        out.actions.push(NavAction::DeleteScene { id: sid.clone() });
                    }
                },
            );
            if clicked {
                out.actions
                    .push(NavAction::Select(NavSelection::Scene(sid)));
            }
        }
    }
    ui.add_space(4.0);
}

fn note_group(
    ui: &mut Ui,
    project: &Project,
    kind: NoteKind,
    selection: &NavSelection,
    out: &mut NavOutcome,
    lang: Lang,
) {
    let pal = Theme::current(ui.ctx()).palette;
    let tip = lang.new_note_tip(kind);
    let kind_icon = widgets::note_kind_icon(kind);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        widgets::icon_colored(ui, kind_icon, 12.0, pal.muted_foreground);
        ui.label(
            RichText::new(lang.note_kind_plural(kind))
                .size(FONT_UI_SM)
                .color(pal.muted_foreground),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_ghost_xs(ui, &lucide::PLUS, &tip).clicked() {
                out.actions.push(NavAction::AddNote(kind));
            }
        });
    });

    for note in project.notes.by_kind(kind) {
        let selected = matches!(selection, NavSelection::Note(id) if id == &note.meta.id);
        let id = note.meta.id.clone();
        let name = note.meta.name.clone();
        let clicked = tree_row(
            ui,
            1,
            selected,
            |ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                widgets::icon_colored(
                    ui,
                    kind_icon,
                    12.0,
                    if selected {
                        pal.accent_foreground
                    } else {
                        pal.muted_foreground
                    },
                );
                ui.add(
                    egui::Label::new(RichText::new(&name).size(FONT_UI).color(if selected {
                        pal.accent_foreground
                    } else {
                        pal.foreground
                    }))
                    .selectable(false)
                    .sense(Sense::hover()),
                );
            },
            |ui| {
                if widgets::icon_ghost_xs(ui, &lucide::PENCIL, lang.rename()).clicked() {
                    out.actions.push(NavAction::RenameNote {
                        id: id.clone(),
                        name: name.clone(),
                    });
                }
                if widgets::icon_ghost_xs(ui, &lucide::TRASH_2, lang.delete()).clicked() {
                    out.actions.push(NavAction::DeleteNote { id: id.clone() });
                }
            },
        );
        if clicked {
            out.actions.push(NavAction::Select(NavSelection::Note(id)));
        }
    }
    ui.add_space(6.0);
}

/// Returns `true` when the content zone (full row minus action icons) was clicked.
fn tree_row(
    ui: &mut Ui,
    depth: usize,
    selected: bool,
    content: impl FnOnce(&mut Ui),
    actions: impl FnOnce(&mut Ui),
) -> bool {
    let indent = 12.0 * depth as f32;
    // IconXs is 20×20; chapters expose up to 6 actions.
    const ICON_W: f32 = 20.0;
    const MAX_ACTIONS: f32 = 6.0;
    let row_h = 28.0;
    let full_w = ui.available_width();
    let (row_rect, row_sense) =
        ui.allocate_exact_size(egui::vec2(full_w, row_h), egui::Sense::hover());
    let show_actions = row_sense.hovered() || row_sense.contains_pointer();
    let actions_w = if show_actions {
        ICON_W * MAX_ACTIONS
    } else {
        0.0
    };

    if selected {
        let fill = ui.visuals().selection.bg_fill;
        ui.painter().rect_filled(row_rect, 4.0, fill);
    }

    let mut content_clicked = false;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(row_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.set_clip_rect(row_rect);
            ui.add_space(indent);
            let text_w = (ui.available_width() - actions_w).max(40.0);
            let text_rect = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(text_w, row_h));
            // Whole content strip is selectable — actions sit outside this rect.
            let hit = ui.interact(
                text_rect,
                ui.id()
                    .with(("ak_nav_row_hit", depth, row_rect.min.y.to_bits())),
                Sense::click(),
            );
            content_clicked = hit.clicked();
            ui.allocate_ui_with_layout(
                egui::vec2(text_w, row_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_max_width(text_w);
                    ui.set_clip_rect(ui.max_rect());
                    // Labels must not offer text selection — row click selects the item.
                    ui.style_mut().interaction.selectable_labels = false;
                    content(ui);
                },
            );
            if show_actions {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    actions(ui);
                });
            }
            // Re-assert after labels so we keep pointer (not I-beam / default).
            if ui.rect_contains_pointer(text_rect) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        },
    );
    content_clicked
}
