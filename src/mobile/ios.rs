//! iOS-specific launch wiring (cargo-xcodebuild / eframe+winit).
//!
//! Kept separate so backend/bootstrap logic remains shared and modular.

#[cfg(target_os = "ios")]
use crate::mobile::MobileLaunchConfig;

#[cfg(target_os = "ios")]
pub fn launch_ios_ui() -> Result<(), eframe::Error> {
    let cfg = MobileLaunchConfig::default();
    let (backend, offline_fallback) = crate::mobile::build_mobile_backend(&cfg.backend);

    let mut app = crate::ui::PinpointAppUI::new(backend, None);
    app.set_offline_mode(cfg.offline_first || offline_fallback);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_title("Pinpoint"),
        ..Default::default()
    };
    eframe::run_native("Pinpoint", options, Box::new(|_cc| Ok(Box::new(app))))
}
