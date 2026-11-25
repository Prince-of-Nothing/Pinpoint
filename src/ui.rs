//! UI module for Pinpoint Desktop using eframe/egui.
//!
//! All network work is spawned on a dedicated tokio runtime; the UI thread
//! only ever reads/writes plain state. Failures from the app layer are
//! surfaced through `PinpointApp::error_message` and shown in the header.
//!
//! Per-screen rendering lives in `ui/` submodule files (one per screen);
//! this file keeps the app shell, pending actions, header/toolbar and the
//! `eframe::App` dispatcher.

pub(crate) mod about;
pub(crate) mod admin;
pub(crate) mod auth;
pub(crate) mod bookmarks;
pub(crate) mod crash_dialog;
pub(crate) mod emergency;
pub(crate) mod friends;
pub(crate) mod kiosk;
pub(crate) mod map;
pub(crate) mod messages;
pub(crate) mod places;
pub(crate) mod profile;
pub(crate) mod qr;
pub(crate) mod route;
pub(crate) mod share_sheet;
pub(crate) mod sharing;
pub(crate) mod theme;
pub(crate) mod today;
pub(crate) mod trips;

use crate::app::{CurrentScreen, PinpointApp};
use crate::backend::DynBackend;
use crate::i18n::{self, key, Lang};
use crate::osm::OsmClient;
use crash_dialog::CrashDialog;
use eframe::egui::{self, Align, RichText};
use route::RoutePlan;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

/// Desktop width (pt) at which the drawer moves from a bottom sheet to a
/// right-hand side panel.
const WIDE_LAYOUT_MIN_WIDTH: f32 = 820.0;

/// Below this width the open drawer overlays the map instead of reserving
/// 380 pt. The map screen needs 300 pt for details + 320 pt for the map +
/// 24 pt gap, so a 380 pt panel leaves too little room below 1100 pt.
const DRAWER_OVERLAY_MAX_WIDTH: f32 = 1100.0;

fn is_wide_width(width: f32) -> bool {
    width >= WIDE_LAYOUT_MIN_WIDTH
}

fn drawer_overlays_map(width: f32) -> bool {
    width < DRAWER_OVERLAY_MAX_WIDTH
}

/// A click left of the overlay panel dismisses it; clicks on the panel do not.
fn overlay_click_dismisses(click: Option<egui::Pos2>, panel_left: f32) -> bool {
    matches!(click, Some(pos) if pos.x < panel_left)
}

/// Plain state changes requested by one frame of the "More" drawer.
#[derive(Debug, Default, PartialEq, Eq)]
struct DrawerChanges {
    screen: Option<CurrentScreen>,
    lang: Option<Lang>,
    publish_open: Option<bool>,
    refresh: bool,
    close: bool,
}

/// Escape dismisses a surface unless a dialog or sheet above it owns the
/// keyboard.
fn escape_dismisses(escape: bool, blocked: bool) -> bool {
    escape && !blocked
}

fn drawer_changes(
    chosen: Option<CurrentScreen>,
    chosen_lang: Option<Lang>,
    publish: bool,
    refresh: bool,
    compact: bool,
) -> DrawerChanges {
    let mut changes = DrawerChanges {
        screen: chosen,
        lang: chosen_lang,
        ..Default::default()
    };
    changes.close = changes.screen.is_some();
    if publish {
        changes.screen = Some(CurrentScreen::Map);
        // The sheet flag only feeds the compact map; desktop already shows the
        // publish card in the map details column.
        changes.publish_open = Some(compact);
        changes.close = true;
    }
    if refresh {
        changes.refresh = true;
        changes.close = true;
    }
    changes
}

#[derive(Clone)]
#[allow(dead_code)]
enum PendingAction {
    Login {
        email: String,
        password: String,
        totp: Option<String>,
    },
    Register {
        email: String,
        password: String,
        full_name: String,
    },
    Logout,
    RestorePersistedSession,
    AnonymousSignIn,
    DeleteAccount {
        confirm: String,
        password: Option<String>,
    },
    RecoverAnonymous {
        code: String,
    },
    SendFriendRequest {
        handle: String,
    },
    RespondFriendship {
        id: i64,
        action: String,
    },
    CreateShare {
        friendship_id: i64,
        direction: String,
        precision: String,
    },
    ToggleShare {
        share_id: i64,
        is_active: bool,
    },
    ChangePassword {
        current: Option<String>,
        new_password: String,
        confirm: String,
    },
    PublishLocation {
        lat: f64,
        lon: f64,
        accuracy: f64,
    },
    CreateNote {
        latitude: f64,
        longitude: f64,
        label: String,
        message: String,
    },
    DeleteNote {
        id: i64,
    },
    RefreshToday,
    SearchPlaces {
        query: String,
    },
    DeleteShare {
        share_id: i64,
    },
    CreateShareEx {
        friendship_id: i64,
        direction: String,
        precision: String,
        duration_hours: Option<i64>,
    },
    SaveShare {
        friendship_id: i64,
        share_id: Option<i64>,
        enabled: bool,
        precision: String,
        /// `None` keeps the existing expiry (existing shares only).
        duration_hours: Option<i64>,
        /// Optional owner-visible name; an empty string clears it.
        label: String,
    },
    DeleteFriendship {
        id: i64,
    },
    CreateCheckIn {
        lat: f64,
        lon: f64,
        note: String,
        kind: String,
    },
    TriggerEmergency {
        lat: f64,
        lon: f64,
    },
    Route {
        from: (f64, f64),
        to: (f64, f64),
        mode: String,
    },
    PlanRoute {
        from: (f64, f64),
        to: (f64, f64),
        mode: String,
        via: Vec<(f64, f64)>,
        avoid: Vec<String>,
    },
    Poi {
        lat: f64,
        lon: f64,
        category: String,
    },
    Reverse {
        lat: f64,
        lon: f64,
    },
    UpdateProfile {
        full_name: String,
    },
    PublishGpsLocation,
    Locate,
    RefreshMessages,
    LoadMessageThread {
        friend_id: i64,
    },
    SendMessage {
        friend_id: i64,
        text: String,
    },
    ExportGpx {
        name: String,
    },
    DownloadTilePack {
        center_lat: f64,
        center_lon: f64,
        zoom: u8,
    },
    RefreshAll,
    CheckForUpdates,
    RefreshSessions,
    RevokeSession {
        id: String,
        current: bool,
    },
    LogoutAll,
    SubmitDiagnostic {
        category: String,
        message: String,
    },
    KioskResolveToken {
        token: String,
    },
    KioskSubmitCheckIn {
        request: crate::webapp::kiosk::KioskCheckInRequest,
    },
    AdminLoadOverview,
    AdminLoadUsers,
    AdminLoadAudit,
    AdminLoadDiagnostics {
        status: Option<String>,
    },
    AdminUpdateUser {
        id: i64,
        role: Option<String>,
        is_active: Option<bool>,
        must_change_password: Option<bool>,
    },
    AdminDeleteUser {
        id: i64,
    },
    AdminResetPassword {
        id: i64,
        password: String,
        reset_mfa: bool,
    },
    AdminCreateUser {
        email: String,
        full_name: String,
        role: String,
        password: String,
    },
    AdminUpdateReport {
        id: i64,
        status: String,
    },
}

#[cfg(target_os = "android")]
static ANDROID_UI_CONTEXT: std::sync::Mutex<Option<egui::Context>> = std::sync::Mutex::new(None);

/// Keep a clone of the egui context so JNI callbacks (new share, biometric
/// result) can wake an idle UI thread.
#[cfg(target_os = "android")]
pub fn store_android_context(ctx: &egui::Context) {
    if let Ok(mut guard) = ANDROID_UI_CONTEXT.lock() {
        if guard.is_none() {
            *guard = Some(ctx.clone());
        }
    }
}

#[cfg(target_os = "android")]
pub fn android_wake() {
    if let Ok(guard) = ANDROID_UI_CONTEXT.lock() {
        if let Some(ctx) = guard.as_ref() {
            ctx.request_repaint();
        }
    }
}

pub struct PinpointAppUI {
    app: Arc<PinpointApp>,
    rt: Runtime,
    /// Current UI language (auto-detected from the OS locale; overridable).
    lang: Lang,
    // Sign-in / sign-up form state
    login_email: String,
    login_password: String,
    login_totp: String,
    show_recover_form: bool,
    recover_code_input: String,
    register_email: String,
    register_password: String,
    register_full_name: String,
    // Friends screen
    friend_handle: String,
    // Today screen: private notes composer
    note_label: String,
    note_message: String,
    /// Draft of the last note the server confirmed, so the composer clears
    /// only after the POST succeeds (and only if it was not edited since).
    note_saved: Arc<Mutex<Option<(String, String)>>>,
    // Sharing screen
    share_direction: String,
    share_precision: String,
    share_duration: String,
    selected_friendship: Option<i64>,
    // Map screen: persistent publish inputs (a fresh String every frame
    // would swallow typing)
    publish_lat: String,
    publish_lon: String,
    // Search query input (results live in AppData)
    search_query: String,
    // Places screen
    poi_category: String,
    reverse_label: String,
    #[allow(dead_code)]
    route_label: String,
    // Trips / check-ins
    checkin_lat: String,
    checkin_lon: String,
    checkin_note: String,
    checkin_kind: String,
    // Bookmarks
    bookmark_name: String,
    bookmark_lat: String,
    bookmark_lon: String,
    bookmark_import_text: String,
    // Profile
    profile_name: String,
    diagnostic_category: String,
    diagnostic_message: String,
    sessions_requested: bool,
    // MFA
    mfa_code: String,
    // Emergency
    emergency_lat: String,
    emergency_lon: String,
    // Password screen
    change_password_current: String,
    change_password_new: String,
    change_password_confirm: String,
    // Account deletion
    delete_confirm: String,
    delete_password: String,
    // Crash reporting
    crash_dialog: Option<CrashDialog>,
    crash_checked: bool,
    // Admin screen
    pub(crate) admin_search_query: String,
    admin_tab: String,
    admin_loaded: bool,
    admin_diag_filter: String,
    admin_reset_password: String,
    admin_reset_mfa: bool,
    admin_new_email: String,
    admin_new_name: String,
    admin_new_role: String,
    admin_new_password: String,
    // Kiosk screen state
    pub(crate) kiosk_token_input: String,
    pub(crate) kiosk_note_input: String,
    pub(crate) kiosk_consent_accepted: bool,
    // Optional native MapLibre map window (feature `map-webview`).
    #[cfg(all(
        feature = "map-webview",
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    map_window: crate::map_window::MapWindowLauncher,
    // Route planner
    route_plan: RoutePlan,
    route_result_seq: u64,
    route_nav_last_fix: Option<std::time::Instant>,
    route_nav_gps_warned: bool,
    session_restore_attempted: bool,
    theme_installed: bool,
    // Per-friend sharing sheet
    share_sheet: Option<share_sheet::ShareSheet>,
    // Mobile shell
    mobile_nav_open: bool,
    mobile_publish_open: bool,
    mobile_selected_place: Option<crate::osm::SearchPlace>,
    // E2E messages
    message_selected: Option<i64>,
    message_draft: String,
    /// Text restored after a failed send; the newer draft wins if the user
    /// typed something else in the meantime.
    message_draft_restore: Arc<Mutex<Option<String>>>,
    message_loaded: bool,
    message_poll_at: Option<std::time::Instant>,
    /// First map view asked for location; a grant retries the fix once.
    location_prompted: bool,
    location_pending: bool,
    /// Last point reported by the Android MapLibre WebView bridge.
    webview_map_click: Option<(f64, f64)>,
    // On-device PIN app lock
    lock_enabled: bool,
    locked: bool,
    lock_armed: bool,
    pin_input: String,
    pin_error: Option<String>,
    pin_retry_at: Option<std::time::Instant>,
    pin_focus_requested: bool,
    pin_setup: String,
    pin_setup_confirm: String,
    biometric_auto_prompted: bool,
    /// Android `pinpoint://publish` requested before the first frame.
    pending_gps_publish_at: Option<std::time::Instant>,
    /// Debounce the anonymous button so a double tap cannot create two accounts.
    anonymous_busy_until: Option<std::time::Instant>,
    // Backend server setting (empty saved value = platform default)
    server_url_input: String,
    server_error: Option<String>,
    // On-device preferences and launch update check
    prefs: crate::prefs::LocalPrefs,
    prefs_store: crate::prefs::PrefsStore,
    update_check_started: bool,
    update_dialog_open: bool,
    update_dialog_for: Option<String>,
}

impl PinpointAppUI {
    /// Create the UI with a modular backend (desktop/mobile). Pass any
    /// `DynBackend` to adapt to changed backend architectures.
    pub fn new(backend: DynBackend, screenshot_path: Option<String>) -> Self {
        let app = Arc::new(PinpointApp::new(backend, screenshot_path));
        #[cfg(target_os = "android")]
        crate::platform::register_android_app(Arc::downgrade(&app));
        PinpointApp::start_auto_refresh(Arc::clone(&app));
        let lock_store = crate::app_lock::LockStore::default_location();
        let lock_enabled = lock_store.is_configured();
        let lock_remaining = lock_store.lockout_remaining();
        let prefs_store = crate::prefs::PrefsStore::default_location();
        let prefs = prefs_store.load();
        let initial_server_url = if prefs.server_url.trim().is_empty() {
            app.backend_url()
        } else {
            prefs.server_url.clone()
        };
        let mut ui = Self {
            app,
            rt: Runtime::new().expect("Failed to create tokio runtime"),
            lang: Lang::system(),
            login_email: String::new(),
            login_password: String::new(),
            login_totp: String::new(),
            show_recover_form: false,
            recover_code_input: String::new(),
            register_email: String::new(),
            register_password: String::new(),
            register_full_name: String::new(),
            friend_handle: String::new(),
            note_label: String::new(),
            note_message: String::new(),
            note_saved: Arc::new(Mutex::new(None)),
            share_direction: "to_friend".to_string(),
            share_precision: "exact".to_string(),
            share_duration: String::new(),
            selected_friendship: None,
            publish_lat: String::new(),
            publish_lon: String::new(),
            search_query: String::new(),
            poi_category: "food".to_string(),
            reverse_label: String::new(),
            route_label: String::new(),
            checkin_lat: String::new(),
            checkin_lon: String::new(),
            checkin_note: String::new(),
            checkin_kind: "manual".to_string(),
            bookmark_name: String::new(),
            bookmark_lat: String::new(),
            bookmark_lon: String::new(),
            bookmark_import_text: String::new(),
            profile_name: String::new(),
            diagnostic_category: "map".to_string(),
            diagnostic_message: String::new(),
            sessions_requested: false,
            mfa_code: String::new(),
            emergency_lat: String::new(),
            emergency_lon: String::new(),
            change_password_current: String::new(),
            change_password_new: String::new(),
            change_password_confirm: String::new(),
            delete_confirm: String::new(),
            delete_password: String::new(),
            crash_dialog: None,
            crash_checked: false,
            admin_search_query: String::new(),
            admin_tab: "overview".to_string(),
            admin_loaded: false,
            admin_diag_filter: String::new(),
            admin_reset_password: String::new(),
            admin_reset_mfa: false,
            admin_new_email: String::new(),
            admin_new_name: String::new(),
            admin_new_role: "resident".to_string(),
            admin_new_password: String::new(),
            kiosk_token_input: String::new(),
            kiosk_note_input: String::new(),
            kiosk_consent_accepted: false,
            #[cfg(all(
                feature = "map-webview",
                not(target_arch = "wasm32"),
                not(target_os = "android"),
                not(target_os = "ios")
            ))]
            map_window: crate::map_window::MapWindowLauncher::new(),
            route_plan: RoutePlan::default(),
            route_result_seq: 0,
            route_nav_last_fix: None,
            route_nav_gps_warned: false,
            session_restore_attempted: false,
            theme_installed: false,
            share_sheet: None,
            mobile_nav_open: false,
            mobile_publish_open: false,
            mobile_selected_place: None,
            message_selected: None,
            message_draft: String::new(),
            message_draft_restore: Arc::new(Mutex::new(None)),
            message_loaded: false,
            message_poll_at: None,
            location_prompted: false,
            location_pending: false,
            webview_map_click: None,
            lock_enabled,
            locked: lock_enabled,
            lock_armed: false,
            pin_input: String::new(),
            pin_error: None,
            pin_retry_at: lock_remaining.map(|remaining| std::time::Instant::now() + remaining),
            pin_focus_requested: lock_enabled,
            pin_setup: String::new(),
            pin_setup_confirm: String::new(),
            biometric_auto_prompted: false,
            pending_gps_publish_at: None,
            anonymous_busy_until: None,
            server_url_input: initial_server_url,
            server_error: None,
            prefs,
            prefs_store,
            update_check_started: false,
            update_dialog_open: false,
            update_dialog_for: None,
        };
        if ui.app.screenshot_mode() {
            ui.seed_screenshot_state();
        }
        ui
    }

    /// Give `--screenshot` a deterministic native map instead of the login
    /// screen. Uses example data only; no session or backend call.
    fn seed_screenshot_state(&mut self) {
        // A screenshot artifact should show real tiles when the local backend
        // is reachable; `main` still sets offline mode for normal launches.
        self.set_offline_mode(false);
        let now = chrono::Utc::now().to_rfc3339();
        let data = self.app.data();
        let mut data = data.lock().unwrap();
        data.user = Some(crate::models::User {
            id: 1,
            username: "demo-user".into(),
            email: "demo@example.invalid".into(),
            full_name: "Demo User".into(),
            role: "resident".into(),
            is_active: true,
            must_change_password: false,
            password_hash: None,
            totp_secret: None,
            mfa_enabled: false,
            mfa_last_step: 0,
            failed_login_attempts: 0,
            locked_until: None,
            auth_method: "email".into(),
            public_key: None,
            recovery_code_hash: None,
            identity_subject: None,
        });
        data.locations = vec![
            crate::models::FriendLocation {
                user_id: 2,
                user_name: "Sam".into(),
                latitude: 47.0245,
                longitude: 28.8322,
                precision_level: "exact".into(),
                updated_at: now.clone(),
                accuracy: Some(10.0),
            },
            crate::models::FriendLocation {
                user_id: 3,
                user_name: "Mira".into(),
                latitude: 47.0055,
                longitude: 28.8500,
                precision_level: "approximate".into(),
                updated_at: now,
                accuracy: Some(800.0),
            },
        ];
        data.my_location = Some(crate::models::FriendLocation {
            user_id: 1,
            user_name: "Demo User".into(),
            latitude: 47.0105,
            longitude: 28.8638,
            precision_level: "exact".into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            accuracy: Some(8.0),
        });
        data.map_center_lat = 47.0105;
        data.map_center_lon = 28.8638;
        data.map_zoom = 13;
        // Evidence hook: open the drawer for the wide/narrow screenshots.
        if std::env::var("PINPOINT_SCREENSHOT_DRAWER").is_ok() {
            self.mobile_nav_open = true;
        }
        if std::env::var("PINPOINT_SCREENSHOT_ROUTE").is_ok() {
            self.route_plan.origin = Some(route::RoutePoint::new(47.0245, 28.8322, "Sam (start)"));
            self.route_plan.destination = Some(route::RoutePoint::new(
                47.0055,
                28.8500,
                "Mira (destination)",
            ));
            self.route_plan.routes = vec![crate::osm::Route {
                points: vec![
                    (47.0245, 28.8322),
                    (47.0180, 28.8420),
                    (47.0110, 28.8460),
                    (47.0055, 28.8500),
                ],
                distance_m: 2800.0,
                duration_s: 480.0,
                steps: vec![
                    crate::osm::RouteStep {
                        text: "Head south-east on Bulevardul Grigore Vieru".into(),
                        distance_m: Some(600.0),
                        duration_s: Some(110.0),
                    },
                    crate::osm::RouteStep {
                        text: "Continue on Strada București".into(),
                        distance_m: Some(1200.0),
                        duration_s: Some(190.0),
                    },
                    crate::osm::RouteStep {
                        text: "Arrive at Mira's pin".into(),
                        distance_m: Some(1000.0),
                        duration_s: Some(180.0),
                    },
                ],
                summary: "Bulevardul Grigore Vieru".into(),
                legs: vec![crate::osm::RouteLeg {
                    distance_m: 2800.0,
                    duration_s: 480.0,
                }],
            }];
            self.route_plan.selected = 0;
            self.route_plan.open = true;
            self.route_plan.apply_selected(&mut data);
        }
        drop(data);
        self.app.set_screen(CurrentScreen::Map);
    }

    /// Mark the mobile offline fallback as untrusted so it cannot restore or
    /// accept a session over the shared loopback endpoint.
    pub fn set_backend_unavailable(&self, unavailable: bool) {
        self.app.set_backend_unavailable(unavailable);
    }

    /// Persist the on-device preferences. A failure is non-fatal: the choice
    /// still applies for this session.
    fn save_prefs(&self) {
        let _ = self.prefs_store.save(&self.prefs);
    }

    /// Rebuild the app against `raw_url` and persist the choice. The old
    /// session and E2E key are discarded so they never cross servers.
    pub fn switch_backend(&mut self, raw_url: &str) -> Result<(), String> {
        let trimmed = raw_url.trim();
        let url = if trimmed.is_empty() {
            crate::platform::default_backend_url()
        } else {
            trimmed.to_string()
        };
        let backend = crate::backend::build_default_backend(&url).map_err(|e| e.to_string())?;
        let old = Arc::clone(&self.app);
        old.stop_auto_refresh();
        old.forget_local_identity();
        let app = Arc::new(PinpointApp::new(backend, None));
        #[cfg(target_os = "android")]
        crate::platform::register_android_app(Arc::downgrade(&app));
        PinpointApp::start_auto_refresh(Arc::clone(&app));
        self.app = app;
        self.session_restore_attempted = false;
        self.sessions_requested = false;
        self.login_password.clear();
        self.login_totp.clear();
        self.mfa_code.clear();
        self.profile_name.clear();
        self.prefs.server_url = if url == crate::platform::default_backend_url() {
            String::new()
        } else {
            url.clone()
        };
        self.save_prefs();
        self.server_url_input = url;
        self.server_error = None;
        self.app.set_screen(crate::app::CurrentScreen::Login);
        Ok(())
    }

    /// Record the check time, persist it, and start a GitHub releases check.
    /// Manual checks are always allowed; launch checks are throttled by
    /// [`crate::prefs::update_check_due`].
    fn request_update_check(&mut self, ctx: &egui::Context) {
        self.prefs.last_update_check_unix = crate::prefs::now_unix();
        self.prefs.last_update_check_version = crate::update::current_version().to_string();
        self.save_prefs();
        self.spawn_action(PendingAction::CheckForUpdates, ctx);
    }

    /// Dismiss the banner for one release tag; a later release shows it again.
    fn dismiss_update_banner(&mut self) {
        let tag = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            match &data.update_status {
                crate::update::UpdateStatus::Available { latest, .. } => latest.tag.clone(),
                _ => return,
            }
        };
        self.prefs.dismissed_update_tag = tag;
        self.save_prefs();
    }

    /// Non-blocking banner shown when a newer GitHub release exists. It is
    /// deliberately visible on every screen except About (which has its own
    /// update card) and remembers a per-release dismissal.
    fn draw_update_banner(&mut self, ui: &mut egui::Ui) {
        if self.app.current_screen() == CurrentScreen::About {
            return;
        }
        let latest = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            crate::update::banner_release(&data.update_status, &self.prefs.dismissed_update_tag)
                .cloned()
        };
        let Some(latest) = latest else { return };
        egui::Frame::new()
            .fill(theme::PANEL_2)
            .stroke(egui::Stroke::new(1.0_f32, theme::ACCENT))
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "Pinpoint {} is available (you have {}).",
                                latest.tag,
                                crate::update::current_version()
                            ))
                            .size(14.0)
                            .strong()
                            .color(theme::TEXT),
                        );
                        if cfg!(target_os = "android") {
                            if let Some(download) = latest.download_url() {
                                if ui.button("Download APK").clicked() {
                                    let _ = webbrowser::open(download);
                                }
                            }
                        }
                        if ui.button("What's new").clicked() {
                            self.app.set_screen(CurrentScreen::About);
                        }
                        if ui.button("Later").clicked() {
                            self.dismiss_update_banner();
                        }
                    });
                    let summary = crate::update::notes_plain(&latest.notes)
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .take(2)
                        .collect::<Vec<_>>()
                        .join(" · ");
                    if !summary.is_empty() {
                        theme::caption(ui, summary);
                    }
                });
            });
        ui.add_space(6.0);
    }

    /// Open the release-notes dialog once per available tag. Dismissing it
    /// stores the tag, so a later launch does not nag for the same release.
    fn maybe_open_update_dialog(&mut self) {
        let tag = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            crate::update::banner_release(&data.update_status, &self.prefs.dismissed_update_tag)
                .map(|latest| latest.tag.clone())
        };
        if let Some(tag) = tag {
            if self.update_dialog_for.as_deref() != Some(tag.as_str()) {
                self.update_dialog_for = Some(tag);
                self.update_dialog_open = true;
            }
        }
    }

    fn draw_update_dialog(&mut self, ctx: &egui::Context) {
        if !self.update_dialog_open {
            return;
        }
        let latest = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            match &data.update_status {
                crate::update::UpdateStatus::Available { latest, .. } => Some(latest.clone()),
                _ => None,
            }
        };
        let Some(latest) = latest else {
            self.update_dialog_open = false;
            return;
        };
        // The dialog is topmost; Escape dismisses it like "Later" does.
        if escape_dismisses(ctx.input(|i| i.key_pressed(egui::Key::Escape)), false) {
            self.update_dialog_open = false;
            self.dismiss_update_banner();
            self.update_dialog_for = None;
            return;
        }
        let mut open = self.update_dialog_open;
        let mut close_and_dismiss = false;
        egui::Window::new(format!("Pinpoint {} is available", latest.tag))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(480.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "You have {}. Here is what changed:",
                        crate::update::current_version()
                    ))
                    .size(12.0),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("update_dialog_notes")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(crate::update::notes_plain(&latest.notes)).size(12.0),
                        );
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if cfg!(target_os = "android") {
                        if let Some(download) = latest.download_url() {
                            if theme::primary_button(ui, "Download APK").clicked() {
                                let _ = webbrowser::open(download);
                            }
                        }
                    }
                    if ui.button("Open release page").clicked() {
                        let _ = webbrowser::open(&latest.url);
                    }
                    if ui.button("Later").clicked() {
                        close_and_dismiss = true;
                    }
                });
                if cfg!(target_os = "android") {
                    if let Some(name) = latest.apk_name() {
                        let size = latest
                            .assets
                            .iter()
                            .find(|asset| asset.name == name)
                            .map(|asset| asset.size)
                            .unwrap_or(0);
                        ui.label(
                            RichText::new(format!(
                                "{name} · {:.1} MB · direct download",
                                size as f64 / 1_048_576.0
                            ))
                            .size(10.0)
                            .color(theme::MUTED),
                        );
                    }
                }
            });
        self.update_dialog_open = open && !close_and_dismiss;
        if close_and_dismiss || !self.update_dialog_open {
            self.dismiss_update_banner();
            self.update_dialog_for = None;
        }
    }

    /// Focus the map on a coordinate, marking the viewport initialized so the
    /// first render does not overwrite it with the default/demo centre.
    pub(crate) fn focus_map_at(&mut self, latitude: f64, longitude: f64) {
        self.app.set_screen(CurrentScreen::Map);
        let data = self.app.data();
        let mut data = data.lock().unwrap();
        data.map_center_lat = latitude;
        data.map_center_lon = longitude;
        data.map_zoom = 15;
    }

    /// Centre the map on a coordinate without changing the zoom.
    pub(crate) fn center_map_at(&mut self, latitude: f64, longitude: f64) {
        self.app.set_screen(CurrentScreen::Map);
        let data = self.app.data();
        let mut data = data.lock().unwrap();
        data.map_center_lat = latitude;
        data.map_center_lon = longitude;
    }

    /// Apply an Android `pinpoint://` or `geo:` launch URI.
    pub fn apply_deep_link(&mut self, link: crate::mobile::MobileDeepLink) {
        match link {
            crate::mobile::MobileDeepLink::Geo {
                latitude,
                longitude,
                ..
            } => {
                self.focus_map_at(latitude, longitude);
            }
            crate::mobile::MobileDeepLink::Screen(screen) => self.app.set_screen(screen),
            crate::mobile::MobileDeepLink::Kiosk { token } => {
                self.app.set_screen(CurrentScreen::Kiosk);
                if let Some(token) = token {
                    self.kiosk_token_input = token;
                }
            }
            crate::mobile::MobileDeepLink::PublishLocation => {
                self.app.set_screen(CurrentScreen::Map);
                self.pending_gps_publish_at = Some(std::time::Instant::now());
            }
        }
    }

    /// Compatibility constructor for the concrete REST client.
    pub fn new_rest(
        api_client: Arc<crate::api::ApiClient>,
        screenshot_path: Option<String>,
    ) -> Self {
        Self::new(api_client as DynBackend, screenshot_path)
    }

    pub fn set_offline_mode(&mut self, offline: bool) {
        self.app.set_offline_mode(offline);
    }

    /// True when the process was started with `--screenshot`.
    pub fn screenshot_mode(&self) -> bool {
        self.app.screenshot_mode()
    }

    /// Share plain text: the Android system share sheet, or the clipboard on
    /// desktop. Used for `geo:` links and kiosk deep links.
    pub(crate) fn share_text(&mut self, ctx: &egui::Context, text: &str) {
        #[cfg(target_os = "android")]
        {
            let _ = ctx;
            if let Err(error) = crate::platform::share_text(text) {
                self.app
                    .set_error(Some(format!("Could not open the share sheet: {error}")));
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            match crash_dialog::copy_to_clipboard(ctx, text) {
                Ok(()) => self
                    .app
                    .set_notice(Some("Link copied to the clipboard.".to_string())),
                Err(error) => self
                    .app
                    .set_error(Some(format!("Could not copy the link: {error}"))),
            }
        }
    }

    pub fn set_screen(&mut self, screen: CurrentScreen) {
        self.app.set_screen(screen);
    }

    /// Touch/narrow layout: always on phones, below 820 pt on desktop.
    fn is_compact(ctx: &egui::Context) -> bool {
        cfg!(any(target_os = "android", target_os = "ios"))
            || !is_wide_width(ctx.screen_rect().width())
    }

    fn spawn_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        let app = Arc::clone(&self.app);
        let note_saved = Arc::clone(&self.note_saved);
        let restore_draft = Arc::clone(&self.message_draft_restore);
        let repaint = ctx.clone();
        self.rt.spawn(async move {
            match action {
                PendingAction::Login {
                    email,
                    password,
                    totp,
                } => {
                    let _ = app.login(email, password, totp).await;
                }
                PendingAction::Register {
                    email,
                    password,
                    full_name,
                } => {
                    let _ = app.register(email, password, full_name).await;
                }
                PendingAction::Logout => {
                    let _ = app.logout().await;
                }
                PendingAction::RestorePersistedSession => {
                    let _ = app.restore_persisted_session().await;
                }
                PendingAction::AnonymousSignIn => {
                    let device =
                        format!("Pinpoint {}", crate::platform::Platform::current().label());
                    if let Ok(code) = app.sign_in_anonymous(device).await {
                        app.data().lock().unwrap().recovery_code = Some(code);
                    }
                }
                PendingAction::RecoverAnonymous { code } => {
                    if let Ok(new_code) = app.recover_anonymous(code).await {
                        app.data().lock().unwrap().recovery_code = Some(new_code);
                    }
                }
                PendingAction::SendFriendRequest { handle } => {
                    let _ = app.send_friend_request(handle).await;
                }
                PendingAction::RespondFriendship { id, action } => {
                    let _ = app.respond_friendship(id, action).await;
                }
                PendingAction::CreateShare {
                    friendship_id,
                    direction,
                    precision,
                } => {
                    let _ = app.create_share(friendship_id, direction, precision).await;
                }
                PendingAction::ToggleShare {
                    share_id,
                    is_active,
                } => {
                    let _ = app
                        .update_share(share_id, Some(is_active), None, None, None)
                        .await;
                }
                PendingAction::ChangePassword {
                    current,
                    new_password,
                    confirm,
                } => {
                    let _ = app.change_password(current, new_password, confirm).await;
                }
                PendingAction::PublishLocation { lat, lon, accuracy } => {
                    let _ = app.publish_location(lat, lon, accuracy).await;
                }
                PendingAction::CreateNote {
                    latitude,
                    longitude,
                    label,
                    message,
                } => {
                    match app
                        .create_place_note(latitude, longitude, label.clone(), message.clone())
                        .await
                    {
                        Ok(_) => {
                            *note_saved.lock().unwrap() = Some((label, message));
                            app.set_notice(Some("Note saved.".to_string()));
                        }
                        Err(e) => app.set_error(Some(e.to_string())),
                    }
                }
                PendingAction::DeleteNote { id } => match app.delete_place_note(id).await {
                    Ok(()) => app.set_notice(Some("Note deleted.".to_string())),
                    Err(e) => app.set_error(Some(e.to_string())),
                },
                PendingAction::RefreshToday => {
                    app.set_loading(true);
                    let result = app.refresh_today().await;
                    app.set_loading(false);
                    match result {
                        Ok(()) => app.set_notice(Some("Today refreshed.".to_string())),
                        Err(e) => app.set_error(Some(e.to_string())),
                    }
                }
                PendingAction::SearchPlaces { query } => {
                    let v = crate::security::validation::validate_search_query(&query);
                    if !v.is_valid() {
                        app.set_error(Some(v.errors.join("; ")));
                        return;
                    }
                    let (lat, lon) = {
                        let d = app.data();
                        let d = d.lock().unwrap();
                        (d.map_center_lat, d.map_center_lon)
                    };
                    match app
                        .map_search(&query, Some(lat), Some(lon), 8)
                        .await
                    {
                        Ok(results) => {
                            let data = app.data();
                            let mut d = data.lock().unwrap();
                            d.search_results = results;
                            d.search_error = None;
                        }
                        Err(e) => {
                            app.data().lock().unwrap().search_error = Some(e.to_string());
                        }
                    }
                }
                PendingAction::DeleteShare { share_id } => {
                    let _ = app.delete_share(share_id).await;
                }
                PendingAction::CreateShareEx {
                    friendship_id,
                    direction,
                    precision,
                    duration_hours,
                } => {
                    let _ = app
                        .create_share_with_expiry(
                            friendship_id,
                            direction,
                            precision,
                            duration_hours,
                            None,
                        )
                        .await;
                }
                PendingAction::DeleteFriendship { id } => {
                    let _ = app.delete_friendship(id).await;
                }
                PendingAction::SaveShare {
                    friendship_id,
                    share_id,
                    enabled,
                    precision,
                    duration_hours,
                    label,
                } => {
                    if let Some(share_id) = share_id {
                        let _ = app
                            .update_share(
                                share_id,
                                Some(enabled),
                                Some(precision),
                                duration_hours,
                                Some(label),
                            )
                            .await;
                    } else if enabled {
                        let expiry = duration_hours.filter(|hours| *hours > 0);
                        let label = if label.trim().is_empty() {
                            None
                        } else {
                            Some(label)
                        };
                        let _ = app
                            .create_share_with_expiry(
                                friendship_id,
                                "to_friend".to_string(),
                                precision,
                                expiry,
                                label,
                            )
                            .await;
                    }
                }
                PendingAction::CreateCheckIn {
                    lat,
                    lon,
                    note,
                    kind,
                } => {
                    let _ = app.create_check_in(lat, lon, note, kind).await;
                }
                PendingAction::TriggerEmergency { lat, lon } => {
                    let _ = app.trigger_emergency(lat, lon).await;
                }
                PendingAction::Route { from, to, mode } => {
                    match app
                        .map_route(from, to, &mode, &[], &[], true)
                        .await
                    {
                        Ok(routes) => match routes.into_iter().next() {
                            Some(route) => {
                                let mins = (route.duration_s / 60.0) as i64;
                                app.set_notice(Some(format!(
                                    "Route: {:.2} km, ~{} min.",
                                    route.distance_m / 1000.0,
                                    mins
                                )));
                                app.data().lock().unwrap().last_route = Some(route);
                            }
                            None => app.set_error(Some("No route found.".to_string())),
                        },
                        Err(e) => app.set_error(Some(e.to_string())),
                    }
                }
                PendingAction::PlanRoute {
                    from,
                    to,
                    mode,
                    via,
                    avoid,
                } => {
                    let result = app.map_route(from, to, &mode, &via, &avoid, true).await;
                    let data = app.data();
                    let mut d = data.lock().unwrap();
                    match result {
                        Ok(routes) => {
                            d.route_results = routes;
                            d.route_error = None;
                        }
                        Err(e) => {
                            d.route_error = Some(e.to_string());
                        }
                    }
                    d.route_result_seq = d.route_result_seq.wrapping_add(1);
                }
                PendingAction::Poi { lat, lon, category } => {
                    if let Ok(client) = OsmClient::new() {
                        match client.pois(lat, lon, &category).await {
                            Ok(pois) => {
                                let count = pois.len();
                                app.data().lock().unwrap().poi_results = pois;
                                app.set_notice(Some(format!(
                                    "Found {} {} place(s) nearby",
                                    count, category
                                )));
                            }
                            Err(e) => app.set_error(Some(e.to_string())),
                        }
                    }
                }
                PendingAction::Reverse { lat, lon } => {
                    match app.map_reverse(lat, lon).await {
                        Ok(place) => {
                            app.set_notice(Some(format!("Address: {}", place.display_name)));
                            app.data().lock().unwrap().selected_place = Some(place);
                        }
                        Err(e) => app.set_error(Some(e.to_string())),
                    }
                }
                PendingAction::UpdateProfile { full_name } => {
                    let _ = app.update_profile(full_name).await;
                }
                PendingAction::PublishGpsLocation => {
                    let _ = app.publish_my_location().await;
                }
                PendingAction::Locate => {
                    if crate::platform::ensure_location_permission()
                        != crate::platform::LocationPermission::Granted
                    {
                        app.set_notice(Some(
                            "Allow location access in system settings, then tap Locate again."
                                .to_string(),
                        ));
                        return;
                    }
                    let fix = match app.locate() {
                        Some(fix) => Some(fix),
                        None => {
                            app.set_notice(Some("Getting your location...".to_string()));
                            tokio::task::spawn_blocking(|| {
                                crate::platform::request_single_fix(std::time::Duration::from_secs(
                                    20,
                                ))
                            })
                            .await
                            .ok()
                            .flatten()
                        }
                    };
                    match fix {
                        Some((lat, lon, accuracy)) => {
                            let data = app.data();
                            let mut d = data.lock().unwrap();
                            let (user_id, user_name) = d
                                .user
                                .as_ref()
                                .map(|u| (u.id, u.full_name.clone()))
                                .unwrap_or((0, "Me".to_string()));
                            d.my_location = Some(crate::models::FriendLocation {
                                user_id,
                                user_name,
                                latitude: lat,
                                longitude: lon,
                                precision_level: "exact".to_string(),
                                updated_at: chrono::Utc::now().to_rfc3339(),
                                accuracy: Some(accuracy),
                            });
                            d.map_center_lat = lat;
                            d.map_center_lon = lon;
                            d.map_zoom = d.map_zoom.max(14);
                            drop(d);
                            app.set_notice(Some(format!("Located: {lat:.5}, {lon:.5}")));
                        }
                        None => app.set_error(Some(
                            "No location fix yet. Check that device Location is on, then try again."
                                .to_string(),
                        )),
                    }
                }
                PendingAction::RefreshMessages => {
                    let _ = app.refresh_messages().await;
                }
                PendingAction::LoadMessageThread { friend_id } => {
                    let _ = app.load_message_thread(friend_id).await;
                }
                PendingAction::SendMessage { friend_id, text } => {
                    if let Err(error) = app.send_message(friend_id, text.clone()).await {
                        app.set_error(Some(format!("Could not send: {error}")));
                        // The composer clears on submit; keep the text so a
                        // transient failure does not lose the message.
                        *restore_draft.lock().unwrap() = Some(text);
                    }
                }
                PendingAction::ExportGpx { name } => {
                    let route = app.data().lock().unwrap().last_route.clone();
                    match route {
                        Some(route) => match crate::osm::export_route_gpx(&route, &name) {
                            Ok(path) => app.set_notice(Some(format!(
                                "GPX exported to {}",
                                path.display()
                            ))),
                            Err(e) => app.set_error(Some(format!("GPX export failed: {e}"))),
                        },
                        None => app.set_error(Some("No route to export.".to_string())),
                    }
                }
                PendingAction::DownloadTilePack {
                    center_lat,
                    center_lon,
                    zoom,
                } => {
                    let delta = 0.02;
                    let z = zoom.clamp(10, 15);
                    let triples = crate::tilepacks::area_pack_tiles(
                        center_lat, center_lon, zoom, delta,
                    );
                    let backend_url = app.backend().backend_url().to_string();
                    let (ok, skipped, failed) = crate::tilepacks::download_pack(
                        &triples,
                        None,
                        Some(&backend_url),
                    )
                    .await;
                    app.set_notice(Some(format!(
                        "Offline tile pack (z{}–z{}): {ok} downloaded, {skipped} cached, {failed} failed.",
                        z.saturating_sub(1),
                        (z + 1).min(18)
                    )));
                }
                PendingAction::RefreshAll => {
                    let _ = app.refresh_all().await;
                    let _ = app.refresh_my_location().await;
                    let _ = app.refresh_check_ins().await;
                    let _ = app.refresh_emergencies().await;
                    let _ = app.refresh_users().await;
                }
                PendingAction::CheckForUpdates => {
                    {
                        app.data().lock().unwrap().update_status =
                            crate::update::UpdateStatus::Checking;
                    }
                    let next = match crate::update::check_latest().await {
                        Ok(Some(latest)) => {
                            let current = crate::update::current_version().to_string();
                            if crate::update::is_newer(&current, &latest.tag) {
                                crate::update::UpdateStatus::Available { current, latest }
                            } else {
                                crate::update::UpdateStatus::UpToDate { current }
                            }
                        }
                        Ok(None) => crate::update::UpdateStatus::NoReleases,
                        Err(_) => crate::update::UpdateStatus::Error(
                            "Could not reach github.com. Check your connection and try again."
                                .to_string(),
                        ),
                    };
                    app.data().lock().unwrap().update_status = next;
                }
                PendingAction::RefreshSessions => {
                    let _ = app.refresh_sessions().await;
                }
                PendingAction::RevokeSession { id, current } => {
                    if current {
                        let _ = app.logout().await;
                    } else {
                        let _ = app.revoke_session(id).await;
                    }
                }
                PendingAction::LogoutAll => {
                    let _ = app.logout_all().await;
                }
                PendingAction::DeleteAccount { confirm, password } => {
                    if let Err(error) = app.delete_my_account(confirm, password).await {
                        app.set_error(Some(error.to_string()));
                    }
                }
                PendingAction::SubmitDiagnostic { category, message } => {
                    match app.submit_diagnostic(category, message).await {
                        Ok(id) => app.set_notice(Some(format!("Report #{id} sent. Thank you."))),
                        Err(e) => app.set_error(Some(e.to_string())),
                    }
                }
                PendingAction::KioskResolveToken { token } => {
                    let backend_url = app.backend().backend_url().to_string();
                    let routes = crate::webapp::kiosk::KioskRestRoutes::default();
                    if let Ok(adapter) =
                        crate::webapp::kiosk_rest::RestKioskAdapter::new(&backend_url, routes)
                    {
                        use crate::webapp::kiosk::KioskBackend;
                        match adapter.resolve_share_token(&token).await {
                            Ok(view) => {
                                app.set_notice(Some("Share token resolved.".into()));
                                app.data().lock().unwrap().kiosk_resolved = Some(view);
                            }
                            Err(e) => app.set_error(Some(format!("Failed to resolve token: {e}"))),
                        }
                    }
                }
                PendingAction::KioskSubmitCheckIn { request } => {
                    let backend_url = app.backend().backend_url().to_string();
                    let routes = crate::webapp::kiosk::KioskRestRoutes::default();
                    if let Ok(adapter) =
                        crate::webapp::kiosk_rest::RestKioskAdapter::new(&backend_url, routes)
                    {
                        use crate::webapp::kiosk::KioskBackend;
                        match adapter.submit_check_in(request).await {
                            Ok(()) => {
                                app.set_notice(Some("Check-in submitted successfully.".into()));
                            }
                            Err(e) => app.set_error(Some(format!("Check-in failed: {e}"))),
                        }
                    }
                }
                PendingAction::AdminLoadOverview => {
                    let _ = app.admin_load_overview().await;
                }
                PendingAction::AdminLoadUsers => {
                    let _ = app.admin_load_users().await;
                }
                PendingAction::AdminLoadAudit => {
                    let _ = app.admin_load_audit().await;
                }
                PendingAction::AdminLoadDiagnostics { status } => {
                    let _ = app.admin_load_diagnostics(status).await;
                }
                PendingAction::AdminUpdateUser {
                    id,
                    role,
                    is_active,
                    must_change_password,
                } => {
                    let mut body = serde_json::json!({});
                    if let Some(role) = role {
                        body["role"] = serde_json::Value::String(role);
                    }
                    if let Some(active) = is_active {
                        body["is_active"] = serde_json::Value::Bool(active);
                    }
                    if let Some(flag) = must_change_password {
                        body["must_change_password"] = serde_json::Value::Bool(flag);
                    }
                    let _ = app.admin_update_user(id, body).await;
                }
                PendingAction::AdminDeleteUser { id } => {
                    let _ = app.admin_delete_user(id).await;
                }
                PendingAction::AdminResetPassword {
                    id,
                    password,
                    reset_mfa,
                } => match app.admin_reset_password(id, password, reset_mfa).await {
                    Ok(()) => app.set_notice(Some(
                        "Temporary password issued. Share it securely.".to_string(),
                    )),
                    Err(error) => app.set_error(Some(error.to_string())),
                },
                PendingAction::AdminCreateUser {
                    email,
                    full_name,
                    role,
                    password,
                } => match app.admin_create_user(email, full_name, role, password).await {
                    Ok(()) => app.set_notice(Some(
                        "User created. Share the temporary password securely.".to_string(),
                    )),
                    Err(error) => app.set_error(Some(error.to_string())),
                },
                PendingAction::AdminUpdateReport { id, status } => {
                    let _ = app.admin_update_report(id, status).await;
                }
            }
            repaint.request_repaint();
        });
        ctx.request_repaint();
    }

    fn draw_header_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let signed_in = self.app.is_signed_in();
        let lang = self.lang;

        if Self::is_compact(ctx) {
            // Android/iOS status bar sits over the window; leave a safe inset.
            ui.add_space(28.0);
            let auth_screen = matches!(
                self.app.current_screen(),
                CurrentScreen::Login | CurrentScreen::Register
            );
            let mut back = false;
            let mut nav_open = self.mobile_nav_open;
            let mut sign_in = false;
            let mut sign_out = false;
            ui.horizontal(|ui| {
                let lead = if auth_screen { "←" } else { "☰" };
                if ui
                    .add(egui::Button::new(lead).min_size(egui::vec2(44.0, 44.0)))
                    .clicked()
                {
                    if auth_screen {
                        back = true;
                    } else {
                        nav_open = !nav_open;
                    }
                }
                // Compact wordmark: the desktop one is too tall for a phone row.
                ui.label(
                    RichText::new("Pinpoint")
                        .size(18.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if !auth_screen {
                        if signed_in {
                            if ui.button(i18n::t(lang, key::SIGN_OUT)).clicked() {
                                sign_out = true;
                            }
                        } else if theme::primary_button(ui, i18n::t(lang, key::SIGN_IN)).clicked() {
                            sign_in = true;
                        }
                    }
                });
            });
            self.mobile_nav_open = nav_open;
            if back {
                self.app.set_screen(CurrentScreen::Map);
            }
            if sign_in {
                self.app.set_screen(CurrentScreen::Login);
            }
            if sign_out {
                self.spawn_action(PendingAction::Logout, ctx);
            }
            // Offline state and the spinner share the status line so the
            // action row stays a single tap target.
            let offline = self.app.is_offline_mode();
            let status = self
                .app
                .error_message()
                .map(|text| (text, theme::DANGER))
                .or_else(|| self.app.notice_message().map(|text| (text, theme::SUCCESS)));
            if offline || status.is_some() || self.app.is_loading() {
                ui.horizontal_wrapped(|ui| {
                    if self.app.is_loading() {
                        ui.spinner();
                    }
                    if offline {
                        ui.label(
                            RichText::new(i18n::t(lang, key::OFFLINE_MODE))
                                .size(11.0)
                                .strong()
                                .color(theme::ACCENT),
                        );
                    }
                    if let Some((text, color)) = status {
                        ui.colored_label(color, text);
                    }
                });
            }
            ui.add_space(6.0);
            return;
        }

        ui.horizontal(|ui| {
            theme::wordmark(ui);
            if self.app.is_offline_mode() {
                ui.label(
                    RichText::new(i18n::t(lang, key::OFFLINE_MODE))
                        .size(11.0)
                        .strong()
                        .color(theme::ACCENT),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if self.app.is_loading() {
                    ui.spinner();
                }
                // Language picker (works for every registered language).
                egui::ComboBox::from_id_salt("lang")
                    .selected_text(self.lang.native_name())
                    .show_ui(ui, |ui| {
                        for l in Lang::all() {
                            if ui
                                .selectable_label(self.lang == *l, l.native_name())
                                .clicked()
                            {
                                self.lang = *l;
                            }
                        }
                    });
                if signed_in {
                    if ui.button(i18n::t(lang, key::SIGN_OUT)).clicked() {
                        self.spawn_action(PendingAction::Logout, ctx);
                    }
                    if let Some(user) = &self.app.data().lock().unwrap().user {
                        egui::Frame::new()
                            .fill(theme::PANEL_2)
                            .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                            .corner_radius(egui::CornerRadius::same(18))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.label(RichText::new(&user.email).size(12.0).color(theme::MUTED));
                            });
                    }
                } else {
                    if theme::primary_button(ui, i18n::t(lang, key::SIGN_IN)).clicked() {
                        self.app.set_screen(CurrentScreen::Login);
                    }
                    ui.label(
                        RichText::new("Private location sharing for trusted friends")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                }
            });
        });
        if let Some(notice) = self.app.notice_message() {
            ui.colored_label(theme::SUCCESS, notice);
        }
        if let Some(err) = self.app.error_message() {
            ui.colored_label(theme::DANGER, err);
        }
        ui.add_space(4.0);
    }

    fn draw_main_toolbar(&mut self, ui: &mut egui::Ui) {
        if Self::is_compact(ui.ctx()) {
            return;
        }
        let lang = self.lang;
        let is_admin = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            data.user
                .as_ref()
                .map(|u| {
                    (u.role.eq_ignore_ascii_case("admin")
                        || u.role.eq_ignore_ascii_case("administrator"))
                        && u.is_active
                })
                .unwrap_or(false)
        };
        let screens = [
            (CurrentScreen::Map, key::MAP),
            (CurrentScreen::Today, "Today"),
            (CurrentScreen::Messages, "Messages"),
            (CurrentScreen::Friends, key::FRIENDS),
            (CurrentScreen::Sharing, key::SHARING),
            (CurrentScreen::Places, key::PLACES),
            (CurrentScreen::Bookmarks, key::SAVED),
            (CurrentScreen::Trips, key::TRIPS),
            (CurrentScreen::Emergency, key::EMERGENCY),
            (CurrentScreen::Profile, key::PROFILE),
            (CurrentScreen::Admin, key::ADMIN),
            (CurrentScreen::Kiosk, key::KIOSK),
            (CurrentScreen::ChangePassword, key::PASSWORD),
        ];
        ui.horizontal(|ui| {
            // Room for the "More" drawer button plus Refresh.
            let trailing_width = 190.0;
            let tabs_width = (ui.available_width() - trailing_width).max(240.0);
            ui.allocate_ui_with_layout(
                egui::vec2(tabs_width, 44.0),
                egui::Layout::left_to_right(Align::Center),
                |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("screen_tabs")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (screen, key_str) in screens {
                                    if screen == CurrentScreen::Admin && !is_admin {
                                        continue;
                                    }
                                    let selected = self.app.current_screen() == screen;
                                    if ui
                                        .add(egui::Button::selectable(
                                            selected,
                                            RichText::new(i18n::t(lang, key_str)).size(14.0),
                                        ))
                                        .clicked()
                                    {
                                        self.app.set_screen(screen);
                                    }
                                }
                            });
                        });
                },
            );
            if ui
                .add(egui::Button::selectable(
                    self.mobile_nav_open,
                    RichText::new("More ☰").size(14.0),
                ))
                .clicked()
            {
                self.mobile_nav_open = !self.mobile_nav_open;
            }
            if ui.button(i18n::t(lang, key::REFRESH)).clicked() {
                let app = self.app.clone();
                let rt = Arc::new(Runtime::new().expect("tokio"));
                // Fire-and-forget refresh on a throwaway runtime thread.
                std::thread::spawn(move || {
                    rt.block_on(async move {
                        let _ = app.refresh_all().await;
                        let _ = app.refresh_my_location().await;
                        let _ = app.refresh_check_ins().await;
                        let _ = app.refresh_emergencies().await;
                    });
                });
            }
        });
        ui.add_space(6.0);
    }

    fn draw_sign_in_required(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, feature: &str) {
        let lang = self.lang;
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(i18n::t(lang, key::SIGN_IN_REQUIRED));
            ui.add_space(16.0);
            ui.label(i18n::t(lang, key::SIGN_IN_TO_USE).replace("{feature}", feature));
            ui.add_space(24.0);
            theme::card(ui, |ui| {
                ui.label(i18n::t(lang, key::SIGN_IN));
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_email)
                        .hint_text("Email")
                        .desired_width(250.0),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_password)
                        .password(true)
                        .hint_text("Password")
                        .desired_width(250.0),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_totp)
                        .hint_text("Authenticator code (if MFA is on)")
                        .desired_width(250.0),
                );
                ui.add_space(16.0);
                if ui.button(i18n::t(lang, key::SIGN_IN)).clicked()
                    && !self.login_email.is_empty()
                    && !self.login_password.is_empty()
                {
                    let email = self.login_email.clone();
                    let password = std::mem::take(&mut self.login_password);
                    let totp = {
                        let code = std::mem::take(&mut self.login_totp);
                        let code = code.trim();
                        if code.is_empty() {
                            None
                        } else {
                            Some(code.to_string())
                        }
                    };
                    self.spawn_action(
                        PendingAction::Login {
                            email,
                            password,
                            totp,
                        },
                        ctx,
                    );
                }
                ui.add_space(8.0);
                ui.label(i18n::t(lang, key::OFFLINE_MAP_HINT));
                ui.add_space(8.0);
                if ui.button(i18n::t(lang, key::GO_TO_MAP)).clicked() {
                    self.app.set_screen(CurrentScreen::Map);
                }
            });
            ui.add_space(24.0);
        });
    }
}

impl PinpointAppUI {
    /// Re-lock when the app comes back from the background, so a borrowed or
    /// unlocked phone cannot see cached locations.
    fn update_focus_lock(&mut self, ctx: &egui::Context) {
        if !self.lock_enabled {
            return;
        }
        if !ctx.input(|i| i.focused) {
            self.lock_armed = true;
        } else if self.lock_armed {
            self.lock_armed = false;
            if !self.locked {
                self.locked = true;
                self.pin_input.clear();
                self.pin_error = None;
                self.pin_focus_requested = true;
                self.biometric_auto_prompted = false;
                tracing::info!("app lock: re-locked after returning to the foreground");
            }
        }
    }

    #[cfg(target_os = "android")]
    fn tick_android_biometric(&mut self) {
        if !self.prefs.biometric_unlock {
            return;
        }
        if !self.biometric_auto_prompted {
            self.biometric_auto_prompted = true;
            if !crate::platform::android_biometric_supported() {
                self.prefs.biometric_unlock = false;
                self.save_prefs();
                self.pin_error = Some("Biometrics are unavailable here; use your PIN.".to_string());
                return;
            }
            let _ = crate::platform::android_start_biometric();
        }
        match crate::platform::android_poll_biometric() {
            crate::platform::BiometricStatus::Success => {
                self.locked = false;
                self.pin_input.clear();
                self.pin_error = None;
                self.pin_retry_at = None;
                self.biometric_auto_prompted = false;
                tracing::info!("app lock: unlocked with biometrics");
            }
            crate::platform::BiometricStatus::Failed => {
                if self.pin_error.is_none() {
                    self.pin_error = Some("Biometrics cancelled; use your PIN.".to_string());
                }
            }
            crate::platform::BiometricStatus::Unavailable => {
                self.prefs.biometric_unlock = false;
                self.save_prefs();
                self.pin_error = Some("Biometrics are unavailable here; use your PIN.".to_string());
            }
            _ => {}
        }
    }

    fn draw_lock_screen(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "android")]
        {
            self.tick_android_biometric();
            if !self.locked {
                return;
            }
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.label(RichText::new("Pinpoint is locked").size(24.0).strong());
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Enter your on-device PIN to view locations.")
                            .size(13.0)
                            .color(theme::MUTED),
                    );
                    ui.add_space(20.0);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.pin_input)
                            .password(true)
                            .char_limit(12)
                            .hint_text("PIN")
                            .desired_width(160.0),
                    );
                    if self.pin_focus_requested {
                        response.request_focus();
                        self.pin_focus_requested = false;
                    }
                    let submitted =
                        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    let retry_secs = self
                        .pin_retry_at
                        .and_then(|at| at.checked_duration_since(std::time::Instant::now()))
                        .map(|d| d.as_secs() + 1)
                        .unwrap_or(0);
                    let can_try = retry_secs == 0 && !self.pin_input.is_empty();
                    if ui
                        .add_enabled(can_try, egui::Button::new("Unlock"))
                        .clicked()
                        || (submitted && can_try)
                    {
                        self.attempt_unlock();
                    }
                    if retry_secs > 0 {
                        ctx.request_repaint_after(std::time::Duration::from_millis(500));
                        ui.label(
                            RichText::new(format!("Try again in {retry_secs}s."))
                                .size(12.0)
                                .color(theme::ACCENT),
                        );
                    } else if let Some(error) = &self.pin_error {
                        ui.label(RichText::new(error).size(12.0).color(theme::DANGER));
                    }
                    #[cfg(target_os = "android")]
                    if self.prefs.biometric_unlock && crate::platform::android_biometric_supported()
                    {
                        ui.add_space(8.0);
                        if ui.button("Unlock with biometrics").clicked() {
                            self.pin_error = None;
                            self.biometric_auto_prompted = true;
                            let _ = crate::platform::android_start_biometric();
                        }
                    }
                });
            });
    }

    fn attempt_unlock(&mut self) {
        let store = crate::app_lock::LockStore::default_location();
        match store.attempt(&self.pin_input) {
            crate::app_lock::LockAttempt::Unlocked => {
                self.locked = false;
                self.pin_input.clear();
                self.pin_error = None;
                self.pin_retry_at = None;
                tracing::info!("app lock: unlocked");
            }
            crate::app_lock::LockAttempt::Wrong {
                failures,
                retry_after_ms,
            } => {
                let secs = retry_after_ms.div_ceil(1000);
                self.pin_retry_at = Some(
                    std::time::Instant::now() + std::time::Duration::from_millis(retry_after_ms),
                );
                self.pin_error = Some(if failures >= 3 {
                    format!("Wrong PIN ({secs}s lockout).")
                } else {
                    "Wrong PIN.".to_string()
                });
                self.pin_input.clear();
                self.pin_focus_requested = true;
            }
            crate::app_lock::LockAttempt::LockedOut { retry_after_ms } => {
                let secs = retry_after_ms.div_ceil(1000);
                self.pin_retry_at = Some(
                    std::time::Instant::now() + std::time::Duration::from_millis(retry_after_ms),
                );
                self.pin_error = Some(format!("Locked. Try again in {secs}s."));
                self.pin_input.clear();
                self.pin_focus_requested = true;
            }
        }
    }

    /// The "More" drawer: a bottom sheet on phones/narrow windows and a right
    /// side panel on wide desktop windows.
    fn draw_nav_drawer(&mut self, ctx: &egui::Context, compact: bool) {
        let modal_open = self.crash_dialog.is_some()
            || self.update_dialog_open
            || self.share_sheet.is_some()
            || self.route_plan.open
            || self.mobile_publish_open
            || self.mobile_selected_place.is_some();
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if escape_dismisses(escape, modal_open) {
            self.mobile_nav_open = false;
            return;
        }
        let screen_width = ctx.screen_rect().width();
        if !compact && drawer_overlays_map(screen_width) {
            let click = ctx.input(|i| {
                if i.pointer.any_click() {
                    i.pointer.interact_pos()
                } else {
                    None
                }
            });
            if overlay_click_dismisses(click, screen_width - 380.0) {
                self.mobile_nav_open = false;
                return;
            }
        }
        let lang = self.lang;
        let current = self.app.current_screen();
        let mut chosen: Option<CurrentScreen> = None;
        let mut chosen_lang: Option<Lang> = None;
        let mut publish_requested = false;
        let mut refresh_requested = false;
        let (is_admin, update_available, unread_messages) = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            let is_admin = data
                .user
                .as_ref()
                .map(|u| {
                    (u.role.eq_ignore_ascii_case("admin")
                        || u.role.eq_ignore_ascii_case("administrator"))
                        && u.is_active
                })
                .unwrap_or(false);
            (
                is_admin,
                data.update_status.is_available(),
                data.unread_messages,
            )
        };
        let frame = egui::Frame::NONE
            .fill(theme::PANEL)
            .inner_margin(egui::Margin::symmetric(12, 6));
        let mut draw = |ui: &mut egui::Ui| {
            if compact {
                let (handle, _) =
                    ui.allocate_exact_size(egui::vec2(42.0, 4.0), egui::Sense::hover());
                ui.painter().rect_filled(handle, 2.0, theme::BORDER);
                ui.add_space(6.0);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("More").size(22.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("×").clicked() {
                        chosen = Some(current.clone());
                    }
                });
            });
            theme::caption(
                ui,
                if compact {
                    "Places, saved pins and settings. The tabs stay at the bottom."
                } else {
                    "Places, saved pins and settings."
                },
            );
            ui.add_space(8.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let tile_width = (ui.available_width() - 8.0) / 2.0;
                ui.horizontal(|ui| {
                    for (label, icon, primary, requested) in [
                        ("Publish", "+", true, &mut publish_requested),
                        ("Refresh", "↻", false, &mut refresh_requested),
                    ] {
                        let ink = if primary {
                            theme::ACCENT_INK
                        } else {
                            theme::TEXT
                        };
                        let button = egui::Button::new(
                            RichText::new(format!("{icon}\n{label}"))
                                .size(14.0)
                                .strong()
                                .color(ink),
                        )
                        .fill(if primary {
                            theme::ACCENT
                        } else {
                            theme::PANEL_2
                        })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            if primary {
                                theme::ACCENT
                            } else {
                                theme::BORDER
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(14))
                        .min_size(egui::vec2(tile_width, 64.0));
                        if ui.add(button).clicked() {
                            *requested = true;
                        }
                    }
                });
                ui.add_space(8.0);
                let mut tools: Vec<(CurrentScreen, &str, &str)> = vec![
                    (CurrentScreen::Messages, "Messages", "M"),
                    (CurrentScreen::Places, "Places", "P"),
                    (CurrentScreen::Bookmarks, "Saved", "B"),
                    (CurrentScreen::Trips, "Trips", "T"),
                    (CurrentScreen::Emergency, "SOS", "!"),
                    (CurrentScreen::Profile, "Profile", "U"),
                    (
                        CurrentScreen::About,
                        if update_available {
                            "Update!"
                        } else {
                            "Updates"
                        },
                        "U",
                    ),
                    (CurrentScreen::Admin, "Admin", "A"),
                    (CurrentScreen::Kiosk, "Kiosk", "K"),
                    (CurrentScreen::ChangePassword, "Password", "PW"),
                ];
                if !is_admin {
                    tools.retain(|(screen, _, _)| *screen != CurrentScreen::Admin);
                }
                egui::Grid::new("mobile_tools_grid")
                    .num_columns(2)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        for (index, (screen, label, icon)) in tools.iter().enumerate() {
                            let selected = current == *screen;
                            let tile_label =
                                if *screen == CurrentScreen::Messages && unread_messages > 0 {
                                    format!("{icon}\n{label} ({unread_messages})")
                                } else {
                                    format!("{icon}\n{label}")
                                };
                            let button = egui::Button::new(
                                RichText::new(tile_label)
                                    .size(13.0)
                                    .strong()
                                    .color(if selected {
                                        theme::ACCENT_INK
                                    } else {
                                        theme::TEXT
                                    }),
                            )
                            .fill(if selected {
                                theme::ACCENT
                            } else {
                                theme::PANEL_2
                            })
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                if selected {
                                    theme::ACCENT
                                } else {
                                    theme::BORDER
                                },
                            ))
                            .corner_radius(egui::CornerRadius::same(14))
                            .min_size(egui::vec2(tile_width, 66.0));
                            if ui.add(button).clicked() {
                                chosen = Some(screen.clone());
                            }
                            if index % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Language").size(12.0));
                    egui::ComboBox::from_id_salt("mobile_lang")
                        .selected_text(lang.native_name())
                        .show_ui(ui, |ui| {
                            for l in Lang::all() {
                                if ui.selectable_label(lang == *l, l.native_name()).clicked() {
                                    chosen_lang = Some(*l);
                                }
                            }
                        });
                });
            });
        };
        if compact {
            egui::TopBottomPanel::bottom("mobile_nav")
                .resizable(false)
                .exact_height((ctx.screen_rect().height() * 0.82).clamp(320.0, 520.0))
                .frame(frame)
                .show(ctx, &mut draw);
        } else if drawer_overlays_map(screen_width) {
            egui::Area::new(egui::Id::new("desktop_nav_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(screen_width - 380.0, 0.0))
                .show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.set_min_height(ctx.screen_rect().height());
                    frame.show(ui, |ui| draw(ui));
                });
        } else {
            egui::SidePanel::right("desktop_nav")
                .resizable(false)
                .exact_width(380.0)
                .frame(frame)
                .show(ctx, &mut draw);
        }
        let changes = drawer_changes(
            chosen,
            chosen_lang,
            publish_requested,
            refresh_requested,
            compact,
        );
        if let Some(screen) = changes.screen {
            self.app.set_screen(screen);
        }
        if let Some(lang) = changes.lang {
            self.lang = lang;
        }
        if let Some(publish_open) = changes.publish_open {
            self.mobile_publish_open = publish_open;
        }
        if changes.close {
            self.mobile_nav_open = false;
        }
        if changes.refresh {
            self.spawn_action(PendingAction::RefreshAll, ctx);
        }
    }
}

impl eframe::App for PinpointAppUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_installed {
            self.theme_installed = true;
            theme::apply(ctx);
        }
        #[cfg(target_os = "android")]
        store_android_context(ctx);
        if self.lock_enabled {
            self.update_focus_lock(ctx);
            if self.locked {
                self.draw_lock_screen(ctx);
                return;
            }
        }
        if !self.app.is_signed_in() {
            self.delete_confirm.clear();
            self.delete_password.clear();
        }
        if !self.crash_checked {
            self.crash_checked = true;
            if let Some(stored) = crate::crash::pending_report() {
                self.crash_dialog = Some(CrashDialog::from_pending(stored));
            }
        }
        if !self.update_check_started {
            self.update_check_started = true;
            if self.prefs.update_check_on_launch
                && crate::prefs::update_check_due_for(
                    self.prefs.last_update_check_unix,
                    crate::prefs::now_unix(),
                    &self.prefs.last_update_check_version,
                    crate::update::current_version(),
                )
            {
                self.request_update_check(ctx);
            }
        }
        self.maybe_open_update_dialog();
        self.sync_route_results();
        self.tick_navigation();
        if !self.session_restore_attempted {
            self.session_restore_attempted = true;
            if self.app.has_persisted_session() && !self.app.is_backend_unavailable() {
                self.spawn_action(PendingAction::RestorePersistedSession, ctx);
            }
        }
        #[cfg(target_os = "android")]
        if let Some(text) = crate::platform::android_take_shared_text() {
            if let Some(link) = crate::mobile::MobileDeepLink::parse_shared(&text) {
                tracing::info!(?link, "android: applied shared deep link");
                self.apply_deep_link(link);
            } else {
                tracing::warn!("android: shared text had no location or kiosk link");
                self.app.set_notice(Some(format!("Shared text: {text}")));
            }
        }
        if let Some(since) = self.pending_gps_publish_at {
            if self.app.is_signed_in() {
                self.pending_gps_publish_at = None;
                self.spawn_action(PendingAction::PublishGpsLocation, ctx);
            } else if since.elapsed() > std::time::Duration::from_secs(12) {
                self.pending_gps_publish_at = None;
                self.app
                    .set_error(Some("Sign in to publish your location.".to_string()));
            }
        }
        if Self::is_compact(ctx) && self.mobile_selected_place.is_none() {
            if let Some(place) = self.app.data().lock().unwrap().selected_place.clone() {
                self.mobile_selected_place = Some(place);
            }
        }
        // Screenshot mode: request a real window capture once the first
        // frames have settled, then write it and close.
        if self.app.screenshot_mode() && !self.app.screenshot_taken() {
            self.app.increment_screenshot_counter();
            let frame = self.app.screenshot_counter();
            if frame == 121 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                ctx.request_repaint();
            } else if frame > 121 {
                let captured = ctx.input(|input| {
                    input.events.iter().find_map(|event| match event {
                        egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                        _ => None,
                    })
                });
                if let Some(image) = captured {
                    if let Some(path) = self.app.screenshot_path().clone() {
                        let mut rgba = Vec::with_capacity(image.pixels.len().saturating_mul(4));
                        for pixel in &image.pixels {
                            rgba.extend_from_slice(&pixel.to_array());
                        }
                        if let Some(buffer) = image::RgbaImage::from_raw(
                            image.size[0] as u32,
                            image.size[1] as u32,
                            rgba,
                        ) {
                            let _ = buffer.save(&path);
                        }
                    }
                    self.app.set_screenshot_taken(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                if frame > 400 {
                    // A backend without readback support must not hang CI.
                    self.app.set_screenshot_taken(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                ctx.request_repaint();
            }
        }

        let compact = Self::is_compact(ctx);
        let auth_screen = matches!(
            self.app.current_screen(),
            CurrentScreen::Login | CurrentScreen::Register
        );
        if self.mobile_nav_open {
            self.draw_nav_drawer(ctx, compact);
        }

        if compact && !auth_screen && !self.mobile_nav_open {
            let current = self.app.current_screen();
            let mut chosen: Option<CurrentScreen> = None;
            egui::TopBottomPanel::bottom("mobile_tabbar")
                .resizable(false)
                // Android 15 draws edge-to-edge; reserve room for the gesture
                // navigation bar so the tabs are not under it.
                .exact_height(74.0)
                .frame(
                    egui::Frame::NONE
                        .fill(theme::PANEL)
                        .inner_margin(egui::Margin {
                            left: 4,
                            right: 4,
                            top: 4,
                            bottom: 20,
                        }),
                )
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let tab_width = ui.available_width() / 5.0;
                    ui.horizontal(|ui| {
                        for (screen, label) in [
                            (CurrentScreen::Map, "Map"),
                            (CurrentScreen::Friends, "Friends"),
                            (CurrentScreen::Sharing, "Sharing"),
                            (CurrentScreen::Today, "Today"),
                            (CurrentScreen::Profile, "Me"),
                        ] {
                            let selected = current == screen;
                            let fill = if selected {
                                theme::ACCENT
                            } else {
                                theme::PANEL
                            };
                            let text_color = if selected {
                                theme::ACCENT_INK
                            } else {
                                theme::TEXT
                            };
                            let button = egui::Button::new(
                                RichText::new(label).size(12.0).strong().color(text_color),
                            )
                            .fill(fill)
                            .min_size(egui::vec2(tab_width, 46.0))
                            .corner_radius(egui::CornerRadius::same(10));
                            if ui.add(button).clicked() {
                                chosen = Some(screen);
                            }
                        }
                    });
                });
            if let Some(screen) = chosen {
                self.app.set_screen(screen);
            }
        }

        let panel = if compact {
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::BG))
        } else {
            egui::CentralPanel::default()
        };
        panel.show(ctx, |ui| {
            self.draw_header_bar(ui, ctx);
            self.draw_update_banner(ui);
            let auth_screen = matches!(
                self.app.current_screen(),
                CurrentScreen::Login | CurrentScreen::Register
            );
            let forced_password = self.app.current_screen() == CurrentScreen::ChangePassword
                && self.app.data().lock().unwrap().must_change_password;
            if !auth_screen && !forced_password {
                self.draw_main_toolbar(ui);
            }

            match self.app.current_screen() {
                CurrentScreen::Map => {
                    self.draw_map_screen(ui, ctx);
                }
                CurrentScreen::Today => {
                    if self.app.is_signed_in() {
                        self.draw_today_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Today");
                    }
                }
                CurrentScreen::Messages => {
                    if self.app.is_signed_in() {
                        self.draw_messages_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Messages");
                    }
                }
                CurrentScreen::Login => self.draw_login_form(ui, ctx),
                CurrentScreen::Register => self.draw_register(ui, ctx),
                CurrentScreen::Places => self.draw_places_screen(ui, ctx),
                CurrentScreen::Bookmarks => self.draw_bookmarks_screen(ui, ctx),
                CurrentScreen::Trips => {
                    if self.app.is_signed_in() {
                        self.draw_trips_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Trips");
                    }
                }
                CurrentScreen::Emergency => {
                    if self.app.is_signed_in() {
                        self.draw_emergency_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Emergency");
                    }
                }
                CurrentScreen::Profile => {
                    if self.app.is_signed_in() {
                        self.draw_profile_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Profile");
                    }
                }
                CurrentScreen::About => self.draw_about_screen(ui, ctx),
                CurrentScreen::Friends => {
                    if self.app.is_signed_in() {
                        self.draw_friends_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Friends");
                    }
                }
                CurrentScreen::Sharing => {
                    if self.app.is_signed_in() {
                        self.draw_sharing_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Sharing");
                    }
                }
                CurrentScreen::Admin => {
                    if self.app.is_signed_in() {
                        self.draw_admin_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Admin");
                    }
                }
                CurrentScreen::Kiosk => {
                    self.draw_kiosk_screen(ui, ctx);
                }
                CurrentScreen::ChangePassword => {
                    if self.app.is_signed_in() {
                        self.draw_change_password_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Change Password");
                    }
                }
            }
        });

        self.draw_share_sheet(ctx);
        self.draw_recovery_code(ctx);
        self.draw_crash_dialog(ctx);
        self.draw_update_dialog(ctx);

        // Request repaint for animation/screenshot
        if self.app.screenshot_mode() && !self.app.screenshot_taken() {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        drawer_changes, drawer_overlays_map, egui, escape_dismisses, is_wide_width,
        overlay_click_dismisses, CurrentScreen, DrawerChanges, Lang, DRAWER_OVERLAY_MAX_WIDTH,
        WIDE_LAYOUT_MIN_WIDTH,
    };

    #[test]
    fn drawer_breakpoint_switches_to_the_side_panel() {
        assert!(!is_wide_width(WIDE_LAYOUT_MIN_WIDTH - 1.0));
        assert!(is_wide_width(WIDE_LAYOUT_MIN_WIDTH));
    }

    #[test]
    fn publish_shortcut_opens_the_sheet_only_when_compact() {
        assert_eq!(
            drawer_changes(None, None, true, false, false),
            DrawerChanges {
                screen: Some(CurrentScreen::Map),
                publish_open: Some(false),
                close: true,
                ..Default::default()
            }
        );
        assert_eq!(
            drawer_changes(None, None, true, false, true).publish_open,
            Some(true)
        );
    }

    #[test]
    fn escape_closes_the_update_dialog_when_nothing_is_above_it() {
        assert!(escape_dismisses(true, false));
        assert!(!escape_dismisses(false, false));
    }

    #[test]
    fn drawer_overlays_below_1100() {
        assert!(drawer_overlays_map(DRAWER_OVERLAY_MAX_WIDTH - 1.0));
        assert!(!drawer_overlays_map(DRAWER_OVERLAY_MAX_WIDTH));
    }

    #[test]
    fn overlay_click_dismisses_only_left_of_the_panel() {
        let panel_left = 520.0;
        assert!(overlay_click_dismisses(
            Some(egui::pos2(100.0, 300.0)),
            panel_left
        ));
        assert!(!overlay_click_dismisses(
            Some(egui::pos2(panel_left, 300.0)),
            panel_left
        ));
        assert!(!overlay_click_dismisses(
            Some(egui::pos2(800.0, 300.0)),
            panel_left
        ));
        assert!(!overlay_click_dismisses(None, panel_left));
    }

    #[test]
    fn escape_dismisses_only_without_a_blocking_modal() {
        for (escape, blocked, expected) in [
            (true, false, true),
            (true, true, false),
            (false, false, false),
            (false, true, false),
        ] {
            assert_eq!(
                escape_dismisses(escape, blocked),
                expected,
                "escape={escape} blocked={blocked}"
            );
        }
    }

    #[test]
    fn drawer_actions_close_the_panel_and_keep_the_screen_otherwise() {
        let chosen = drawer_changes(Some(CurrentScreen::Today), None, false, false, false);
        assert_eq!(chosen.screen, Some(CurrentScreen::Today));
        assert!(chosen.close);
        assert_eq!(chosen.publish_open, None);

        let language = drawer_changes(None, Some(Lang::En), false, false, true);
        assert_eq!(language.lang, Some(Lang::En));
        assert!(!language.close);

        let refresh = drawer_changes(None, None, false, true, false);
        assert!(refresh.refresh);
        assert!(refresh.close);
        assert_eq!(refresh.screen, None);
    }
}
