//! AuthorKit theming — IBM Plex only, zinc + accents logo (cyan → blue → violet).

use egui::{Color32, CornerRadius, Shadow};
use serde::{Deserialize, Serialize};
use shadcn_egui::fonts::files;
use shadcn_egui::{FontName, Palette, Style, Theme};
use std::path::PathBuf;

use crate::i18n::Lang;

/// Body / UI text size — one size everywhere unless explicitly overridden.
pub const FONT_UI: f32 = 13.0;
pub const FONT_UI_SM: f32 = 12.0;
pub const FONT_TITLE: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPrefs {
    pub theme: ThemeMode,
    #[serde(default = "default_true")]
    pub show_nav: bool,
    #[serde(default = "default_true")]
    pub show_sidebar: bool,
    #[serde(default)]
    pub nav_width: f32,
    #[serde(default)]
    pub sidebar_width: f32,
    /// Height of the Manuscrit block in the left nav (Univers fills the rest).
    #[serde(default)]
    pub manuscript_height: f32,
    #[serde(default)]
    pub onboarding_done: bool,
    #[serde(default)]
    pub recent_projects: Vec<String>,
    /// Session-only — always start in Visual; not restored from disk.
    #[serde(default, skip_serializing)]
    pub code_mode: bool,
    #[serde(default)]
    pub locale: Lang,
}

fn default_true() -> bool {
    true
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Light,
            show_nav: true,
            show_sidebar: true,
            nav_width: 280.0,
            sidebar_width: 340.0,
            manuscript_height: 320.0,
            onboarding_done: false,
            recent_projects: Vec::new(),
            code_mode: false,
            locale: Lang::Fr,
        }
    }
}

/// Root for UI prefs / secrets. Overridable via `AUTHORKIT_HOME` (tests).
pub fn authorkit_home() -> PathBuf {
    if let Ok(p) = std::env::var("AUTHORKIT_HOME") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".authorkit-rust")
}

impl UiPrefs {
    fn path() -> PathBuf {
        authorkit_home().join("ui.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(mut p) = serde_json::from_str::<UiPrefs>(&raw) {
                // Keep in sync with NAV_WIDTH_* / SIDEBAR_WIDTH_* in app.rs
                p.nav_width = p.nav_width.clamp(280.0, 420.0);
                if p.sidebar_width < 280.0 {
                    p.sidebar_width = 340.0;
                }
                if p.sidebar_width > 520.0 {
                    p.sidebar_width = 520.0;
                }
                if p.manuscript_height < 120.0 {
                    p.manuscript_height = 320.0;
                }
                // Always open in Visual unless the user switches this session.
                p.code_mode = false;
                return p;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, raw);
        }
    }

    pub fn push_recent(&mut self, path: &std::path::Path) {
        let s = path.display().to_string();
        self.recent_projects.retain(|p| p != &s);
        self.recent_projects.insert(0, s);
        self.recent_projects.truncate(8);
        self.save();
    }
}

/// Logo gradient stops — cyan nib → blue body → violet tip / spark.
const BRAND_CYAN_NEON: Color32 = Color32::from_rgb(34, 211, 238); // cyan-400 — dark neon
const BRAND_BLUE: Color32 = Color32::from_rgb(0, 112, 248); // logo mid
const BRAND_INDIGO: Color32 = Color32::from_rgb(79, 70, 229); // indigo-600 — light focus
const BRAND_VIOLET: Color32 = Color32::from_rgb(112, 48, 248); // logo tip
const BRAND_VIOLET_NEON: Color32 = Color32::from_rgb(192, 132, 252); // violet-400 — dark neon

pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    // Same font family for body AND headings — no Instrument Serif in chrome.
    let body = files(FontName::IbmPlexSans).unwrap_or_else(|| files(FontName::Inter).unwrap());
    let dark = matches!(mode, ThemeMode::Dark);

    let mut palette =
        Palette::from_upstream("zinc", "zinc", "neutral", dark).unwrap_or_else(|| {
            if dark {
                Palette::dark()
            } else {
                Palette::light()
            }
        });

    if dark {
        // Neon cyan / violet on deep zinc — luminous, not pastel.
        palette.ring = BRAND_CYAN_NEON;
        palette.accent = Color32::from_rgb(46, 16, 101); // violet-950 wash
        palette.accent_foreground = BRAND_CYAN_NEON;
    } else {
        // Paper + zinc; accents = saturated indigo/violet (not washed sky).
        palette.background = Color32::from_rgb(250, 250, 249);
        palette.card = Color32::from_rgb(255, 255, 255);
        palette.popover = Color32::from_rgb(255, 255, 255);
        palette.foreground = Color32::from_rgb(24, 24, 27);
        palette.card_foreground = palette.foreground;
        palette.popover_foreground = palette.foreground;
        palette.primary = Color32::from_rgb(39, 39, 42);
        palette.primary_foreground = Color32::from_rgb(250, 250, 250);
        palette.secondary = Color32::from_rgb(244, 244, 245);
        palette.secondary_foreground = Color32::from_rgb(39, 39, 42);
        palette.muted = Color32::from_rgb(244, 244, 245);
        palette.muted_foreground = Color32::from_rgb(113, 113, 122);
        palette.border = Color32::from_rgb(228, 228, 231);
        palette.input = Color32::from_rgb(228, 228, 231);
        palette.ring = BRAND_INDIGO;
        palette.accent = Color32::from_rgb(237, 233, 254); // violet-100
        palette.accent_foreground = Color32::from_rgb(91, 33, 182); // violet-800
    }

    let mut theme = Theme::new(Style::vega(), palette, 8.0).fonts(body, None);
    theme.dark = dark;
    theme.metrics.text_sm = FONT_UI;
    theme.metrics.text_xs = FONT_UI_SM;
    theme.metrics.text_base = FONT_UI;
    theme.metrics.text_lg = FONT_TITLE;
    theme.metrics.text_heading = FONT_TITLE;
    theme.apply(ctx);

    let soft_shadow = Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 40 } else { 16 }),
    };
    ctx.global_style_mut(|style| {
        if dark {
            // Neon selection — cyan wash + violet edge.
            style.visuals.selection.bg_fill = Color32::from_rgb(8, 145, 178).gamma_multiply(0.35);
            style.visuals.selection.stroke =
                egui::Stroke::new(1.0_f32, BRAND_VIOLET_NEON.gamma_multiply(0.7));
            style.visuals.hyperlink_color = BRAND_CYAN_NEON;
        } else {
            // Crisp indigo/violet selection — readable on white.
            style.visuals.selection.bg_fill = Color32::from_rgb(224, 231, 255); // indigo-100
            style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, BRAND_INDIGO);
            style.visuals.hyperlink_color = BRAND_BLUE;
        }
        // Buttons / clickable chrome → pointer; plain labels are not text-selectable.
        style.visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        style.interaction.selectable_labels = false;
        style.visuals.popup_shadow = soft_shadow;
        style.visuals.window_shadow = soft_shadow;
        style.visuals.menu_corner_radius = CornerRadius::same(8);
        style.spacing.menu_width = 280.0;
        style.spacing.combo_height = 320.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(FONT_UI));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(FONT_UI));
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional(FONT_UI_SM),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(FONT_TITLE),
        );
    });
}

/// Distinct accent per scene writing status (readable on light + dark).
pub fn scene_status_color(status: ak_core::SceneStatus, dark: bool) -> Color32 {
    use ak_core::SceneStatus::*;
    match (status, dark) {
        (ToWrite, true) => Color32::from_rgb(161, 161, 170), // zinc-400
        (ToWrite, false) => Color32::from_rgb(113, 113, 122), // zinc-500
        (Draft, true) => Color32::from_rgb(251, 191, 36),    // amber-400
        (Draft, false) => Color32::from_rgb(217, 119, 6),    // amber-600
        (ToRevise, true) => Color32::from_rgb(56, 189, 248), // sky-400
        (ToRevise, false) => Color32::from_rgb(2, 132, 199), // sky-600
        (Done, true) => Color32::from_rgb(52, 211, 153),     // emerald-400
        (Done, false) => Color32::from_rgb(5, 150, 105),     // emerald-600
    }
}

pub fn chip_fill(_pal: &Palette, dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(76, 29, 149).gamma_multiply(0.75) // violet-900
    } else {
        Color32::from_rgb(224, 231, 255) // indigo-100
    }
}

pub fn chip_fg(_pal: &Palette, dark: bool) -> Color32 {
    if dark {
        BRAND_VIOLET_NEON
    } else {
        Color32::from_rgb(67, 56, 202) // indigo-700
    }
}

/// Assistant / AI bubble — violet tip of the logo (neon in dark, soft in light).
pub fn ai_bubble_fill(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(46, 16, 101).gamma_multiply(0.7)
    } else {
        Color32::from_rgb(245, 243, 255) // violet-50
    }
}

/// Modal scrim — darker than the default shadcn overlay.
pub fn modal_backdrop(dark: bool) -> Color32 {
    if dark {
        Color32::from_black_alpha(200)
    } else {
        Color32::from_black_alpha(140)
    }
}

#[allow(dead_code)]
pub fn brand_violet() -> Color32 {
    BRAND_VIOLET
}
