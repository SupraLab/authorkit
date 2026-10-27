use egui::{RichText, Ui};
use shadcn_egui::components::button::{Button, ButtonSize, ButtonVariant};
use shadcn_egui::Theme;
use std::path::PathBuf;

use crate::brand::BrandAssets;
use crate::i18n::Lang;
use crate::theme::{FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

pub struct WelcomeAction {
    pub new_project: bool,
    pub open_project: bool,
    pub bootstrap: bool,
    pub open_recent: Option<PathBuf>,
}

pub fn show(ui: &mut Ui, brand: &BrandAssets, recents: &[String], lang: Lang) -> WelcomeAction {
    let mut action = WelcomeAction {
        new_project: false,
        open_project: false,
        bootstrap: false,
        open_recent: None,
    };
    let pal = Theme::current(ui.ctx()).palette;
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, pal.background);

    const FOOTER_H: f32 = 108.0;
    let footer =
        egui::Rect::from_min_max(egui::pos2(full.left(), full.bottom() - FOOTER_H), full.max);
    let content = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), footer.top()));

    // —— Main: hero + recents (scrollable) ——
    ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("ak_welcome_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.08).clamp(28.0, 80.0));
                    brand.show_hero(ui, lang);
                    ui.add_space(28.0);

                    let col_w = 420.0_f32.min(ui.available_width() - 32.0).max(280.0);
                    ui.set_max_width(col_w);

                    if !recents.is_empty() {
                        widgets::eyebrow(ui, lang.recents());
                        ui.add_space(8.0);
                        for path in recents.iter().take(6) {
                            let pb = PathBuf::from(path);
                            let name = pb
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or(path)
                                .to_string();
                            let parent = pb
                                .parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();

                            let resp = egui::Frame::new()
                                .fill(pal.card)
                                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                                .corner_radius(10.0)
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_width(col_w - 4.0);
                                    ui.horizontal(|ui| {
                                        widgets::icon(ui, &lucide::FOLDER_OPEN, 16.0);
                                        ui.add_space(8.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&name)
                                                    .size(FONT_UI)
                                                    .strong()
                                                    .color(pal.foreground),
                                            );
                                            if !parent.is_empty() {
                                                ui.label(
                                                    RichText::new(&parent)
                                                        .size(FONT_UI_SM)
                                                        .color(pal.muted_foreground),
                                                );
                                            }
                                        });
                                    });
                                })
                                .response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text(path)
                                .interact(egui::Sense::click());

                            if resp.clicked() {
                                action.open_recent = Some(pb);
                            }
                            ui.add_space(6.0);
                        }
                    } else {
                        ui.label(
                            RichText::new(lang.empty_hint())
                                .size(FONT_UI_SM)
                                .color(pal.muted_foreground),
                        );
                    }

                    ui.add_space(24.0);
                });
            });
    });

    // —— Fixed bottom: Commencer + 3 actions, centered ——
    ui.painter().hline(
        footer.x_range(),
        footer.top(),
        egui::Stroke::new(1.0_f32, pal.border),
    );
    ui.painter()
        .rect_filled(footer, 0.0, pal.card.gamma_multiply(0.55));

    ui.scope_builder(egui::UiBuilder::new().max_rect(footer), |ui| {
        ui.add_space(14.0);
        ui.vertical_centered(|ui| {
            widgets::eyebrow(ui, lang.get_started());
        });
        ui.add_space(10.0);

        // egui LTR layouts start from the left — center by padding with measured width.
        let row_id = ui.id().with("ak_welcome_actions_w");
        let known_w: f32 = ui.ctx().data(|d| d.get_temp(row_id)).unwrap_or(0.0);
        let avail = ui.available_width();
        ui.horizontal(|ui| {
            ui.set_min_height(44.0);
            if known_w > 0.0 {
                ui.add_space(((avail - known_w) * 0.5).max(0.0));
            }
            let group = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if ui
                    .add(
                        Button::new(lang.new_project())
                            .icon(&lucide::PLUS)
                            .size(ButtonSize::Lg),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    action.new_project = true;
                }
                if ui
                    .add(
                        Button::new(lang.open_project())
                            .variant(ButtonVariant::Outline)
                            .icon(&lucide::BOOK_OPEN)
                            .size(ButtonSize::Lg),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    action.open_project = true;
                }
                if ui
                    .add(
                        Button::new(lang.new_idea())
                            .variant(ButtonVariant::Secondary)
                            .icon(&lucide::SPARKLES)
                            .size(ButtonSize::Lg),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(lang.new_idea_tip())
                    .clicked()
                {
                    action.bootstrap = true;
                }
            });
            ui.ctx()
                .data_mut(|d| d.insert_temp(row_id, group.response.rect.width()));
        });
    });

    action
}
