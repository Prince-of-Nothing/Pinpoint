//! Main entry point for Pinpoint Desktop (Windows / Linux / macOS).
//!
//! One `eframe` binary for all desktop OSes; Android/iOS reuse the same
//! `src/` via `cargo-apk` / `cargo-xcodebuild` (see `docs/client-platforms.md`),
//! and the Web (WASM) client shares `src/` via a separate entry
//! (`src/bin/pinpoint-web.rs` + `web/`). Backend URL resolution:
//! `--backend <url>` > `PINPOINT_BACKEND` env > per-platform default.

#[cfg(not(target_arch = "wasm32"))]
fn print_help_and_exit() -> ! {
    let (map_window_usage, map_window_help) = if cfg!(all(
        feature = "map-webview",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    )) {
        (
            " [--map-window [URL]]",
            "\x20  --map-window [URL]  Open the MapLibre map window in a native WebView.\n\
             \x20                      Without URL, starts the loopback asset server first.\n",
        )
    } else {
        ("", "")
    };
    eprintln!(
        "Pinpoint {} ({})\n\
         \n\
         USAGE:\n\
         \x20  pinpoint-desktop [--backend <URL>] [--screenshot <PATH>]{map_window_usage} [--help]\n\
         \n\
         OPTIONS:\n\
         \x20  --backend <URL>     Backend base URL (default: {} or $PINPOINT_BACKEND).\n\
         \x20                      Plain HTTP is only allowed for loopback (ADR-0006);\n\
         \x20                      anything else must be HTTPS.\n\
         \x20  --screenshot <PATH> Headless screenshot mode (CI/test artifact).\n\
         {map_window_help}\
         \x20  --help              Show this help.\n\
         ",
        pinpoint_desktop::platform::build_identity(),
        pinpoint_desktop::platform::Platform::current().label(),
        pinpoint_desktop::platform::default_backend_url(),
    );
    std::process::exit(0);
}

#[cfg(not(target_arch = "wasm32"))]
/// Screenshot evidence hook: `PINPOINT_SCREENSHOT_SIZE=700x800`.
fn window_size_from_env() -> [f32; 2] {
    parse_window_size(&std::env::var("PINPOINT_SCREENSHOT_SIZE").unwrap_or_default())
        .unwrap_or([1200.0, 800.0])
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_window_size(raw: &str) -> Option<[f32; 2]> {
    let (width, height) = raw.split_once(|c: char| c.eq_ignore_ascii_case(&'x'))?;
    let width: f32 = width.trim().parse().ok()?;
    let height: f32 = height.trim().parse().ok()?;
    (width >= 320.0 && height >= 240.0).then_some([width, height])
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `--map-window [<loopback-url>]` is a standalone child mode launched by
    // the eframe app (feature `map-webview`). It never starts the normal app.
    #[cfg(all(
        feature = "map-webview",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    if args.get(1).map(String::as_str) == Some("--map-window") {
        let url = args.get(2).cloned();
        if let Some(raw) = url.as_deref() {
            let loopback = url::Url::parse(raw)
                .map(|parsed| {
                    parsed.scheme() == "http"
                        && parsed.host_str() == Some("127.0.0.1")
                        && parsed.path() == "/map/index.html"
                })
                .unwrap_or(false);
            if !loopback {
                eprintln!("--map-window only accepts a 127.0.0.1 loopback map URL");
                std::process::exit(2);
            }
        }
        pinpoint_desktop::map_window::run_map_window_mode(url);
    }
    #[cfg(not(all(
        feature = "map-webview",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    )))]
    if args.get(1).map(String::as_str) == Some("--map-window") {
        eprintln!(
            "--map-window requires a desktop build with the map-webview feature; \
             use the egui map in this build."
        );
        std::process::exit(2);
    }

    let mut screenshot_path = None;
    let mut backend_url = pinpoint_desktop::platform::backend_url_from_env_or_default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => print_help_and_exit(),
            "--screenshot" => {
                if i + 1 < args.len() {
                    screenshot_path = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("--screenshot requires a file path");
                    std::process::exit(1);
                }
            }
            "--backend" => {
                if i + 1 < args.len() {
                    backend_url = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("--backend requires a URL");
                    std::process::exit(1);
                }
            }
            unknown => {
                eprintln!("unknown argument: {unknown} (try --help)");
                std::process::exit(1);
            }
        }
    }

    pinpoint_desktop::logging::init();
    pinpoint_desktop::crash::install_panic_hook();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        platform = %pinpoint_desktop::platform::Platform::current().label(),
        "Pinpoint starting"
    );

    // The map is fully functional offline. Build the backend via the modular
    // factory (ADR-0006 transport policy enforced by the REST adapter). If the
    // configured URL is rejected, fall back to the loopback default so the map
    // still opens offline. Swapping backend architecture means changing
    // `BackendConfig`/adding an adapter — not this call site.
    let backend = match pinpoint_desktop::backend::build_default_backend(&backend_url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Backend URL {backend_url:?} rejected ({e}); starting in offline mode.");
            pinpoint_desktop::backend::build_default_backend("http://localhost:8000")
                .unwrap_or_else(|e2| {
                    eprintln!("Cannot create backend client: {e2}");
                    std::process::exit(1);
                })
        }
    };

    let mut app = pinpoint_desktop::ui::PinpointAppUI::new(backend, screenshot_path);

    if !app.screenshot_mode() {
        app.set_offline_mode(true);
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(window_size_from_env())
            .with_title("Pinpoint — Map First"),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native("Pinpoint", options, Box::new(|_cc| Ok(Box::new(app)))) {
        pinpoint_desktop::crash::capture_startup_error(&format!("Desktop launch failed: {e}"));
        eprintln!("Error running app: {:?}", e);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("Web target - use wasm-bindgen entry point");
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::parse_window_size;

    #[test]
    fn screenshot_window_size_parses_and_rejects_garbage() {
        assert_eq!(parse_window_size("700x800"), Some([700.0, 800.0]));
        assert_eq!(parse_window_size(" 640X480 "), Some([640.0, 480.0]));
        assert_eq!(parse_window_size("700"), None);
        assert_eq!(parse_window_size("100x800"), None);
        assert_eq!(parse_window_size("widex800"), None);
    }
}
