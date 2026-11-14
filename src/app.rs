use crate::backend::DynBackend;
use crate::models::*;
use crate::osm::{MapCapabilities, Route, SearchPlace};
use crate::patterns::state::StateContext;
use crate::security::{AuditLogger, KeyManager, RateLimiter};
use std::sync::{Arc, Mutex};

/// A mutation that failed because all backends were unavailable.
/// Queued locally and replayed when a backend comes back online.
#[derive(Debug, Clone)]
pub enum PendingMutation {
    PublishLocation {
        lat: f64,
        lon: f64,
        accuracy: f64,
    },
    CreateShare {
        friendship_id: i64,
        direction: String,
        precision: String,
    },
    UpdateShare {
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    },
    DeleteShare {
        share_id: i64,
    },
    CreateShareWithExpiry {
        friendship_id: i64,
        direction: String,
        precision: String,
        duration_hours: Option<i64>,
        label: Option<String>,
    },
    DeleteFriendship {
        id: i64,
    },
    SendFriendRequest {
        identifier: String,
    },
    RespondFriendship {
        id: i64,
        action: String,
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
    UpdateUser {
        id: i64,
        full_name: String,
    },
    ChangePassword {
        current_password: Option<String>,
        new_password: String,
        confirm_password: String,
    },
    MFAVerify {
        code: String,
    },
    MFADisable,
}

impl PendingMutation {
    /// Human-readable description for UI display.
    pub fn description(&self) -> &str {
        match self {
            PendingMutation::PublishLocation { .. } => "Publish location",
            PendingMutation::CreateShare { .. } => "Create location share",
            PendingMutation::UpdateShare { .. } => "Update location share",
            PendingMutation::DeleteShare { .. } => "Delete location share",
            PendingMutation::CreateShareWithExpiry { .. } => "Create share with expiry",
            PendingMutation::DeleteFriendship { .. } => "Delete friendship",
            PendingMutation::SendFriendRequest { .. } => "Send friend request",
            PendingMutation::RespondFriendship { .. } => "Respond to friendship",
            PendingMutation::CreateCheckIn { .. } => "Create check-in",
            PendingMutation::TriggerEmergency { .. } => "Trigger emergency",
            PendingMutation::UpdateUser { .. } => "Update profile",
            PendingMutation::ChangePassword { .. } => "Change password",
            PendingMutation::MFAVerify { .. } => "Verify MFA",
            PendingMutation::MFADisable => "Disable MFA",
        }
    }
}

/// Core application state/logic shared by desktop/mobile clients.
///
/// Depends on [`DynBackend`] (the modular backend abstraction) rather than a
/// concrete HTTP client, so changed backend architectures only require a new
/// adapter implementation.
/// Keychain entry for the opaque session cookie + CSRF snapshot.
const SESSION_SECRET: &str = "session_snapshot";

pub struct PinpointApp {
    data: Arc<Mutex<AppData>>,
    backend: DynBackend,
    is_loading: Arc<Mutex<bool>>,
    screenshot_path: Option<String>,
    screenshot_mode: bool,
    screenshot_counter: Arc<Mutex<i32>>,
    screenshot_taken: Arc<Mutex<bool>>,
    current_screen: Arc<Mutex<CurrentScreen>>,
    error_message: Arc<Mutex<Option<String>>>,
    notice_message: Arc<Mutex<Option<String>>>,
    offline_mode: Arc<Mutex<bool>>,
    /// True only when the mobile backend fell back to loopback because the
    /// configured URL was rejected. No persisted session or login is trusted.
    backend_unavailable: Arc<Mutex<bool>>,
    /// Mutations that failed because all backends were down.
    /// Replayed automatically when connectivity is restored.
    pending_mutations: Arc<Mutex<Vec<PendingMutation>>>,
    state_context: Arc<Mutex<StateContext>>,
    rate_limiter: Arc<Mutex<RateLimiter>>,
    audit_logger: Arc<AuditLogger>,
    key_manager: Arc<KeyManager>,
    /// Per-user E2E location encryption key (`e2e$<salt>$<iters>$<key>`),
    /// derived on login/register and persisted in the OS keychain.
    e2e_location_key: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum CurrentScreen {
    #[default]
    Map,
    Register,
    Login,
    Friends,
    Sharing,
    Places,
    Bookmarks,
    Trips,
    Emergency,
    Profile,
    About,
    Admin,
    ChangePassword,
    Kiosk,
}

impl PinpointApp {
    fn demo_locations() -> Vec<FriendLocation> {
        let now = "2026-11-08T12:00:00Z".to_string();
        vec![
            FriendLocation {
                user_id: 1001,
                user_name: "Demo: Ana".to_string(),
                latitude: 47.0105,
                longitude: 28.8638,
                precision_level: "exact".to_string(),
                updated_at: now.clone(),
                accuracy: Some(8.0),
            },
            FriendLocation {
                user_id: 1002,
                user_name: "Demo: Mihai".to_string(),
                latitude: 47.0056,
                longitude: 28.8575,
                precision_level: "approximate".to_string(),
                updated_at: now.clone(),
                accuracy: Some(250.0),
            },
            FriendLocation {
                user_id: 1003,
                user_name: "Demo: Elena".to_string(),
                latitude: 47.0240,
                longitude: 28.8320,
                precision_level: "city_only".to_string(),
                updated_at: now,
                accuracy: None,
            },
        ]
    }

    /// Create the app with any [`DynBackend`] implementation (REST v1 by default,
    /// swappable for changed backend architectures).
    pub fn new(backend: DynBackend, screenshot_path: Option<String>) -> Self {
        let screenshot_mode = screenshot_path.is_some();
        // Seed demo friend locations so the map is functional offline
        // before any server connection. These are example locations
        // around Chicago for demonstration purposes only.
        let data = AppData {
            locations: Self::demo_locations(),
            ..Default::default()
        };

        // Persist the active encryption key in the OS keychain when one is
        // available (Windows Credential Manager / macOS Keychain / Linux
        // Secret Service), falling back to an in-memory key otherwise.
        let key_manager =
            match crate::security::keychain::open_default(crate::platform::app_data_dir()) {
                Ok(kc) => KeyManager::with_keychain(kc),
                Err(_) => KeyManager::new(),
            };
        if key_manager.load_or_generate().is_none() {
            let _ = key_manager.generate_key();
        }
        let persisted_e2e_key = key_manager
            .load_secret("e2e_location_key")
            .and_then(|b| String::from_utf8(b).ok());

        Self {
            data: Arc::new(Mutex::new(data)),
            backend,
            is_loading: Arc::new(Mutex::new(false)),
            screenshot_path,
            screenshot_mode,
            screenshot_counter: Arc::new(Mutex::new(0)),
            screenshot_taken: Arc::new(Mutex::new(false)),
            current_screen: Arc::new(Mutex::new(CurrentScreen::Map)),
            error_message: Arc::new(Mutex::new(None)),
            notice_message: Arc::new(Mutex::new(None)),
            offline_mode: Arc::new(Mutex::new(true)),
            backend_unavailable: Arc::new(Mutex::new(false)),
            pending_mutations: Arc::new(Mutex::new(Vec::new())),
            state_context: Arc::new(Mutex::new(StateContext::new())),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::with_defaults())),
            audit_logger: Arc::new(AuditLogger::with_console()),
            key_manager: Arc::new(key_manager),
            e2e_location_key: Arc::new(Mutex::new(persisted_e2e_key)),
        }
    }

    /// Convenience constructor from the concrete REST client (kept for
    /// compatibility with existing call sites).
    pub fn new_rest(
        api_client: Arc<crate::api::ApiClient>,
        screenshot_path: Option<String>,
    ) -> Self {
        Self::new(api_client as DynBackend, screenshot_path)
    }

    pub fn data(&self) -> Arc<Mutex<AppData>> {
        Arc::clone(&self.data)
    }

    pub fn backend(&self) -> DynBackend {
        Arc::clone(&self.backend)
    }

    pub fn backend_url(&self) -> String {
        self.backend.backend_url().to_string()
    }

    /// Spawn the realtime polling loop (default ≤30s, settings-configurable,
    /// 0 disables via refresh_interval_secs — here clamped to ≥5s when on).
    /// One thread, never blocks the UI thread; only runs while signed in.
    pub fn start_auto_refresh(app: Arc<Self>) {
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return,
            };
            rt.block_on(async move {
                loop {
                    let interval = app
                        .data()
                        .lock()
                        .map(|d| d.settings.refresh_interval_secs)
                        .unwrap_or(30);
                    if interval == 0 {
                        // Polling disabled — idle.
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        continue;
                    }
                    let secs = interval.clamp(5, 3600);
                    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                    if app.is_signed_in() && !app.is_offline_mode() {
                        let _ = app.refresh_all().await;
                    }
                }
            });
        });
    }

    pub fn set_loading(&self, loading: bool) {
        *self.is_loading.lock().unwrap() = loading;
    }

    pub fn is_loading(&self) -> bool {
        *self.is_loading.lock().unwrap()
    }

    pub fn screenshot_path(&self) -> Option<String> {
        self.screenshot_path.clone()
    }

    pub fn screenshot_mode(&self) -> bool {
        self.screenshot_mode
    }

    pub fn screenshot_counter(&self) -> i32 {
        *self.screenshot_counter.lock().unwrap()
    }

    pub fn increment_screenshot_counter(&self) {
        *self.screenshot_counter.lock().unwrap() += 1;
    }

    pub fn screenshot_taken(&self) -> bool {
        *self.screenshot_taken.lock().unwrap()
    }

    pub fn set_screenshot_taken(&self, taken: bool) {
        *self.screenshot_taken.lock().unwrap() = taken;
    }

    pub fn current_screen(&self) -> CurrentScreen {
        self.current_screen.lock().unwrap().clone()
    }

    pub fn set_screen(&self, screen: CurrentScreen) {
        *self.current_screen.lock().unwrap() = screen.clone();
        let _ = self.state_context.lock().unwrap().set_state(screen);
    }

    pub fn state_context(&self) -> Arc<Mutex<StateContext>> {
        Arc::clone(&self.state_context)
    }

    pub fn rate_limiter(&self) -> Arc<Mutex<RateLimiter>> {
        Arc::clone(&self.rate_limiter)
    }

    pub fn audit_logger(&self) -> Arc<AuditLogger> {
        Arc::clone(&self.audit_logger)
    }

    pub fn key_manager(&self) -> Arc<KeyManager> {
        Arc::clone(&self.key_manager)
    }

    /// Returns the active E2E location encryption key (`e2e$<salt>$<iters>$<key>`), if any.
    pub fn e2e_location_key(&self) -> Option<String> {
        self.e2e_location_key.lock().unwrap().clone()
    }

    /// Derive a per-user E2E location key from `password`, store it in memory,
    /// and persist it in the OS/file keychain via `KeyManager`.
    pub fn activate_e2e_location_key(&self, password: &str) -> String {
        let key = crate::security::e2e::derive_location_key(password);
        self.key_manager
            .persist_secret("e2e_location_key", key.as_bytes());
        *self.e2e_location_key.lock().unwrap() = Some(key.clone());
        key
    }

    /// Encrypt `(lat, lon, accuracy)` with the active E2E location key.
    pub fn encrypt_my_location(&self, lat: f64, lon: f64, accuracy: f64) -> Option<String> {
        let key = self.e2e_location_key()?;
        crate::security::e2e::encrypt_location(&key, lat, lon, accuracy)
    }

    /// Decrypt an E2E location payload using the active E2E location key.
    pub fn decrypt_location_payload(&self, ciphertext: &str) -> Option<(f64, f64, f64)> {
        let key = self.e2e_location_key()?;
        crate::security::e2e::decrypt_location(&key, ciphertext)
    }

    pub fn is_signed_in(&self) -> bool {
        self.data.lock().unwrap().user.is_some()
    }

    pub fn is_offline_mode(&self) -> bool {
        *self.offline_mode.lock().unwrap()
    }

    pub fn set_offline_mode(&self, offline: bool) {
        *self.offline_mode.lock().unwrap() = offline;
    }

    /// A release fallback with no configured backend. Do not auto-restore
    /// sessions or accept credentials through the shared loopback endpoint.
    pub fn is_backend_unavailable(&self) -> bool {
        *self.backend_unavailable.lock().unwrap()
    }

    pub fn set_backend_unavailable(&self, unavailable: bool) {
        *self.backend_unavailable.lock().unwrap() = unavailable;
    }

    /// Returns the number of pending (unsynced) mutations.
    pub fn pending_mutation_count(&self) -> usize {
        self.pending_mutations.lock().unwrap().len()
    }

    /// Returns descriptions of all pending mutations (for UI display).
    pub fn pending_mutation_descriptions(&self) -> Vec<String> {
        self.pending_mutations
            .lock()
            .unwrap()
            .iter()
            .map(|m| m.description().to_string())
            .collect()
    }

    /// Queue a mutation for later replay when a backend becomes available.
    fn queue_mutation(&self, mutation: PendingMutation) {
        self.pending_mutations.lock().unwrap().push(mutation);
        self.set_offline_mode(true);
    }

    /// Replay all pending mutations against the current backend.
    /// Returns the number of mutations still pending after the sync attempt.
    pub async fn sync_pending_mutations(&self) -> usize {
        let pending = {
            let mut guard = self.pending_mutations.lock().unwrap();
            let count = guard.len();
            if count == 0 {
                return 0;
            }
            // Clone the list, clear the queue, replay one by one.
            let mutations = guard.clone();
            guard.clear();
            mutations
        };

        let mut still_pending = Vec::new();
        for mutation in pending {
            let result = self.replay_mutation(&mutation).await;
            if result.is_err() {
                // Put it back at the front of the queue.
                still_pending.push(mutation);
            }
        }

        let remaining = still_pending.len();
        if remaining > 0 {
            let mut guard = self.pending_mutations.lock().unwrap();
            guard.extend(still_pending);
        }

        remaining
    }

    /// Replay a single pending mutation.
    async fn replay_mutation(&self, mutation: &PendingMutation) -> anyhow::Result<()> {
        match mutation {
            PendingMutation::PublishLocation { lat, lon, accuracy } => {
                self.backend.publish_location(*lat, *lon, *accuracy).await?;
                self.refresh_friend_locations().await?;
                self.refresh_my_location().await?;
            }
            PendingMutation::CreateShare {
                friendship_id,
                direction,
                precision,
            } => {
                self.backend
                    .create_location_share(*friendship_id, direction, precision)
                    .await?;
                self.refresh_shares().await?;
            }
            PendingMutation::UpdateShare {
                share_id,
                is_active,
                precision,
                duration_hours,
            } => {
                self.backend
                    .update_location_share(
                        *share_id,
                        *is_active,
                        precision.clone(),
                        *duration_hours,
                    )
                    .await?;
                self.refresh_shares().await?;
            }
            PendingMutation::DeleteShare { share_id } => {
                self.backend.delete_share(*share_id).await?;
                self.refresh_shares().await?;
            }
            PendingMutation::CreateShareWithExpiry {
                friendship_id,
                direction,
                precision,
                duration_hours,
                label,
            } => {
                self.backend
                    .create_share_with_expiry(
                        *friendship_id,
                        direction,
                        precision,
                        *duration_hours,
                        label.as_deref(),
                    )
                    .await?;
                self.refresh_shares().await?;
            }
            PendingMutation::DeleteFriendship { id } => {
                self.backend.delete_friendship(*id).await?;
                self.refresh_friendships().await?;
                self.refresh_shares().await?;
            }
            PendingMutation::SendFriendRequest { identifier } => {
                self.backend.send_friend_request(identifier).await?;
                self.refresh_friendships().await?;
            }
            PendingMutation::RespondFriendship { id, action } => {
                self.backend.respond_to_friendship(*id, action).await?;
                self.refresh_friendships().await?;
                self.refresh_shares().await?;
            }
            PendingMutation::CreateCheckIn {
                lat,
                lon,
                note,
                kind,
            } => {
                self.backend.create_check_in(*lat, *lon, note, kind).await?;
                self.refresh_check_ins().await?;
            }
            PendingMutation::TriggerEmergency { lat, lon } => {
                self.backend.trigger_emergency(*lat, *lon).await?;
                self.refresh_emergencies().await?;
                self.refresh_shares().await?;
            }
            PendingMutation::UpdateUser { id, full_name } => {
                self.backend
                    .update_user(*id, serde_json::json!({ "full_name": full_name }))
                    .await?;
                self.refresh_users().await?;
            }
            PendingMutation::ChangePassword {
                current_password,
                new_password,
                confirm_password,
            } => {
                self.backend
                    .change_password(current_password.as_deref(), new_password, confirm_password)
                    .await?;
            }
            PendingMutation::MFAVerify { code } => {
                self.backend.mfa_verify(code).await?;
            }
            PendingMutation::MFADisable => {
                self.backend.mfa_disable().await?;
            }
        }
        Ok(())
    }

    pub fn requires_sign_in(&self, screen: CurrentScreen) -> bool {
        self.state_context
            .lock()
            .unwrap()
            .get_state(screen.clone())
            .map(|s| s.requires_auth())
            .unwrap_or(matches!(
                screen,
                CurrentScreen::Friends
                    | CurrentScreen::Sharing
                    | CurrentScreen::Admin
                    | CurrentScreen::ChangePassword
            ))
    }

    pub fn error_message(&self) -> Option<String> {
        self.error_message.lock().unwrap().clone()
    }

    pub fn set_error(&self, msg: Option<String>) {
        if msg.is_some() {
            *self.notice_message.lock().unwrap() = None;
        }
        *self.error_message.lock().unwrap() = msg;
    }

    pub fn notice_message(&self) -> Option<String> {
        self.notice_message.lock().unwrap().clone()
    }

    pub fn set_notice(&self, msg: Option<String>) {
        if msg.is_some() {
            *self.error_message.lock().unwrap() = None;
        }
        *self.notice_message.lock().unwrap() = msg;
    }

    /// Applies a successful login/register: records the user, chooses the
    /// initial screen (forced password change first when the server
    /// demands it) and refreshes the visible data.
    fn apply_signed_in(&self, user: User) {
        let forced_change = user.must_change_password;
        {
            let mut data = self.data.lock().unwrap();
            data.state = if user.role == "admin" {
                AppState::Admin
            } else {
                AppState::Main
            };
            data.must_change_password = forced_change;
            data.user = Some(user);
        }
        self.persist_session();
        if forced_change {
            self.set_screen(CurrentScreen::ChangePassword);
        } else {
            self.set_screen(CurrentScreen::Map);
        }
    }

    pub async fn login(
        &self,
        email: String,
        password: String,
        totp: Option<String>,
    ) -> anyhow::Result<()> {
        let email_valid = crate::security::validation::validate_email(&email);
        if !email_valid.is_valid() {
            self.set_error(Some(email_valid.errors.join("; ")));
            return Err(anyhow::anyhow!(email_valid.errors.join("; ")));
        }
        if !self
            .rate_limiter
            .lock()
            .unwrap()
            .check("api_login")
            .is_allowed()
        {
            self.set_error(Some("Rate limited — try again shortly.".to_string()));
            return Err(anyhow::anyhow!("Rate limited — try again shortly."));
        }
        self.set_loading(true);
        self.set_error(None);

        let result = self
            .backend
            .login_with_totp(&email, &password, totp.as_deref())
            .await;

        self.set_loading(false);

        match result {
            Ok(user) => {
                self.audit_logger
                    .log_event("auth", "login", Some(&email), "success");
                self.activate_e2e_location_key(&password);
                self.set_offline_mode(false);
                self.apply_signed_in(user);
                if self.current_screen() == CurrentScreen::Map {
                    self.refresh_all().await;
                }
                Ok(())
            }
            Err(e) => {
                self.audit_logger
                    .log_event("auth", "login", Some(&email), "failure");
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    pub async fn register(
        &self,
        email: String,
        password: String,
        full_name: String,
    ) -> anyhow::Result<()> {
        for v in [
            crate::security::validation::validate_email(&email),
            crate::security::validation::validate_password(&password),
            crate::security::validation::validate_name(&full_name),
        ] {
            if !v.is_valid() {
                self.set_error(Some(v.errors.join("; ")));
                return Err(anyhow::anyhow!(v.errors.join("; ")));
            }
        }
        self.set_loading(true);
        self.set_error(None);

        let result = self.backend.register(&email, &password, &full_name).await;

        self.set_loading(false);

        match result {
            Ok(user) => {
                self.audit_logger
                    .log_event("auth", "register", Some(&email), "success");
                self.activate_e2e_location_key(&password);
                self.set_offline_mode(false);
                self.apply_signed_in(user);
                if self.current_screen() == CurrentScreen::Map {
                    self.refresh_all().await;
                }
                Ok(())
            }
            Err(e) => {
                self.audit_logger
                    .log_event("auth", "register", Some(&email), "failure");
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Logs out only after the server confirms the revocation; a failed
    /// server-side logout keeps local state and surfaces the error so the
    /// UI never claims a logout that did not happen.
    pub async fn logout(&self) -> anyhow::Result<()> {
        self.set_loading(true);
        let result = self.backend.logout().await;
        self.set_loading(false);

        match result {
            Ok(()) => {
                self.audit_logger
                    .log_event("auth", "logout", None, "success");
                *self.e2e_location_key.lock().unwrap() = None;
                self.key_manager.delete_secret("e2e_location_key");
                self.clear_persisted_session();
                let fresh = AppData {
                    locations: Self::demo_locations(),
                    ..Default::default()
                };
                *self.data.lock().unwrap() = fresh;
                self.set_screen(CurrentScreen::Map);
                self.set_offline_mode(true);
                self.set_notice(Some("Signed out.".to_string()));
                Ok(())
            }
            Err(e) => {
                self.audit_logger
                    .log_event("auth", "logout", None, "failure");
                self.set_error(Some(format!("Logout failed: {e}")));
                Err(e)
            }
        }
    }

    pub fn has_persisted_session(&self) -> bool {
        self.key_manager.load_secret(SESSION_SECRET).is_some()
    }

    fn persist_session(&self) {
        if let Some(raw) = self.backend.export_session() {
            self.key_manager
                .persist_secret(SESSION_SECRET, raw.as_bytes());
        }
    }

    fn clear_persisted_session(&self) {
        self.key_manager.delete_secret(SESSION_SECRET);
    }

    /// Restore the opaque cookie snapshot and validate it with `/auth/me`.
    pub async fn restore_persisted_session(&self) -> anyhow::Result<()> {
        let raw = self
            .key_manager
            .load_secret(SESSION_SECRET)
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(|| anyhow::anyhow!("no persisted session"))?;
        self.backend.import_session(&raw)?;
        match self.restore_session().await {
            Ok(()) => {
                self.persist_session();
                Ok(())
            }
            Err(e) => {
                self.clear_persisted_session();
                self.set_screen(CurrentScreen::Login);
                Err(e)
            }
        }
    }

    /// Restore a persisted session by asking `/auth/me`; the caller decides
    /// whether to show the login screen on failure.
    pub async fn restore_session(&self) -> anyhow::Result<()> {
        self.set_loading(true);
        let result = self.backend.me().await;
        self.set_loading(false);
        match result {
            Ok(user) => {
                self.set_offline_mode(false);
                self.apply_signed_in(user);
                if self.current_screen() == CurrentScreen::Map {
                    self.refresh_all().await;
                }
                Ok(())
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Create a device-bound anonymous account and return its one-time
    /// recovery code. The code is never logged.
    pub async fn sign_in_anonymous(&self, device_name: String) -> anyhow::Result<String> {
        self.set_loading(true);
        self.set_error(None);
        let result = self.backend.anonymous(&device_name).await;
        self.set_loading(false);
        match result {
            Ok((user, code)) => {
                self.set_offline_mode(false);
                self.apply_signed_in(user);
                self.refresh_all().await;
                Ok(code)
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    /// Restore an anonymous account with a recovery code; the server rotates
    /// the code and returns the new one.
    pub async fn recover_anonymous(&self, code: String) -> anyhow::Result<String> {
        self.set_loading(true);
        self.set_error(None);
        let result = self.backend.recover_account(&code).await;
        self.set_loading(false);
        match result {
            Ok((user, new_code)) => {
                self.set_offline_mode(false);
                self.apply_signed_in(user);
                self.refresh_all().await;
                Ok(new_code)
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    pub async fn refresh_sessions(&self) -> anyhow::Result<()> {
        let sessions = self.backend.list_sessions().await?;
        self.data.lock().unwrap().sessions = sessions;
        Ok(())
    }

    pub async fn revoke_session(&self, id: String) -> anyhow::Result<()> {
        self.backend.revoke_session(&id).await?;
        self.refresh_sessions().await
    }

    /// Revoke every server session. Local state is cleared even when the
    /// network call fails, matching the web client's fail-safe behavior.
    pub async fn logout_all(&self) -> anyhow::Result<()> {
        let result = self.backend.logout_all().await;
        *self.e2e_location_key.lock().unwrap() = None;
        self.key_manager.delete_secret("e2e_location_key");
        self.clear_persisted_session();
        let fresh = AppData {
            locations: Self::demo_locations(),
            ..Default::default()
        };
        *self.data.lock().unwrap() = fresh;
        self.set_screen(CurrentScreen::Map);
        self.set_offline_mode(true);
        match result {
            Ok(()) => {
                self.set_notice(Some("Signed out on every device.".to_string()));
                Ok(())
            }
            Err(e) => {
                self.set_error(Some(format!(
                    "Signed out locally; server revoke failed: {e}"
                )));
                Err(e)
            }
        }
    }

    pub async fn submit_diagnostic(
        &self,
        category: String,
        message: String,
    ) -> anyhow::Result<i64> {
        let platform = crate::platform::Platform::current().label().to_lowercase();
        self.backend
            .submit_diagnostic(&category, &message, &platform)
            .await
    }

    /// Changes the password; returns the user to the login screen when the
    /// server revoked sessions (regular change), otherwise keeps the
    /// session (forced change).
    pub async fn change_password(
        &self,
        current_password: Option<String>,
        new_password: String,
        confirm_password: String,
    ) -> anyhow::Result<()> {
        self.set_loading(true);
        self.set_error(None);

        let result = self
            .backend
            .change_password(
                current_password.as_deref(),
                &new_password,
                &confirm_password,
            )
            .await;

        self.set_loading(false);

        match result {
            Ok(sessions_revoked) => {
                if sessions_revoked {
                    self.clear_persisted_session();
                    let fresh = AppData {
                        locations: Self::demo_locations(),
                        ..Default::default()
                    };
                    *self.data.lock().unwrap() = fresh;
                    self.set_screen(CurrentScreen::Map);
                    self.set_offline_mode(true);
                    self.set_notice(Some(
                        "Password changed. Your other sessions were signed out; please sign in again."
                            .to_string(),
                    ));
                } else {
                    {
                        let mut data = self.data.lock().unwrap();
                        data.must_change_password = false;
                    }
                    self.set_screen(CurrentScreen::Map);
                    self.set_notice(Some("Password changed.".to_string()));
                    self.refresh_all().await;
                }
                Ok(())
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::ChangePassword {
                    current_password,
                    new_password,
                    confirm_password,
                });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn refresh_all(&self) {
        let screen = self.current_screen.lock().unwrap().clone();
        if screen == CurrentScreen::Login || screen == CurrentScreen::Register {
            return;
        }
        // Surface the first refresh failure instead of silently dropping it.
        if let Err(e) = self.refresh_friendships().await {
            self.set_error(Some(e.to_string()));
            return;
        }
        if let Err(e) = self.refresh_shares().await {
            self.set_error(Some(e.to_string()));
            return;
        }
        if let Err(e) = self.refresh_friend_locations().await {
            self.set_error(Some(e.to_string()));
        }
    }

    pub async fn refresh_friendships(&self) -> anyhow::Result<()> {
        let friendships = self.backend.get_friendships().await?;
        let mut data = self.data.lock().unwrap();
        data.friendships = friendships;
        Ok(())
    }

    pub async fn refresh_shares(&self) -> anyhow::Result<()> {
        let shares = self.backend.get_location_shares().await?;
        let mut data = self.data.lock().unwrap();
        data.shares = shares;
        Ok(())
    }

    pub async fn refresh_friend_locations(&self) -> anyhow::Result<()> {
        let locations = self.backend.get_friend_locations().await?;
        let mut data = self.data.lock().unwrap();
        data.locations = locations;
        Ok(())
    }

    pub async fn send_friend_request(&self, identifier: String) -> anyhow::Result<()> {
        let identifier = identifier.trim().trim_start_matches('@').to_string();
        if identifier.is_empty() {
            self.set_error(Some("Enter a username or email.".to_string()));
            return Err(anyhow::anyhow!("Enter a username or email."));
        }
        if identifier.contains('@') {
            let v = crate::security::validation::validate_email(&identifier);
            if !v.is_valid() {
                self.set_error(Some(v.errors.join("; ")));
                return Err(anyhow::anyhow!(v.errors.join("; ")));
            }
        } else if identifier.len() > 64
            || !identifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            self.set_error(Some(
                "Usernames look like calm-otter-42 (letters, digits, dashes).".to_string(),
            ));
            return Err(anyhow::anyhow!("Invalid username."));
        }
        self.set_loading(true);
        self.set_error(None);
        let result = self.backend.send_friend_request(&identifier).await;
        self.set_loading(false);
        if result.is_ok() {
            self.audit_logger
                .log_event("friends", "send_request", Some(&identifier), "success");
            if let Err(e) = self.refresh_friendships().await {
                self.set_error(Some(e.to_string()));
            }
        } else {
            self.audit_logger
                .log_event("friends", "send_request", Some(&identifier), "failure");
            self.queue_mutation(PendingMutation::SendFriendRequest { identifier });
            self.set_error(Some(format!(
                "All backends unavailable — queued for sync ({} pending)",
                self.pending_mutation_count()
            )));
        }
        result
    }

    pub async fn respond_friendship(&self, id: i64, action: String) -> anyhow::Result<()> {
        self.set_loading(true);
        let result = self.backend.respond_to_friendship(id, &action).await;
        self.set_loading(false);
        match &result {
            Ok(()) => {
                if let Err(e) = self.refresh_friendships().await {
                    self.set_error(Some(e.to_string()));
                } else if let Err(e) = self.refresh_shares().await {
                    self.set_error(Some(e.to_string()));
                }
            }
            Err(_e) => {
                self.queue_mutation(PendingMutation::RespondFriendship { id, action });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
            }
        }
        result
    }

    pub async fn create_share(
        &self,
        friendship_id: i64,
        direction: String,
        precision: String,
    ) -> anyhow::Result<()> {
        self.set_loading(true);
        let result = self
            .backend
            .create_location_share(friendship_id, &direction, &precision)
            .await;
        self.set_loading(false);
        match &result {
            Ok(()) => {
                if let Err(e) = self.refresh_shares().await {
                    self.set_error(Some(e.to_string()));
                }
            }
            Err(_e) => {
                self.queue_mutation(PendingMutation::CreateShare {
                    friendship_id,
                    direction,
                    precision,
                });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
            }
        }
        result
    }

    pub async fn update_share(
        &self,
        share_id: i64,
        is_active: Option<bool>,
        precision: Option<String>,
        duration_hours: Option<i64>,
    ) -> anyhow::Result<()> {
        self.set_loading(true);
        let result = self
            .backend
            .update_location_share(share_id, is_active, precision.clone(), duration_hours)
            .await;
        self.set_loading(false);
        match &result {
            Ok(()) => {
                if let Err(e) = self.refresh_shares().await {
                    self.set_error(Some(e.to_string()));
                }
            }
            Err(_e) => {
                self.queue_mutation(PendingMutation::UpdateShare {
                    share_id,
                    is_active,
                    precision,
                    duration_hours,
                });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
            }
        }
        result
    }

    /// Read the current device fix without publishing it. The caller decides
    /// whether to show a self marker, centre the map, or publish.
    pub fn locate(&self) -> Option<(f64, f64, f64)> {
        crate::platform::default_gps_provider().current_position()
    }

    /// Publish the device's current GPS fix (if any). Returns an error
    /// when no GPS provider is available (desktop / unsupported target)
    /// or the fix cannot be obtained, so callers can fall back to a
    /// manual location.
    pub async fn publish_my_location(&self) -> anyhow::Result<()> {
        match crate::platform::default_gps_provider().current_position() {
            Some((lat, lon, accuracy)) => self.publish_location(lat, lon, accuracy).await,
            None => {
                self.set_error(Some(
                    "No GPS fix available. Wait for a location fix (or open a maps app once) and try again."
                        .to_string(),
                ));
                Err(anyhow::anyhow!("No GPS fix available."))
            }
        }
    }

    pub async fn publish_location(&self, lat: f64, lon: f64, accuracy: f64) -> anyhow::Result<()> {
        let v = crate::security::validation::validate_location(lat, lon, Some(accuracy));
        if !v.is_valid() {
            self.set_error(Some(v.errors.join("; ")));
            return Err(anyhow::anyhow!(v.errors.join("; ")));
        }
        if !self
            .rate_limiter
            .lock()
            .unwrap()
            .check("api_general")
            .is_allowed()
        {
            self.set_error(Some("Rate limited — try again shortly.".to_string()));
            return Err(anyhow::anyhow!("Rate limited — try again shortly."));
        }
        // Produce client-side AES-256-GCM E2E ciphertext before publishing
        // whenever an E2E location key is active (derived at login/register
        // or restored from the OS keychain).
        if let Some(ciphertext) = self.encrypt_my_location(lat, lon, accuracy) {
            self.data.lock().unwrap().last_e2e_location_ciphertext = Some(ciphertext);
            self.audit_logger
                .log_event("location", "e2e_encrypt", None, "success");
        }
        let result = self.backend.publish_location(lat, lon, accuracy).await;
        match result {
            Ok(()) => {
                self.audit_logger
                    .log_event("location", "publish", None, "success");
                self.set_notice(Some("Location published.".to_string()));
                if let Err(e) = self.refresh_friend_locations().await {
                    self.set_error(Some(e.to_string()));
                }
                if let Err(e) = self.refresh_my_location().await {
                    self.set_error(Some(e.to_string()));
                }
            }
            Err(e) => {
                // Queue for replay when a backend comes back.
                self.queue_mutation(PendingMutation::PublishLocation { lat, lon, accuracy });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                return Err(e);
            }
        }
        Ok(())
    }

    // ---- Maximum-feature additions ----
    pub async fn refresh_users(&self) -> anyhow::Result<()> {
        let users = self.backend.list_users().await?;
        self.data.lock().unwrap().users = users;
        Ok(())
    }

    pub async fn admin_load_overview(&self) -> anyhow::Result<()> {
        let overview = self.backend.admin_overview().await?;
        self.data.lock().unwrap().admin_overview = Some(overview);
        Ok(())
    }

    pub async fn admin_load_users(&self) -> anyhow::Result<()> {
        let users = self.backend.admin_users().await?;
        self.data.lock().unwrap().admin_users = users;
        Ok(())
    }

    pub async fn admin_update_user(&self, id: i64, body: serde_json::Value) -> anyhow::Result<()> {
        self.backend.admin_update_user(id, body).await?;
        self.admin_load_users().await
    }

    pub async fn admin_delete_user(&self, id: i64) -> anyhow::Result<()> {
        self.backend.admin_delete_user(id).await?;
        self.admin_load_users().await
    }

    pub async fn admin_load_diagnostics(&self, status: Option<String>) -> anyhow::Result<()> {
        let (reports, total, open) = self.backend.admin_diagnostics(status.as_deref()).await?;
        let data = self.data();
        let mut d = data.lock().unwrap();
        d.admin_reports = reports;
        d.admin_report_total = total;
        d.admin_report_open = open;
        Ok(())
    }

    pub async fn admin_update_report(&self, id: i64, status: String) -> anyhow::Result<()> {
        self.backend.admin_update_report(id, &status).await?;
        self.admin_load_diagnostics(None).await
    }

    pub async fn admin_load_audit(&self) -> anyhow::Result<()> {
        let entries = self.backend.admin_audit().await?;
        self.data.lock().unwrap().admin_audit = entries;
        Ok(())
    }

    pub async fn refresh_my_location(&self) -> anyhow::Result<()> {
        match self.backend.get_my_location().await {
            Ok(loc) => {
                self.data.lock().unwrap().my_location = Some(loc);
                Ok(())
            }
            Err(_) => Ok(()), // 404 when never published — not an error
        }
    }

    pub async fn map_capabilities(&self) -> anyhow::Result<MapCapabilities> {
        self.backend.map_capabilities().await
    }

    pub async fn map_search(
        &self,
        query: &str,
        lat: Option<f64>,
        lon: Option<f64>,
        limit: usize,
    ) -> anyhow::Result<Vec<SearchPlace>> {
        self.backend.map_search(query, lat, lon, limit).await
    }

    pub async fn map_reverse(&self, lat: f64, lon: f64) -> anyhow::Result<SearchPlace> {
        self.backend.map_reverse(lat, lon).await
    }

    pub async fn map_route(
        &self,
        from: (f64, f64),
        to: (f64, f64),
        mode: &str,
        via: &[(f64, f64)],
        avoid: &[String],
        alternatives: bool,
    ) -> anyhow::Result<Vec<Route>> {
        self.backend
            .map_route(from, to, mode, via, avoid, alternatives)
            .await
    }

    pub async fn refresh_check_ins(&self) -> anyhow::Result<()> {
        self.data.lock().unwrap().check_ins = self.backend.list_check_ins().await?;
        Ok(())
    }

    pub async fn refresh_emergencies(&self) -> anyhow::Result<()> {
        self.data.lock().unwrap().emergencies = self.backend.list_emergencies().await?;
        Ok(())
    }

    pub async fn delete_share(&self, share_id: i64) -> anyhow::Result<()> {
        let result = self.backend.delete_share(share_id).await;
        match result {
            Ok(()) => {
                self.audit_logger
                    .log_event("shares", "delete", None, "success");
                self.refresh_shares().await
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::DeleteShare { share_id });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn create_share_with_expiry(
        &self,
        friendship_id: i64,
        direction: String,
        precision: String,
        duration_hours: Option<i64>,
        label: Option<String>,
    ) -> anyhow::Result<()> {
        let result = self
            .backend
            .create_share_with_expiry(
                friendship_id,
                &direction,
                &precision,
                duration_hours,
                label.as_deref(),
            )
            .await;
        match result {
            Ok(()) => self.refresh_shares().await,
            Err(e) => {
                self.queue_mutation(PendingMutation::CreateShareWithExpiry {
                    friendship_id,
                    direction,
                    precision,
                    duration_hours,
                    label,
                });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn delete_friendship(&self, id: i64) -> anyhow::Result<()> {
        let result = self.backend.delete_friendship(id).await;
        match result {
            Ok(()) => {
                self.refresh_friendships().await?;
                self.refresh_shares().await
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::DeleteFriendship { id });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn create_check_in(
        &self,
        lat: f64,
        lon: f64,
        note: String,
        kind: String,
    ) -> anyhow::Result<()> {
        let result = self.backend.create_check_in(lat, lon, &note, &kind).await;
        match result {
            Ok(()) => {
                self.audit_logger
                    .log_event("checkins", "create", None, "success");
                self.refresh_check_ins().await
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::CreateCheckIn {
                    lat,
                    lon,
                    note,
                    kind,
                });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn trigger_emergency(&self, lat: f64, lon: f64) -> anyhow::Result<()> {
        let result = self.backend.trigger_emergency(lat, lon).await;
        match result {
            Ok(()) => {
                self.audit_logger
                    .log_event("emergency", "trigger", None, "success");
                self.set_notice(Some(
                    "Emergency alert sent — all shares activated.".to_string(),
                ));
                self.refresh_emergencies().await?;
                self.refresh_shares().await
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::TriggerEmergency { lat, lon });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn update_profile(&self, full_name: String) -> anyhow::Result<()> {
        let v = crate::security::validation::validate_name(&full_name);
        if !v.is_valid() {
            self.set_error(Some(v.errors.join("; ")));
            return Err(anyhow::anyhow!(v.errors.join("; ")));
        }
        let my_id = self.data.lock().unwrap().user.as_ref().map(|u| u.id);
        let Some(id) = my_id else {
            self.set_error(Some("Sign in required".to_string()));
            return Err(anyhow::anyhow!("Sign in required"));
        };
        let result = self
            .backend
            .update_user(id, serde_json::json!({ "full_name": full_name }))
            .await;
        match result {
            Ok(()) => self.refresh_users().await,
            Err(e) => {
                self.queue_mutation(PendingMutation::UpdateUser { id, full_name });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn mfa_enroll(&self) -> anyhow::Result<()> {
        let result = self.backend.mfa_enroll().await;
        match result {
            Ok((secret, _uri)) => {
                self.data.lock().unwrap().totp_pending_secret = Some(secret);
                Ok(())
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }

    pub async fn mfa_verify(&self, code: String) -> anyhow::Result<()> {
        let result = self.backend.mfa_verify(code.trim()).await;
        match result {
            Ok(()) => {
                let mut data = self.data.lock().unwrap();
                data.mfa_enabled = true;
                data.totp_pending_secret = None;
                Ok(())
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::MFAVerify { code });
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn mfa_disable(&self) -> anyhow::Result<()> {
        let result = self.backend.mfa_disable().await;
        match result {
            Ok(()) => {
                let mut data = self.data.lock().unwrap();
                data.mfa_enabled = false;
                data.totp_pending_secret = None;
                Ok(())
            }
            Err(e) => {
                self.queue_mutation(PendingMutation::MFADisable);
                self.set_error(Some(format!(
                    "All backends unavailable — queued for sync ({} pending)",
                    self.pending_mutation_count()
                )));
                Err(e)
            }
        }
    }

    pub async fn refresh_mfa_status(&self) -> anyhow::Result<()> {
        match self.backend.mfa_status().await {
            Ok(enabled) => {
                self.data.lock().unwrap().mfa_enabled = enabled;
                Ok(())
            }
            Err(e) => {
                self.set_error(Some(e.to_string()));
                Err(e)
            }
        }
    }
}
