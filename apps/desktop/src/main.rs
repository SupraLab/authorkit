#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use authorkit_desktop::{brand, theme, AuthorKitApp, UiPrefs};

fn main() -> eframe::Result<()> {
    let prefs = UiPrefs::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1320.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_title("AuthorKit")
            .with_icon(brand::window_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "AuthorKit",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx, prefs.theme);
            Ok(Box::new(AuthorKitApp::new(&cc.egui_ctx)))
        }),
    )
}
