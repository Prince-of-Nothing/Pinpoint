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
pub(crate) mod places;
pub(crate) mod profile;
pub(crate) mod qr;
pub(crate) mod route;
pub(crate) mod share_sheet;
pub(crate) mod sharing;
pub(crate) mod trips;

use crate::app::{CurrentScreen, PinpointApp};
use crate::backend::DynBackend;
use crate::i18n::{self, key, Lang};
use crate::osm::OsmClient;
use crash_dialog::CrashDialog;
use eframe::egui::{self, Align, Color32, Direction, RichText};
use route::RoutePlan;
use std::sync::Arc;
use tokio::runtime::Runtime;

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
        duration_hours: i64,
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
    },
    AdminDeleteUser {
        id: i64,
    },
    AdminUpdateReport {
        id: i64,
        status: String,
    },
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
    // Crash reporting
    crash_dialog: Option<CrashDialog>,
    crash_checked: bool,
    // Admin screen
    pub(crate) admin_search_query: String,
    admin_tab: String,
    admin_loaded: bool,
    admin_diag_filter: String,
    // Kiosk screen state
    pub(crate) kiosk_token_input: String,
    pub(crate) kiosk_note_input: String,
    pub(crate) kiosk_consent_accepted: bool,
    // Route planner
    route_plan: RoutePlan,
    route_result_seq: u64,
    route_nav_last_fix: Option<std::time::Instant>,
    route_nav_gps_warned: bool,
    session_restore_attempted: bool,
    // Per-friend sharing sheet
    share_sheet: Option<share_sheet::ShareSheet>,
    // Mobile shell
    mobile_nav_open: bool,
    mobile_publish_open: bool,
    mobile_selected_place: Option<crate::osm::SearchPlace>,
    // On-device PIN app lock
    lock_enabled: bool,
    locked: bool,
    lock_armed: bool,
    pin_input: String,
    pin_error: Option<String>,
    pin_failures: u32,
    pin_retry_at: Option<std::time::Instant>,
    pin_focus_requested: bool,
    pin_setup: String,
    pin_setup_confirm: String,
}

impl PinpointAppUI {
    /// Create the UI with a modular backend (desktop/mobile). Pass any
    /// `DynBackend` to adapt to changed backend architectures.
    pub fn new(backend: DynBackend, screenshot_path: Option<String>) -> Self {
        let app = Arc::new(PinpointApp::new(backend, screenshot_path));
        PinpointApp::start_auto_refresh(Arc::clone(&app));
        let lock_enabled = crate::app_lock::LockStore::default_location().is_configured();
        Self {
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
            crash_dialog: None,
            crash_checked: false,
            admin_search_query: String::new(),
            admin_tab: "overview".to_string(),
            admin_loaded: false,
            admin_diag_filter: String::new(),
            kiosk_token_input: String::new(),
            kiosk_note_input: String::new(),
            kiosk_consent_accepted: false,
            route_plan: RoutePlan::default(),
            route_result_seq: 0,
            route_nav_last_fix: None,
            route_nav_gps_warned: false,
            session_restore_attempted: false,
            share_sheet: None,
            mobile_nav_open: false,
            mobile_publish_open: false,
            mobile_selected_place: None,
            lock_enabled,
            locked: lock_enabled,
            lock_armed: false,
            pin_input: String::new(),
            pin_error: None,
            pin_failures: 0,
            pin_retry_at: None,
            pin_focus_requested: lock_enabled,
            pin_setup: String::new(),
            pin_setup_confirm: String::new(),
        }
    }

    /// Mark the mobile offline fallback as untrusted so it cannot restore or
    /// accept a session over the shared loopback endpoint.
    pub fn set_backend_unavailable(&self, unavailable: bool) {
        self.app.set_backend_unavailable(unavailable);
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

    /// Show a short user-facing notice from platform startup code.
    pub fn show_notice(&self, message: impl Into<String>) {
        self.app.set_notice(Some(message.into()));
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

    /// Touch/narrow layout: below ~700 pt the desktop tab strip cannot fit.
    fn is_compact(ctx: &egui::Context) -> bool {
        ctx.screen_rect().width() < 700.0
    }

    fn spawn_action(&mut self, action: PendingAction, ctx: &egui::Context) {
        let app = Arc::clone(&self.app);
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
                        .update_share(share_id, Some(is_active), None, None)
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
                } => {
                    if let Some(share_id) = share_id {
                        let _ = app
                            .update_share(
                                share_id,
                                Some(enabled),
                                Some(precision),
                                Some(duration_hours),
                            )
                            .await;
                    } else if enabled {
                        let expiry = if duration_hours > 0 {
                            Some(duration_hours)
                        } else {
                            None
                        };
                        let _ = app
                            .create_share_with_expiry(
                                friendship_id,
                                "to_friend".to_string(),
                                precision,
                                expiry,
                                None,
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
                PendingAction::Locate => match app.locate() {
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
                        "Location unavailable — no GPS fix. Set PINPOINT_GPS_FIX or publish manually."
                            .to_string(),
                    )),
                },
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
                } => {
                    let mut body = serde_json::json!({});
                    if let Some(role) = role {
                        body["role"] = serde_json::Value::String(role);
                    }
                    if let Some(active) = is_active {
                        body["is_active"] = serde_json::Value::Bool(active);
                    }
                    let _ = app.admin_update_user(id, body).await;
                }
                PendingAction::AdminDeleteUser { id } => {
                    let _ = app.admin_delete_user(id).await;
                }
                PendingAction::AdminUpdateReport { id, status } => {
                    let _ = app.admin_update_report(id, status).await;
                }
            }
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
            ui.horizontal(|ui| {
                if auth_screen {
                    if ui.button("←").clicked() {
                        self.app.set_screen(CurrentScreen::Map);
                    }
                } else if ui.button("☰").clicked() {
                    self.mobile_nav_open = !self.mobile_nav_open;
                }
                ui.label(
                    RichText::new(i18n::t(lang, key::APP_TITLE))
                        .size(20.0)
                        .strong(),
                );
                if self.app.is_offline_mode() {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(i18n::t(lang, key::OFFLINE_MODE))
                            .size(10.0)
                            .strong()
                            .color(Color32::from_rgb(250, 204, 21)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if !auth_screen {
                        if signed_in {
                            if ui.button(i18n::t(lang, key::SIGN_OUT)).clicked() {
                                self.spawn_action(PendingAction::Logout, ctx);
                            }
                        } else if ui.button(i18n::t(lang, key::SIGN_IN)).clicked() {
                            self.app.set_screen(CurrentScreen::Login);
                        }
                    }
                    if self.app.is_loading() {
                        ui.spinner();
                    }
                });
            });
            if let Some(err) = self.app.error_message() {
                ui.colored_label(Color32::RED, err);
            } else if let Some(notice) = self.app.notice_message() {
                ui.colored_label(Color32::from_rgb(52, 211, 153), notice);
            }
            ui.separator();
            ui.add_space(4.0);
            return;
        }

        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading(i18n::t(lang, key::APP_TITLE));
                },
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
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
                ui.add_space(4.0);
                if signed_in {
                    if ui.button(i18n::t(lang, key::SIGN_OUT)).clicked() {
                        self.spawn_action(PendingAction::Logout, ctx);
                    }
                    if let Some(user) = &self.app.data().lock().unwrap().user {
                        ui.label(user.email.to_string());
                    }
                } else {
                    if ui.button(i18n::t(lang, key::SIGN_IN)).clicked() {
                        self.app.set_screen(CurrentScreen::Login);
                    }
                }
                ui.add_space(4.0);
                if self.app.is_offline_mode() {
                    ui.label(i18n::t(lang, key::OFFLINE_MODE));
                }
                if self.app.is_loading() {
                    ui.spinner();
                }
                if let Some(notice) = self.app.notice_message() {
                    ui.colored_label(Color32::from_rgb(52, 211, 153), notice);
                }
                if let Some(err) = self.app.error_message() {
                    ui.colored_label(Color32::RED, err);
                }
            });
        });
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
            for (screen, key_str) in screens {
                if screen == CurrentScreen::Admin && !is_admin {
                    continue;
                }
                if ui
                    .selectable_label(self.app.current_screen() == screen, i18n::t(lang, key_str))
                    .clicked()
                {
                    self.app.set_screen(screen);
                }
                ui.add_space(4.0);
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
        ui.add_space(12.0);
    }

    fn draw_sign_in_required(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, feature: &str) {
        let lang = self.lang;
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(i18n::t(lang, key::SIGN_IN_REQUIRED));
            ui.add_space(16.0);
            ui.label(i18n::t(lang, key::SIGN_IN_TO_USE).replace("{feature}", feature));
            ui.add_space(24.0);
            ui.group(|ui| {
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
                    let password = self.login_password.clone();
                    let totp = {
                        let code = self.login_totp.trim();
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
                tracing::info!("app lock: re-locked after returning to the foreground");
            }
        }
    }

    fn draw_lock_screen(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(11, 16, 32))
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
                            .color(Color32::from_rgb(148, 163, 184)),
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
                        ui.label(
                            RichText::new(format!("Try again in {retry_secs}s."))
                                .size(12.0)
                                .color(Color32::from_rgb(250, 204, 21)),
                        );
                    } else if let Some(error) = &self.pin_error {
                        ui.label(
                            RichText::new(error)
                                .size(12.0)
                                .color(Color32::from_rgb(248, 113, 113)),
                        );
                    }
                });
            });
    }

    fn attempt_unlock(&mut self) {
        if crate::app_lock::LockStore::default_location().verify(&self.pin_input) {
            self.locked = false;
            self.pin_input.clear();
            self.pin_error = None;
            self.pin_failures = 0;
            self.pin_retry_at = None;
            tracing::info!("app lock: unlocked");
            return;
        }
        self.pin_failures = self.pin_failures.saturating_add(1);
        let delay = 1u64 << self.pin_failures.min(5);
        self.pin_retry_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(delay));
        self.pin_error = Some(if self.pin_failures >= 3 {
            format!("Wrong PIN ({delay}s lockout).")
        } else {
            "Wrong PIN.".to_string()
        });
        self.pin_input.clear();
        self.pin_focus_requested = true;
    }
}

impl eframe::App for PinpointAppUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.lock_enabled {
            self.update_focus_lock(ctx);
            if self.locked {
                self.draw_lock_screen(ctx);
                return;
            }
        }
        if !self.crash_checked {
            self.crash_checked = true;
            if let Some(stored) = crate::crash::pending_report() {
                self.crash_dialog = Some(CrashDialog::from_pending(stored));
            }
        }
        self.sync_route_results();
        self.tick_navigation();
        if !self.session_restore_attempted {
            self.session_restore_attempted = true;
            if self.app.has_persisted_session() && !self.app.is_backend_unavailable() {
                self.spawn_action(PendingAction::RestorePersistedSession, ctx);
            }
        }
        if Self::is_compact(ctx) && self.mobile_selected_place.is_none() {
            if let Some(place) = self.app.data().lock().unwrap().selected_place.clone() {
                self.mobile_selected_place = Some(place);
            }
        }
        // Handle screenshot mode
        if self.app.screenshot_mode() && !self.app.screenshot_taken() {
            self.app.increment_screenshot_counter();

            if self.app.screenshot_counter() > 120 {
                if let Some(path) = self.app.screenshot_path().clone() {
                    // Create a representative PNG
                    let mut img = image::RgbaImage::new(1200, 800);
                    for (x, y, pixel) in img.enumerate_pixels_mut() {
                        let r = ((x as f32 / 1200.0) * 255.0) as u8;
                        let g = ((y as f32 / 800.0) * 255.0) as u8;
                        let b = 100u8;
                        *pixel = image::Rgba([r, g, b, 255]);
                    }

                    let _ = img.save(&path);

                    self.app.set_screenshot_taken(true);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
            }
        }

        let compact = Self::is_compact(ctx);
        let auth_screen = matches!(
            self.app.current_screen(),
            CurrentScreen::Login | CurrentScreen::Register
        );
        if compact && self.mobile_nav_open {
            let lang = self.lang;
            let current = self.app.current_screen();
            let mut chosen: Option<CurrentScreen> = None;
            let mut chosen_lang: Option<Lang> = None;
            let mut publish_requested = false;
            let mut refresh_requested = false;
            let (is_admin, update_available) = {
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
                (is_admin, data.update_status.is_available())
            };
            egui::TopBottomPanel::bottom("mobile_nav")
                .resizable(false)
                .exact_height(500.0)
                .frame(
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(8, 12, 24))
                        .inner_margin(egui::Margin::symmetric(12, 6)),
                )
                .show(ctx, |ui| {
                    let (handle, _) =
                        ui.allocate_exact_size(egui::vec2(42.0, 4.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(handle, 2.0, Color32::from_rgb(71, 85, 105));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Tools").size(22.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("×").clicked() {
                                chosen = Some(current.clone());
                            }
                        });
                    });
                    ui.label(
                        RichText::new("Everything Pinpoint can do, one tap away.")
                            .size(11.0)
                            .color(Color32::from_rgb(148, 163, 184)),
                    );
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let tile_width = (ui.available_width() - 8.0) / 2.0;
                        let accent = Color32::from_rgb(52, 211, 153);
                        ui.horizontal(|ui| {
                            for (label, icon, requested) in [
                                ("Publish", "+", &mut publish_requested),
                                ("Refresh", "↻", &mut refresh_requested),
                            ] {
                                let button = egui::Button::new(
                                    RichText::new(format!("{icon}\n{label}"))
                                        .size(14.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(Color32::from_rgb(24, 72, 58))
                                .stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(52, 211, 153)))
                                .corner_radius(egui::CornerRadius::same(14))
                                .min_size(egui::vec2(tile_width, 64.0));
                                if ui.add(button).clicked() {
                                    *requested = true;
                                }
                            }
                        });
                        ui.add_space(8.0);
                        let mut tools: Vec<(CurrentScreen, &str, &str, Color32)> = vec![
                            (
                                CurrentScreen::Map,
                                "Map",
                                "⌖",
                                Color32::from_rgb(26, 45, 74),
                            ),
                            (
                                CurrentScreen::Friends,
                                "Friends",
                                "👥",
                                Color32::from_rgb(55, 35, 70),
                            ),
                            (
                                CurrentScreen::Sharing,
                                "Share",
                                "◎",
                                Color32::from_rgb(24, 62, 54),
                            ),
                            (
                                CurrentScreen::Places,
                                "Places",
                                "⌖",
                                Color32::from_rgb(30, 58, 80),
                            ),
                            (
                                CurrentScreen::Bookmarks,
                                "Saved",
                                "★",
                                Color32::from_rgb(72, 56, 20),
                            ),
                            (
                                CurrentScreen::Trips,
                                "Trips",
                                "T",
                                Color32::from_rgb(38, 45, 82),
                            ),
                            (
                                CurrentScreen::Emergency,
                                "SOS",
                                "!",
                                Color32::from_rgb(86, 34, 34),
                            ),
                            (
                                CurrentScreen::Profile,
                                "Profile",
                                "P",
                                Color32::from_rgb(42, 48, 60),
                            ),
                            (
                                CurrentScreen::About,
                                if update_available {
                                    "Update!"
                                } else {
                                    "Updates"
                                },
                                "U",
                                if update_available {
                                    Color32::from_rgb(98, 62, 20)
                                } else {
                                    Color32::from_rgb(62, 40, 62)
                                },
                            ),
                            (
                                CurrentScreen::Admin,
                                "Admin",
                                "A",
                                Color32::from_rgb(70, 48, 24),
                            ),
                            (
                                CurrentScreen::Kiosk,
                                "Kiosk",
                                "K",
                                Color32::from_rgb(24, 58, 66),
                            ),
                            (
                                CurrentScreen::ChangePassword,
                                "Password",
                                "PW",
                                Color32::from_rgb(38, 42, 52),
                            ),
                        ];
                        if !is_admin {
                            tools.retain(|(screen, _, _, _)| *screen != CurrentScreen::Admin);
                        }
                        egui::Grid::new("mobile_tools_grid")
                            .num_columns(2)
                            .spacing([8.0, 8.0])
                            .show(ui, |ui| {
                                for (index, (screen, label, icon, fill)) in tools.iter().enumerate()
                                {
                                    let selected = current == *screen;
                                    let button = egui::Button::new(
                                        RichText::new(format!("{icon}\n{label}"))
                                            .size(13.0)
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(if selected { accent } else { *fill })
                                    .stroke(egui::Stroke::new(
                                        1.0_f32,
                                        if selected {
                                            accent
                                        } else {
                                            Color32::from_rgb(31, 41, 55)
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
                                        if ui
                                            .selectable_label(lang == *l, l.native_name())
                                            .clicked()
                                        {
                                            chosen_lang = Some(*l);
                                        }
                                    }
                                });
                        });
                    });
                });
            if let Some(screen) = chosen {
                self.app.set_screen(screen);
                self.mobile_nav_open = false;
            }
            if let Some(l) = chosen_lang {
                self.lang = l;
            }
            if publish_requested {
                self.app.set_screen(CurrentScreen::Map);
                self.mobile_publish_open = true;
                self.mobile_nav_open = false;
            }
            if refresh_requested {
                self.spawn_action(PendingAction::RefreshAll, ctx);
                self.mobile_nav_open = false;
            }
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
                        .fill(Color32::from_rgb(15, 23, 42))
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
                            (CurrentScreen::Sharing, "Share"),
                            (CurrentScreen::Bookmarks, "Saved"),
                            (CurrentScreen::Profile, "Me"),
                        ] {
                            let selected = current == screen;
                            let fill = if selected {
                                Color32::from_rgb(52, 211, 153)
                            } else {
                                Color32::from_rgb(15, 23, 42)
                            };
                            let text_color = if selected {
                                Color32::from_rgb(5, 46, 27)
                            } else {
                                Color32::from_rgb(229, 231, 235)
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
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(Color32::from_rgb(11, 16, 32)))
        } else {
            egui::CentralPanel::default()
        };
        panel.show(ctx, |ui| {
            self.draw_header_bar(ui, ctx);

            match self.app.current_screen() {
                CurrentScreen::Map => {
                    self.draw_main_toolbar(ui);
                    self.draw_map_screen(ui, ctx);
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
                        self.draw_main_toolbar(ui);
                        self.draw_friends_screen(ui, ctx);
                    } else {
                        self.draw_sign_in_required(ui, ctx, "Friends");
                    }
                }
                CurrentScreen::Sharing => {
                    if self.app.is_signed_in() {
                        self.draw_main_toolbar(ui);
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
                    self.draw_main_toolbar(ui);
                    self.draw_kiosk_screen(ui, ctx);
                }
                CurrentScreen::ChangePassword => {
                    if self.app.is_signed_in() {
                        self.draw_main_toolbar(ui);
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

        // Request repaint for animation/screenshot
        if self.app.screenshot_mode() && !self.app.screenshot_taken() {
            ctx.request_repaint();
        }
    }
}
