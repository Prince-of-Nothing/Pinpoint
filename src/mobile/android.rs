//! Android-specific launch wiring (cargo-apk / eframe+winit).
//!
//! Kept separate so backend/bootstrap logic remains shared and modular.
//! eframe/winit Android integration depends on the selected activity backend
//! and crate features; this module provides the Pinpoint-side launch path.

#[cfg(target_os = "android")]
use crate::mobile::MobileLaunchConfig;

#[cfg(target_os = "android")]
pub fn launch_android_ui(
    app_handle: winit::platform::android::activity::AndroidApp,
) -> Result<(), eframe::Error> {
    let cfg = MobileLaunchConfig::default();
    let (backend, offline_fallback) = crate::mobile::build_mobile_backend(&cfg.backend);

    let mut app = crate::ui::PinpointAppUI::new(backend, None);
    app.set_offline_mode(cfg.offline_first || offline_fallback);

    match cfg.app_kind {
        crate::webapp::WebAppKind::Kiosk => {
            app.set_screen(crate::app::CurrentScreen::Kiosk);
        }
        crate::webapp::WebAppKind::Admin => {
            app.set_screen(crate::app::CurrentScreen::Admin);
        }
        crate::webapp::WebAppKind::Web => {}
    }

    let options = eframe::NativeOptions {
        android_app: Some(app_handle),
        viewport: eframe::egui::ViewportBuilder::default().with_title("Pinpoint"),
        ..Default::default()
    };
    eframe::run_native("Pinpoint", options, Box::new(|_cc| Ok(Box::new(app))))
}

/// Native entry point expected by `android-activity` / `cargo-apk` in the
/// cdylib. The eframe `NativeOptions::android_app` field receives the handle.
#[cfg(target_os = "android")]
#[no_mangle]
#[allow(improper_ctypes_definitions)] // AndroidApp is the android-activity ABI type
pub extern "C" fn android_main(app_handle: winit::platform::android::activity::AndroidApp) {
    crate::logging::init();
    crate::crash::install_panic_hook();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "Android launch starting"
    );
    if let Err(e) = launch_android_ui(app_handle) {
        crate::crash::capture_startup_error(&format!("Android launch failed: {e}"));
        tracing::error!(error = %e, "Android launch failed");
        eprintln!("Android launch failed: {e}");
    }
}
