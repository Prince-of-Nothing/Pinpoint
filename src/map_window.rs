//! Native WebView window and browser fallback for the shared MapLibre page.
//!
//! Enabled by the opt-in `map-webview` Cargo feature. The egui application
//! starts a loopback [`crate::map_server::MapServer`] and opens the page in a
//! child process:
//!
//! * `tao` (the event loop behind `wry`) requires its event loop on the main
//!   thread, but the eframe application already owns winit's main-thread loop.
//!   A second event loop in the same process would panic, so the UI spawns
//!   `pinpoint-desktop --map-window <loopback-url>`.
//! * On Windows the child uses WebView2, on Linux WebKitGTK and on macOS
//!   WKWebView. If that runtime is missing or the WebView cannot start, the
//!   launcher opens the same loopback URL in the user's browser and keeps the
//!   server alive while Pinpoint runs. This is a real page, never a fake
//!   embedded view.
//! * The egui map stays untouched and remains the fallback.
//!
//! Navigation is restricted to the loopback origin; external attribution
//! links open in the system browser. The page never receives session, CSRF or
//! friend data from this module.

use std::process::{Child, Command};
use std::time::Duration;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, WebViewBuilder};

/// Result of pressing "Open map window".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOutcome {
    /// A native WebView child process is running.
    Started,
    /// No WebView runtime was available; the loopback URL opened in the
    /// system browser instead.
    BrowserFallback,
    /// The map window was already open.
    AlreadyRunning,
}

/// Owns the loopback server and the child process that hosts the native map
/// WebView (or the browser fallback state).
#[derive(Default)]
pub struct MapWindowLauncher {
    server: Option<crate::map_server::MapServer>,
    child: Option<Child>,
    url: Option<String>,
    webview_unavailable: bool,
}

impl MapWindowLauncher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Serve the map assets and open them in a native WebView. When the
    /// platform WebView runtime is missing, open the same loopback URL in the
    /// system browser instead and keep serving assets until Pinpoint exits.
    pub fn open(&mut self) -> Result<LaunchOutcome, String> {
        if self.is_running() {
            return Ok(LaunchOutcome::AlreadyRunning);
        }

        if self.server.is_none() {
            let root = crate::map_server::find_web_root().map_err(|error| error.to_string())?;
            let server =
                crate::map_server::MapServer::start(root).map_err(|error| error.to_string())?;
            self.url = Some(server.index_url());
            self.server = Some(server);
        }
        let url = self
            .url
            .clone()
            .ok_or_else(|| "the map server has no URL".to_string())?;

        if !self.webview_unavailable && self.spawn_webview(&url)? {
            return Ok(LaunchOutcome::Started);
        }

        // The child exited before showing a window: almost always a missing
        // WebView2/WebKitGTK runtime. Keep the loopback server and hand the
        // exact same URL to the system browser.
        self.webview_unavailable = true;
        webbrowser::open(&url).map_err(|error| {
            format!("No WebView runtime and the system browser could not be opened: {error}")
        })?;
        Ok(LaunchOutcome::BrowserFallback)
    }

    /// Spawn the child and report whether it is still alive after the startup
    /// grace period.
    fn spawn_webview(&mut self, url: &str) -> Result<bool, String> {
        let executable =
            std::env::current_exe().map_err(|error| format!("No Pinpoint executable: {error}"))?;
        let child = Command::new(executable)
            .arg("--map-window")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Could not start the map window: {error}"))?;
        self.child = Some(child);

        // A missing WebView2/WebKitGTK runtime makes the child exit
        // immediately. Wait briefly so the UI can fall back instead of
        // leaving a dead button.
        for _ in 0..16 {
            std::thread::sleep(Duration::from_millis(50));
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
            {
                self.child = None;
                tracing::warn!(%status, "map WebView child exited during startup");
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether the native WebView child is still alive (`false` after the user
    /// closes the map window).
    pub fn is_running(&mut self) -> bool {
        let Some(child) = self.child.as_mut() else {
            return false;
        };
        match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) | Err(_) => {
                self.child = None;
                false
            }
        }
    }

    /// True after the browser fallback was used; the button can say so.
    pub fn browser_fallback(&self) -> bool {
        self.webview_unavailable
    }
}

impl Drop for MapWindowLauncher {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Dropping `server` stops the loopback listener.
    }
}

/// CLI mode: `pinpoint-desktop --map-window [<loopback-url>]`.
///
/// With a URL the child only opens the WebView; the parent owns the server.
/// Without one it starts a server first (standalone testing) and falls back to
/// the system browser, keeping the server alive until Ctrl+C.
pub fn run_map_window_mode(url: Option<String>) -> ! {
    crate::logging::init();

    let (url, server) = match url {
        Some(url) => (url, None),
        None => {
            let root = match crate::map_server::find_web_root() {
                Ok(root) => root,
                Err(error) => {
                    eprintln!("Pinpoint map window: {error}");
                    std::process::exit(1);
                }
            };
            let server = match crate::map_server::MapServer::start(&root) {
                Ok(server) => server,
                Err(error) => {
                    eprintln!("Pinpoint map window: {error}");
                    std::process::exit(1);
                }
            };
            let url = server.index_url();
            (url, Some(server))
        }
    };

    eprintln!("Pinpoint map window: {url}");
    tracing::info!(%url, "opening the MapLibre WebView");

    if let Err(error) = run_webview(&url) {
        eprintln!("Pinpoint map window could not open the native WebView: {error}");
        #[cfg(target_os = "windows")]
        eprintln!(
            "Install the Microsoft Edge WebView2 Runtime \
             (https://developer.microsoft.com/microsoft-edge/webview2/) \
             or use the egui map."
        );
        #[cfg(target_os = "linux")]
        eprintln!("Install the WebKitGTK 4.1 development/runtime packages or use the egui map.");

        if server.is_some() {
            // Standalone mode: keep the loopback origin alive for the browser.
            eprintln!("Serving the map for the system browser at {url}; press Ctrl+C to stop.");
            if let Err(error) = webbrowser::open(&url) {
                eprintln!("Could not open the system browser either: {error}");
                std::process::exit(1);
            }
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
        std::process::exit(1);
    }

    // `run_webview` only returns on error; keep this arm explicit for the type
    // checker and clean exits.
    drop(server);
    std::process::exit(0);
}

fn run_webview(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|error| format!("invalid map URL: {error}"))?;
    let origin = parsed.origin().ascii_serialization();
    let origin_path = format!("{origin}/");

    let event_loop = EventLoop::new();
    // Create hidden so a missing WebView2/WebKitGTK runtime cannot leave a
    // white "Not Responding" window on screen in the browser-fallback path.
    let window = WindowBuilder::new()
        .with_title("Pinpoint — MapLibre map")
        .with_inner_size(LogicalSize::new(1100.0, 760.0))
        .with_min_inner_size(LogicalSize::new(640.0, 420.0))
        .with_visible(false)
        .build(&event_loop)
        .map_err(|error| format!("could not create the map window: {error}"))?;

    let builder = WebViewBuilder::new()
        .with_url(url)
        .with_navigation_handler(move |target| {
            if target == origin || target.starts_with(&origin_path) {
                return true;
            }
            if target.starts_with("about:") {
                return true;
            }
            open_external(&target);
            false
        })
        .with_new_window_req_handler(move |target, _features| {
            open_external(&target);
            NewWindowResponse::Deny
        });

    let webview = build_webview(builder, &window)?;
    window.set_visible(true);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
        // Keep the WebView alive for the whole event loop.
        let _ = &webview;
    });
}

#[cfg(not(target_os = "linux"))]
fn build_webview(
    builder: WebViewBuilder<'_>,
    window: &tao::window::Window,
) -> Result<wry::WebView, String> {
    builder
        .build(window)
        .map_err(|error| format!("{error}. The platform WebView runtime may be missing."))
}

#[cfg(target_os = "linux")]
fn build_webview(
    builder: WebViewBuilder<'_>,
    window: &tao::window::Window,
) -> Result<wry::WebView, String> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;

    let vbox = window
        .default_vbox()
        .ok_or_else(|| "the GTK container is unavailable".to_string())?;
    builder
        .build_gtk(vbox)
        .map_err(|error| format!("{error}. The WebKitGTK runtime may be missing."))
}

/// Attribution and other links open in the user's browser; the WebView never
/// navigates away from the loopback map origin.
fn open_external(target: &str) {
    if target.starts_with("https://") || target.starts_with("http://") {
        if let Err(error) = webbrowser::open(target) {
            tracing::warn!(%error, %target, "could not open an external map link");
        }
    }
}
