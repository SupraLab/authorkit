//! Thin helpers around shadcn-egui components.

use std::hash::Hash;

use egui::{Color32, CornerRadius, Frame, Id, Margin, RichText, Shadow, Stroke, Ui};
use shadcn_egui::components::button::{Button, ButtonSize, ButtonVariant};
use shadcn_egui::components::dialog;
use shadcn_egui::components::icon::IconView;
use shadcn_egui::Icon;
use shadcn_egui::Theme;

use crate::theme::{FONT_TITLE, FONT_UI};

pub use shadcn_egui::icons::lucide;

/// Modal footer: cancel + action side-by-side on the right (action outermost).
/// Inside `add_actions`, add the primary/destructive button first, then cancel —
/// `dialog::footer` lays out right-to-left.
pub fn modal_footer(ui: &mut Ui, add_actions: impl FnOnce(&mut Ui)) {
    dialog::footer(ui, add_actions);
}

/// Fade id shared by [`FadeDialog`] and [`dialog_settled`].
fn fade_id(id: impl Hash) -> Id {
    Id::new(id).with("ak_fade")
}

/// True when the dialog is fully gone (logical close + exit fade finished).
pub fn dialog_settled(ctx: &egui::Context, id: impl Hash, open: bool) -> bool {
    !open && ctx.animate_bool_responsive(fade_id(id), false) <= 0.001
}

/// Dialog with fade-in **and** fade-out. Keep calling while `open` is false until
/// [`dialog_settled`] — clearing backing state early skips the exit animation.
pub struct FadeDialog<'a> {
    id: Id,
    title: &'a str,
    description: Option<&'a str>,
    width: f32,
    /// When set, dialog content keeps this height (stable across tab switches).
    min_height: Option<f32>,
    backdrop: Color32,
}

impl<'a> FadeDialog<'a> {
    pub fn new(id_salt: impl Hash, title: &'a str) -> Self {
        Self {
            id: Id::new(id_salt),
            title,
            description: None,
            width: 420.0,
            min_height: None,
            backdrop: Color32::from_black_alpha(140),
        }
    }

    pub fn description(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = Some(h);
        self
    }

    pub fn backdrop(mut self, color: Color32) -> Self {
        self.backdrop = color;
        self
    }

    /// Show while open or while the exit fade is running. Sets `*open = false` on dismiss.
    pub fn show<R>(
        self,
        ctx: &egui::Context,
        open: &mut bool,
        body: impl FnOnce(&mut Ui) -> R,
    ) -> Option<R> {
        let t = ctx.animate_bool_responsive(fade_id(self.id), *open);
        if t <= 0.001 {
            return None;
        }

        let pal = Theme::current(ctx).palette;
        let area = egui::Modal::default_area(self.id).fade_in(false);
        let frame = Frame::new()
            .fill(pal.card.gamma_multiply(t))
            .stroke(Stroke::new(1.0_f32, pal.border.gamma_multiply(t)))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(16))
            .shadow(Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha((48.0 * t) as u8),
            });

        let content_w = (self.width - 32.0).max(200.0);
        let mut close_btn = false;
        let resp = egui::Modal::new(self.id)
            .area(area)
            .backdrop_color(self.backdrop.gamma_multiply(t))
            .frame(frame)
            .show(ctx, |ui| {
                ui.multiply_opacity(t);
                // Ignore clicks while fading out so we don't re-trigger dismiss.
                if !*open {
                    ui.disable();
                }
                ui.set_width(content_w);
                if let Some(h) = self.min_height {
                    ui.set_min_height(h);
                    ui.set_max_height(h);
                    ui.set_height(h);
                }

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.title)
                            .size(FONT_TITLE)
                            .strong()
                            .color(pal.card_foreground),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_btn(ui, &lucide::X, "").clicked() {
                            close_btn = true;
                        }
                    });
                });
                if let Some(desc) = self.description {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(desc)
                            .size(FONT_UI)
                            .color(pal.muted_foreground),
                    );
                }
                ui.add_space(10.0);
                body(ui)
            });

        if *open && (close_btn || resp.should_close()) {
            *open = false;
        }
        Some(resp.inner)
    }
}

/// Force the pointer cursor on any clickable response (shadcn buttons, custom hits).
#[inline]
pub fn pointer(resp: egui::Response) -> egui::Response {
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn primary(ui: &mut Ui, label: &str) -> egui::Response {
    pointer(ui.add(Button::new(label)))
}

pub fn outline(ui: &mut Ui, label: &str) -> egui::Response {
    pointer(ui.add(Button::new(label).variant(ButtonVariant::Outline)))
}

pub fn ghost(ui: &mut Ui, label: &str) -> egui::Response {
    pointer(ui.add(Button::new(label).variant(ButtonVariant::Ghost)))
}

pub fn icon_btn(ui: &mut Ui, icon: &Icon, tip: &str) -> egui::Response {
    pointer(
        ui.add(
            Button::new("")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconSm)
                .icon(icon),
        )
        .on_hover_text(tip),
    )
}

pub fn icon_ghost_xs(ui: &mut Ui, icon: &Icon, tip: &str) -> egui::Response {
    pointer(
        ui.add(
            Button::new("")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::IconXs)
                .icon(icon),
        )
        .on_hover_text(tip),
    )
}

pub fn icon_outline(ui: &mut Ui, icon: &Icon, tip: &str) -> egui::Response {
    icon_toggle(ui, icon, tip, false)
}

/// Icon button — `active` uses Secondary so the format looks “on”.
pub fn icon_toggle(ui: &mut Ui, icon: &Icon, tip: &str, active: bool) -> egui::Response {
    pointer(
        ui.add(
            Button::new("")
                .variant(if active {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Ghost
                })
                .size(ButtonSize::IconSm)
                .icon(icon),
        )
        .on_hover_text(tip),
    )
}

/// Add a shadcn [`Button`] with pointer cursor.
pub fn button(ui: &mut Ui, btn: Button) -> egui::Response {
    pointer(ui.add(btn))
}

pub fn icon(ui: &mut Ui, icon: &Icon, size: f32) {
    let _ = ui.add(IconView::new(icon).size(size));
}

pub fn icon_colored(ui: &mut Ui, icon: &Icon, size: f32, color: Color32) -> egui::Response {
    ui.add(IconView::new(icon).size(size).color(color))
}

/// Lucide glyph for a scene writing status.
pub fn scene_status_icon(status: ak_core::SceneStatus) -> &'static Icon {
    use ak_core::SceneStatus::*;
    match status {
        ToWrite => &lucide::CIRCLE_DASHED,
        Draft => &lucide::PEN_LINE,
        ToRevise => &lucide::REFRESH_CW,
        Done => &lucide::CIRCLE_CHECK,
    }
}

/// Lucide glyph for note categories — shared by nav, link chips, and the Relier menu.
pub fn note_kind_icon(kind: ak_core::NoteKind) -> &'static Icon {
    use ak_core::NoteKind::*;
    match kind {
        Character => &lucide::USER,
        Setting => &lucide::MAP_PIN,
        Lore => &lucide::BOOK,
    }
}

/// Section eyebrow — muted, never loud primary.
pub fn eyebrow(ui: &mut Ui, text: &str) {
    let pal = Theme::current(ui.ctx()).palette;
    ui.label(
        RichText::new(text.to_uppercase())
            .size(crate::theme::FONT_UI_SM)
            .strong()
            .color(pal.muted_foreground),
    );
}

pub fn format_wrap(body: &mut String, prefix: &str, suffix: &str) {
    if body.is_empty() {
        body.push_str(&format!("{prefix}texte{suffix}"));
        return;
    }
    if !body.ends_with(['\n', ' ']) {
        body.push(' ');
    }
    body.push_str(&format!("{prefix}texte{suffix}"));
}

#[cfg(test)]
mod tests {
    use super::format_wrap;

    #[test]
    fn format_wrap_empty_vs_body() {
        let mut empty = String::new();
        format_wrap(&mut empty, "**", "**");
        assert_eq!(empty, "**texte**");

        let mut body = String::from("Hello");
        format_wrap(&mut body, "*", "*");
        assert_eq!(body, "Hello *texte*");

        let mut spaced = String::from("Hello ");
        format_wrap(&mut spaced, "_", "_");
        assert_eq!(spaced, "Hello _texte_");
    }
}
