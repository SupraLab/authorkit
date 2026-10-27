//! Shared LLM provider presets + modern model catalogs (settings + onboarding).

use std::collections::HashMap;

use ak_llm::{LlmProfile, LlmSettings, ProviderKind};
use eframe::egui::{
    self, Color32, ColorImage, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, Vec2,
};
use shadcn_egui::Theme;

use crate::theme::{FONT_UI, FONT_UI_SM};

const CARD_HEIGHT: f32 = 56.0;

const LOGO_OPENAI: &[u8] = include_bytes!("../assets/providers/openai.png");
const LOGO_ANTHROPIC: &[u8] = include_bytes!("../assets/providers/anthropic.png");
const LOGO_GEMINI: &[u8] = include_bytes!("../assets/providers/gemini.png");
const LOGO_MISTRAL: &[u8] = include_bytes!("../assets/providers/mistral.png");
const LOGO_DEEPSEEK: &[u8] = include_bytes!("../assets/providers/deepseek.png");
const LOGO_XAI: &[u8] = include_bytes!("../assets/providers/xai.png");
const LOGO_OPENROUTER: &[u8] = include_bytes!("../assets/providers/openrouter.png");
const LOGO_GROQ: &[u8] = include_bytes!("../assets/providers/groq.png");
const LOGO_OLLAMA: &[u8] = include_bytes!("../assets/providers/ollama.png");

/// GPU textures for provider brand marks (loaded once at startup).
pub struct ProviderIcons {
    logos: HashMap<&'static str, TextureHandle>,
}

impl ProviderIcons {
    pub fn load(ctx: &egui::Context) -> Self {
        let mut logos = HashMap::new();
        for (id, bytes) in [
            ("openai", LOGO_OPENAI),
            ("anthropic", LOGO_ANTHROPIC),
            ("gemini", LOGO_GEMINI),
            ("mistral", LOGO_MISTRAL),
            ("deepseek", LOGO_DEEPSEEK),
            ("xai", LOGO_XAI),
            ("openrouter", LOGO_OPENROUTER),
            ("groq", LOGO_GROQ),
            ("ollama", LOGO_OLLAMA),
        ] {
            logos.insert(
                id,
                texture_from_png(ctx, &format!("ak_provider_{id}"), bytes),
            );
        }
        Self { logos }
    }

    fn get(&self, id: &str) -> Option<&TextureHandle> {
        self.logos.get(id)
    }
}

#[derive(Clone, Copy)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub provider: ProviderKind,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

pub fn presets() -> [ProviderPreset; 9] {
    [
        ProviderPreset {
            id: "openai",
            label: "OpenAI",
            hint: "GPT-5 · API",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1",
            default_model: "gpt-5-mini",
        },
        ProviderPreset {
            id: "anthropic",
            label: "Anthropic",
            hint: "Claude 4.5",
            provider: ProviderKind::Anthropic,
            base_url: "https://api.anthropic.com/v1",
            default_model: "claude-sonnet-4-5",
        },
        ProviderPreset {
            id: "gemini",
            label: "Google",
            hint: "Gemini · API",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
            default_model: "gemini-2.5-flash",
        },
        ProviderPreset {
            id: "mistral",
            label: "Mistral",
            hint: "EU · open-weight",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.mistral.ai/v1",
            default_model: "mistral-small-latest",
        },
        ProviderPreset {
            id: "deepseek",
            label: "DeepSeek",
            hint: "Chat · Reasoner",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.deepseek.com/v1",
            default_model: "deepseek-chat",
        },
        ProviderPreset {
            id: "xai",
            label: "xAI",
            hint: "Grok",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.x.ai/v1",
            default_model: "grok-3-mini",
        },
        ProviderPreset {
            id: "openrouter",
            label: "OpenRouter",
            hint: "Multi-modèles",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://openrouter.ai/api/v1",
            default_model: "openai/gpt-5-mini",
        },
        ProviderPreset {
            id: "groq",
            label: "Groq",
            hint: "Rapide · local-like",
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.groq.com/openai/v1",
            default_model: "llama-3.3-70b-versatile",
        },
        ProviderPreset {
            id: "ollama",
            label: "Ollama",
            hint: "Local · sans clé",
            provider: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1",
            default_model: "llama3.2",
        },
    ]
}

/// Provider tiles in 3 equal columns spanning the available width. Returns the clicked preset.
pub fn show_grid(
    ui: &mut Ui,
    icons: &ProviderIcons,
    is_active: impl Fn(&ProviderPreset) -> bool,
) -> Option<ProviderPreset> {
    const COLS: usize = 3;
    const GAP: f32 = 8.0;
    let card_w = ((ui.available_width() - GAP * (COLS - 1) as f32) / COLS as f32).floor();
    let mut picked = None;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(GAP);
        for row in presets().chunks(COLS) {
            ui.horizontal(|ui| {
                for preset in row {
                    if show_card(ui, icons, *preset, is_active(preset), card_w).clicked() {
                        picked = Some(*preset);
                    }
                }
            });
        }
    });
    picked
}

/// Provider card — width set by the grid, fixed height so rows align.
fn show_card(
    ui: &mut Ui,
    icons: &ProviderIcons,
    preset: ProviderPreset,
    active: bool,
    width: f32,
) -> egui::Response {
    let pal = Theme::current(ui.ctx()).palette;
    let dark = Theme::current(ui.ctx()).dark;
    let fill = if active {
        pal.accent
    } else {
        pal.muted.gamma_multiply(0.45)
    };
    let stroke = if active {
        Stroke::new(1.5_f32, pal.ring)
    } else {
        Stroke::new(1.0_f32, pal.border)
    };

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CARD_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.painter()
        .rect(rect, 10.0, fill, stroke, StrokeKind::Inside);

    let inner = rect.shrink2(Vec2::new(10.0, 8.0));
    let logo_sz = 26.0_f32;
    let logo_rect = egui::Rect::from_min_size(
        inner.min + Vec2::new(0.0, (inner.height() - logo_sz) * 0.5),
        Vec2::splat(logo_sz),
    );
    paint_logo(ui, icons, preset.id, logo_rect, dark, active);

    let text_x = logo_rect.max.x + 8.0;
    let text_w = (inner.max.x - text_x).max(0.0);
    for (text, font, color, dy) in [
        (preset.label, FONT_UI, pal.foreground, 4.0),
        (preset.hint, FONT_UI_SM, pal.muted_foreground, 22.0),
    ] {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.to_owned(),
            egui::FontId::proportional(font),
            color,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(text_w);
        let galley = ui.painter().layout_job(job);
        ui.painter()
            .galley(egui::pos2(text_x, inner.min.y + dy), galley, color);
    }

    response.on_hover_text(format!("{} — {}", preset.label, preset.hint))
}

fn brand_tint(id: &str, dark: bool) -> Color32 {
    // Slightly brighter brands in dark so marks read on muted cards.
    match (id, dark) {
        ("openai", true) => Color32::from_rgb(52, 211, 153),
        ("openai", false) => Color32::from_rgb(16, 163, 127),
        ("anthropic", true) => Color32::from_rgb(232, 184, 148),
        ("anthropic", false) => Color32::from_rgb(191, 122, 88),
        ("gemini", true) => Color32::from_rgb(96, 165, 250),
        ("gemini", false) => Color32::from_rgb(66, 133, 244),
        ("mistral", true) => Color32::from_rgb(255, 138, 96),
        ("mistral", false) => Color32::from_rgb(234, 88, 36),
        ("deepseek", true) => Color32::from_rgb(129, 140, 248),
        ("deepseek", false) => Color32::from_rgb(67, 91, 232),
        ("xai", true) => Color32::from_rgb(244, 244, 245),
        ("xai", false) => Color32::from_rgb(39, 39, 42),
        ("openrouter", true) => Color32::from_rgb(147, 164, 255),
        ("openrouter", false) => Color32::from_rgb(79, 99, 230),
        ("groq", true) => Color32::from_rgb(251, 191, 36),
        ("groq", false) => Color32::from_rgb(217, 119, 6),
        ("ollama", true) => Color32::from_rgb(212, 212, 216),
        ("ollama", false) => Color32::from_rgb(82, 82, 91),
        (_, true) => Color32::from_rgb(212, 212, 216),
        (_, false) => Color32::from_rgb(113, 113, 122),
    }
}

fn paint_logo(
    ui: &mut Ui,
    icons: &ProviderIcons,
    id: &str,
    rect: egui::Rect,
    dark: bool,
    active: bool,
) {
    let tint = brand_tint(id, dark);
    // Badge: light wash in light mode, stronger brand wash in dark so the mark pops.
    let bg = if active {
        tint.gamma_multiply(if dark { 0.38 } else { 0.2 })
    } else if dark {
        tint.gamma_multiply(0.32)
    } else {
        tint.gamma_multiply(0.14)
    };
    ui.painter().rect_filled(rect, 6.0, bg);

    if let Some(tex) = icons.get(id) {
        let pad = 4.0_f32;
        let img = rect.shrink(pad);
        // Texture is white-on-transparent; multiply by brand tint.
        ui.painter().image(
            tex.id(),
            img,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(FONT_UI_SM),
            tint,
        );
    }
}

fn texture_from_png(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let icon = eframe::icon_data::from_png_bytes(bytes).unwrap_or_default();
    let mut rgba = icon.rgba;
    // Lobe icons rasterize as black `currentColor`. egui tint multiplies RGB, so
    // black stays black — invert ink to white coverage (premultiplied alpha).
    for px in rgba.as_chunks_mut::<4>().0 {
        let a = px[3] as u16;
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        let lum = (px[0] as u16 + px[1] as u16 + px[2] as u16) / 3;
        let coverage = (((255 - lum) * a) / 255) as u8;
        px[0] = coverage;
        px[1] = coverage;
        px[2] = coverage;
        px[3] = coverage;
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

pub fn models_for(llm: &LlmSettings) -> Vec<&'static str> {
    let Some(p) = llm.active() else {
        return Vec::new();
    };
    models_for_url(p.provider, &p.base_url)
}

pub fn models_for_url(provider: ProviderKind, base_url: &str) -> Vec<&'static str> {
    let url = base_url.to_ascii_lowercase();
    match provider {
        ProviderKind::OpenAiCompatible if url.contains("openrouter") => vec![
            "openai/gpt-5-mini",
            "openai/gpt-5",
            "anthropic/claude-sonnet-4.5",
            "google/gemini-2.5-flash",
            "deepseek/deepseek-chat",
        ],
        ProviderKind::OpenAiCompatible if url.contains("groq") => vec![
            "llama-3.3-70b-versatile",
            "llama-3.1-8b-instant",
            "mixtral-8x7b-32768",
        ],
        ProviderKind::OpenAiCompatible if url.contains("generativelanguage.googleapis") => {
            vec!["gemini-2.5-flash", "gemini-2.5-pro", "gemini-2.0-flash"]
        }
        ProviderKind::OpenAiCompatible if url.contains("mistral.ai") => vec![
            "mistral-small-latest",
            "mistral-medium-latest",
            "mistral-large-latest",
            "codestral-latest",
        ],
        ProviderKind::OpenAiCompatible if url.contains("deepseek.com") => {
            vec!["deepseek-chat", "deepseek-reasoner"]
        }
        ProviderKind::OpenAiCompatible if url.contains("x.ai") => {
            vec!["grok-3-mini", "grok-3", "grok-2"]
        }
        ProviderKind::OpenAiCompatible => {
            vec!["gpt-5-mini", "gpt-5", "gpt-4.1-mini", "gpt-4.1", "o4-mini"]
        }
        ProviderKind::Anthropic => {
            vec!["claude-sonnet-4-5", "claude-opus-4-5", "claude-haiku-4-5"]
        }
        ProviderKind::Ollama => vec!["llama3.2", "mistral", "qwen2.5", "gemma3"],
    }
}

/// Dropdown bound to `current`. Curated list for the active provider; picking updates `current`.
pub fn model_select(
    ui: &mut Ui,
    id: &str,
    current: &mut String,
    models: &[&str],
    placeholder: &str,
) {
    use shadcn_egui::components::select::Select;

    if models.is_empty() {
        let _ =
            ui.add(shadcn_egui::components::input::Input::new(current).max_width(f32::INFINITY));
        return;
    }

    let mut idx = models
        .iter()
        .position(|m| *m == current.as_str())
        .unwrap_or(models.len());
    let shown = if current.is_empty() {
        placeholder.to_string()
    } else {
        current.clone()
    };
    let w = ui.available_width();
    let resp = ui.add(
        Select::new(id, &mut idx, models)
            .width(w)
            .placeholder(shown),
    );
    if resp.changed() && idx < models.len() {
        *current = models[idx].to_string();
    }
}

pub fn apply_preset(llm: &mut LlmSettings, preset: ProviderPreset) {
    let name = preset.label.to_string();
    if let Some(existing) = llm.profiles.iter_mut().find(|p| p.name == name) {
        existing.provider = preset.provider;
        existing.base_url = preset.base_url.into();
        existing.model = preset.default_model.into();
    } else {
        llm.profiles.push(LlmProfile {
            name: name.clone(),
            provider: preset.provider,
            base_url: preset.base_url.into(),
            model: preset.default_model.into(),
            keyring_account: String::new(),
        });
    }
    llm.active_profile = name;
    // Stable credential id from endpoint (shared across duplicate profiles).
    llm.normalize_keyring_accounts();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_for_url_branches() {
        let openrouter = models_for_url(
            ProviderKind::OpenAiCompatible,
            "https://openrouter.ai/api/v1",
        );
        assert!(openrouter.contains(&"openai/gpt-5-mini"));

        let groq = models_for_url(
            ProviderKind::OpenAiCompatible,
            "https://api.groq.com/openai",
        );
        assert!(groq.contains(&"llama-3.3-70b-versatile"));

        let anthropic = models_for_url(ProviderKind::Anthropic, "https://api.anthropic.com");
        assert!(anthropic.contains(&"claude-sonnet-4-5"));

        let ollama = models_for_url(ProviderKind::Ollama, "http://localhost:11434");
        assert!(ollama.contains(&"llama3.2"));
    }
}
