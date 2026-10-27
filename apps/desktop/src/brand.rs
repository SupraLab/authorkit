//! AuthorKit brand assets (icon mark + lockup).

use eframe::egui::{self, Color32, ColorImage, RichText, TextureHandle, TextureOptions};
use shadcn_egui::Theme;

use crate::i18n::Lang;
use crate::theme::FONT_TITLE;

/// Window / Dock icon (from macOS `AuthorKit.icns`) — not the in-app mark.
const ICON_PNG: &[u8] = include_bytes!("../assets/brand/icon-512.png");
/// In-UI quill mark (gradient cutout) — separate from the mac app icon.
const MARK_PNG: &[u8] = include_bytes!("../assets/brand/mark-256.png");

pub fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(ICON_PNG).unwrap_or_default()
}

pub struct BrandAssets {
    pub mark: TextureHandle,
}

impl BrandAssets {
    pub fn load(ctx: &egui::Context) -> Self {
        Self {
            mark: texture_from_png(ctx, "ak_mark", MARK_PNG),
        }
    }

    /// Compact mark — transparent cutout, no background fill.
    pub fn show_mark(&self, ui: &mut egui::Ui, size: f32) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
        ui.painter().image(
            self.mark.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    /// Mark + « AuthorKit » bold, vertically centered as one lockup.
    pub fn show_wordmark(&self, ui: &mut egui::Ui, mark_size: f32, name_size: f32) {
        let pal = Theme::current(ui.ctx()).palette;
        let gap = (mark_size * 0.22).clamp(5.0, 10.0);
        ui.horizontal(|ui| {
            ui.set_height(mark_size);
            ui.spacing_mut().item_spacing.x = gap;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                self.show_mark(ui, mark_size);
                ui.label(
                    RichText::new("AuthorKit")
                        .size(name_size)
                        .strong()
                        .extra_letter_spacing(0.35)
                        .color(pal.foreground),
                );
            });
        });
    }

    /// Welcome hero: large mark + bold name + tagline.
    pub fn show_hero(&self, ui: &mut egui::Ui, lang: Lang) {
        let pal = Theme::current(ui.ctx()).palette;
        ui.vertical_centered(|ui| {
            self.show_mark(ui, 72.0);
            ui.add_space(12.0);
            ui.label(
                RichText::new("AuthorKit")
                    .size(FONT_TITLE + 12.0)
                    .strong()
                    .color(pal.foreground),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(lang.tagline())
                    .size(crate::theme::FONT_UI)
                    .color(pal.muted_foreground),
            );
        });
    }
}

fn texture_from_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let icon = eframe::icon_data::from_png_bytes(bytes).unwrap_or_default();
    let mut rgba = icon.rgba;
    // Premultiply-safe: zero RGB when fully transparent (avoids black/white halo)
    for px in rgba.as_chunks_mut::<4>().0 {
        if px[3] == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        } else if px[3] < 255 {
            let a = px[3] as u16;
            px[0] = ((px[0] as u16 * a) / 255) as u8;
            px[1] = ((px[1] as u16 * a) / 255) as u8;
            px[2] = ((px[2] as u16 * a) / 255) as u8;
        }
    }
    let image =
        ColorImage::from_rgba_premultiplied([icon.width as usize, icon.height as usize], &rgba);
    ctx.load_texture(
        name,
        image,
        TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        },
    )
}
