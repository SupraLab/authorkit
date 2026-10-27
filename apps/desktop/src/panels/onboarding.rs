use ak_llm::{LlmSettings, ProviderKind};
use egui::RichText;
use shadcn_egui::components::button::{Button, ButtonVariant};
use shadcn_egui::components::input::Input;
use shadcn_egui::components::label::Label;
use shadcn_egui::Theme;

use crate::i18n::Lang;
use crate::providers::{self, ProviderPreset};
use crate::theme::{FONT_UI, FONT_UI_SM};
use crate::widgets::{self, lucide};

pub struct OnboardingState {
    pub open: bool,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub provider_id: String,
    pub error: String,
}

impl OnboardingState {
    pub fn for_first_launch(done: bool, llm: &LlmSettings) -> Self {
        let active = llm.active();
        let preset = providers::presets()[0];
        Self {
            open: !done,
            api_key: String::new(),
            model: active
                .map(|p| p.model.clone())
                .unwrap_or_else(|| preset.default_model.into()),
            base_url: active
                .map(|p| p.base_url.clone())
                .unwrap_or_else(|| preset.base_url.into()),
            provider_id: preset.id.into(),
            error: String::new(),
        }
    }
}

pub enum OnboardingAction {
    None,
    Saved,
    Skipped,
}

pub fn show(
    ctx: &egui::Context,
    state: &mut OnboardingState,
    llm: &mut LlmSettings,
    lang: Lang,
    icons: &crate::providers::ProviderIcons,
) -> OnboardingAction {
    let mut action = OnboardingAction::None;
    let mut open = state.open;
    let mut skip = false;
    let mut save_now = false;
    let pal = Theme::current(ctx).palette;
    let was_open = open;

    let dark = Theme::current(ctx).dark;
    widgets::FadeDialog::new("ak_onboard", "Bienvenue sur AuthorKit")
        .description(lang.tagline())
        .width(520.0)
        .backdrop(crate::theme::modal_backdrop(dark))
        .show(ctx, &mut open, |ui| {
            ui.label(
                RichText::new("Choisissez un fournisseur et un modèle (BYOK).")
                    .size(FONT_UI)
                    .color(pal.muted_foreground),
            );
            ui.add_space(10.0);

            ui.label(
                RichText::new("Fournisseur")
                    .size(FONT_UI_SM)
                    .color(pal.muted_foreground),
            );
            ui.add_space(4.0);
            let current = state.provider_id.clone();
            if let Some(preset) = providers::show_grid(ui, icons, |p| p.id == current) {
                apply_preset_to_state(state, preset);
            }

            ui.add_space(10.0);
            ui.label(
                RichText::new("Modèle")
                    .size(FONT_UI_SM)
                    .color(pal.muted_foreground),
            );
            ui.add_space(4.0);
            let provider = providers::presets()
                .into_iter()
                .find(|p| p.id == state.provider_id)
                .map(|p| p.provider)
                .unwrap_or(ProviderKind::OpenAiCompatible);
            let models = providers::models_for_url(provider, &state.base_url);
            providers::model_select(
                ui,
                "ak_onboard_model",
                &mut state.model,
                &models,
                "Choisir un modèle…",
            );

            if provider != ProviderKind::Ollama {
                ui.add_space(8.0);
                let _ = ui.add(Label::new("Clé API"));
                let _ = ui.add(
                    Input::new(&mut state.api_key)
                        .password(true)
                        .hint("sk-…")
                        .max_width(f32::INFINITY),
                );
            }

            if !state.error.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(&state.error)
                        .size(FONT_UI_SM)
                        .color(pal.destructive),
                );
            }

            ui.add_space(14.0);
            crate::widgets::modal_footer(ui, |ui| {
                if ui
                    .add(Button::new("Enregistrer").icon(&lucide::CHECK))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    save_now = true;
                }
                if ui
                    .add(Button::new("Plus tard").variant(ButtonVariant::Ghost))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    skip = true;
                }
            });
        });

    if save_now {
        if let Some(preset) = providers::presets()
            .into_iter()
            .find(|p| p.id == state.provider_id)
        {
            providers::apply_preset(llm, preset);
            if let Some(p) = llm.profiles.iter_mut().find(|p| p.name == preset.label) {
                p.model = state.model.clone();
                p.base_url = state.base_url.clone();
            }
            llm.normalize_keyring_accounts();
            if preset.provider != ProviderKind::Ollama {
                if state.api_key.trim().is_empty() {
                    state.error = "Ajoutez une clé API.".into();
                    return OnboardingAction::None;
                }
                if let Some(p) = llm.active().cloned() {
                    if let Err(e) = llm.set_api_key(&p, &state.api_key) {
                        state.error = format!("Clé non enregistrée: {e}");
                        return OnboardingAction::None;
                    }
                }
            }
            if let Err(e) = llm.save() {
                state.error = e.to_string();
                return OnboardingAction::None;
            }
            open = false;
            action = OnboardingAction::Saved;
        }
    } else if skip || (was_open && !open) {
        open = false;
        action = OnboardingAction::Skipped;
    }

    state.open = open;
    action
}

fn apply_preset_to_state(state: &mut OnboardingState, preset: ProviderPreset) {
    state.provider_id = preset.id.into();
    state.model = preset.default_model.into();
    state.base_url = preset.base_url.into();
    state.error.clear();
}
