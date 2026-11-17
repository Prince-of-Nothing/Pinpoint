//! Profile and settings screen (incl. MFA)
use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, RichText};
use std::sync::Arc;

impl PinpointAppUI {
    /// Backend URL editor shared by Settings and the sign-in screen.
    pub fn draw_server_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("Current: {}", self.app.backend_url()));
        ui.add(
            egui::TextEdit::singleline(&mut self.server_url_input)
                .hint_text("https://pinpoint-uj4y.onrender.com")
                .desired_width(300.0),
        );
        ui.horizontal_wrapped(|ui| {
            if theme::primary_button(ui, "Save & reconnect").clicked() {
                let url = self.server_url_input.clone();
                match self.switch_backend(&url) {
                    Ok(()) => self
                        .app
                        .set_notice(Some(format!("Connected to {}", self.app.backend_url()))),
                    Err(error) => self.server_error = Some(error),
                }
            }
            if ui.button("Reset to default").clicked() {
                match self.switch_backend("") {
                    Ok(()) => self
                        .app
                        .set_notice(Some(format!("Connected to {}", self.app.backend_url()))),
                    Err(error) => self.server_error = Some(error),
                }
            }
        });
        if let Some(error) = &self.server_error {
            ui.colored_label(theme::DANGER, error);
        }
        ui.label(
            RichText::new(
                "HTTPS is required for any server except localhost. Changing the server signs you out.",
            )
            .size(10.0)
            .color(theme::MUTED),
        );
    }

    pub fn draw_profile_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.sessions_requested {
            self.sessions_requested = true;
            self.spawn_action(PendingAction::RefreshSessions, ctx);
        }
        ui.heading("Profile & settings");
        ui.add_space(8.0);
        let binding = self.app.data();
        let data = binding.lock().unwrap();
        let (email, name, username) = match &data.user {
            Some(u) => (u.email.clone(), u.full_name.clone(), u.username.clone()),
            None => ("—".to_string(), String::new(), String::new()),
        };
        let metric = data.settings.metric;
        drop(data);
        ui.label(format!("Signed in as: {email}"));
        if !username.is_empty() {
            ui.horizontal(|ui| {
                ui.label(format!("Public username: @{username}"));
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(username.clone());
                }
            });
            ui.label("Share this handle so friends can add you; names and emails stay private.");
        }
        theme::card(ui, |ui| {
            theme::card_title(ui, "Display name");
            ui.add(
                egui::TextEdit::singleline(&mut self.profile_name)
                    .hint_text(&name)
                    .desired_width(220.0),
            );
            if theme::primary_button(ui, "Save name").clicked()
                && !self.profile_name.trim().is_empty()
            {
                self.spawn_action(
                    PendingAction::UpdateProfile {
                        full_name: self.profile_name.clone(),
                    },
                    ctx,
                );
            }
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Server");
            self.draw_server_settings(ui);
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Units");
            ui.selectable_value(
                &mut self.app.data().lock().unwrap().settings.metric,
                true,
                "Metric (km/m)",
            );
            ui.selectable_value(
                &mut self.app.data().lock().unwrap().settings.metric,
                false,
                "Imperial (mi/ft)",
            );
            let _ = metric;
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Updates");
            let mut check_on_launch = self.prefs.update_check_on_launch;
            if ui
                .checkbox(
                    &mut check_on_launch,
                    "Check GitHub for a newer release at launch",
                )
                .changed()
            {
                self.prefs.update_check_on_launch = check_on_launch;
                self.save_prefs();
            }
            ui.label(
                RichText::new(
                    "At most once a day. Only GitHub's public releases feed is requested; no \
                     account, location or friend data is sent.",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
        });
        ui.add_space(8.0);
        #[cfg(target_os = "android")]
        {
            theme::card(ui, |ui| {
                theme::card_title(ui, "Background sharing");
                ui.label(
                    "Publishes your latest location on a fixed interval while Pinpoint is in \
                     the background. A persistent notification stays visible; stop it here or \
                     in Android settings.",
                );
                let mut interval = self.prefs.background_interval_secs.clamp(15, 3600);
                egui::ComboBox::from_id_salt("background_interval")
                    .selected_text(format!("{interval}s"))
                    .show_ui(ui, |ui| {
                        for value in [15_u64, 30, 60, 120, 300, 600] {
                            ui.selectable_value(&mut interval, value, format!("{value}s"));
                        }
                    });
                if interval != self.prefs.background_interval_secs {
                    self.prefs.background_interval_secs = interval;
                    self.save_prefs();
                }
                let running = crate::platform::android_background_location_running();
                if running {
                    ui.colored_label(theme::SUCCESS, format!("Running every {interval}s."));
                    if ui.button("Stop background sharing").clicked() {
                        match crate::platform::android_stop_background_location() {
                            Ok(()) => self
                                .app
                                .set_notice(Some("Background sharing stopped.".to_string())),
                            Err(error) => self.app.set_error(Some(error)),
                        }
                    }
                } else {
                    if theme::primary_button(ui, "Start background sharing").clicked() {
                        let fine = crate::platform::ensure_location_permission();
                        let background = crate::platform::ensure_background_location_permission();
                        if fine != crate::platform::LocationPermission::Granted {
                            self.app.set_notice(Some(
                                "Allow location access, then start background sharing again."
                                    .to_string(),
                            ));
                        } else if background != crate::platform::LocationPermission::Granted {
                            self.app.set_notice(Some(
                                "Allow location \"all the time\" in app settings, then start again."
                                    .to_string(),
                            ));
                        } else {
                            match crate::platform::android_start_background_location(interval) {
                                Ok(()) => {
                                    self.prefs.background_interval_secs = interval;
                                    self.save_prefs();
                                    self.app.set_notice(Some(
                                        "Background sharing started.".to_string(),
                                    ));
                                }
                                Err(error) => self.app.set_error(Some(error)),
                            }
                        }
                    }
                    if ui.button("Open app settings").clicked() {
                        if let Err(error) = crate::platform::open_app_settings() {
                            self.app.set_error(Some(error));
                        }
                    }
                }
                ui.label(
                    RichText::new(
                        "Only while signed in. Location is latest-only; no history is kept. \
                         Android may stop the service when the app is force-stopped.",
                    )
                    .size(10.0)
                    .color(theme::MUTED),
                );
            });
            ui.add_space(8.0);
        }
        theme::card(ui, |ui| {
            theme::card_title(ui, "Safety");
            ui.label("Share expiry, pause, and delete live under Sharing. Emergency alerts under Emergency.");
            ui.label("Location is latest-only; no history is stored.");
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            ui.label(
                RichText::new("App lock (on-device PIN)")
                    .size(14.0)
                    .strong(),
            );
            if self.lock_enabled {
                ui.label(
                    "Locations stay hidden until the PIN is entered after launch or \
                     returning to the app.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Lock now").clicked() {
                        self.locked = true;
                        self.pin_input.clear();
                        self.pin_error = None;
                        self.pin_retry_at = None;
                        self.pin_focus_requested = true;
                        self.biometric_auto_prompted = false;
                    }
                    if ui.button("Remove PIN").clicked() {
                        match crate::app_lock::LockStore::default_location().remove() {
                            Ok(()) => {
                                self.lock_enabled = false;
                                self.locked = false;
                                self.app.set_notice(Some("App lock removed.".to_string()));
                            }
                            Err(error) => self
                                .app
                                .set_error(Some(format!("Could not remove the PIN: {error}"))),
                        }
                    }
                });
            } else {
                ui.label(
                    "Set a 4-12 digit PIN so cached locations stay hidden when the \
                     app is reopened.",
                );
            }
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.pin_setup)
                    .password(true)
                    .char_limit(12)
                    .hint_text("New PIN")
                    .desired_width(140.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.pin_setup_confirm)
                    .password(true)
                    .char_limit(12)
                    .hint_text("Confirm PIN")
                    .desired_width(140.0),
            );
            let action = if self.lock_enabled {
                "Change PIN"
            } else {
                "Set PIN"
            };
            if ui.button(action).clicked() {
                if self.pin_setup != self.pin_setup_confirm {
                    self.app
                        .set_error(Some("The PIN entries do not match.".to_string()));
                } else {
                    let pin = self.pin_setup.clone();
                    match crate::app_lock::LockStore::default_location().set_pin(&pin) {
                        Ok(()) => {
                            self.lock_enabled = true;
                            self.pin_setup.clear();
                            self.pin_setup_confirm.clear();
                            self.app.set_notice(Some("App lock PIN saved.".to_string()));
                        }
                        Err(error) => self
                            .app
                            .set_error(Some(format!("Could not save the PIN: {error}"))),
                    }
                }
            }
            ui.label(
                RichText::new("The PIN is stored only as an Argon2id hash on this device.")
                    .size(10.0)
                    .color(theme::MUTED),
            );
            ui.label(
                RichText::new(
                    "Casual privacy lock only: it does not encrypt app data. The retry delay \
                     is persisted on this device, so force-stopping the app does not clear it.",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
            #[cfg(target_os = "android")]
            if self.lock_enabled {
                ui.add_space(6.0);
                if crate::platform::android_biometric_supported() {
                    let mut enabled = self.prefs.biometric_unlock;
                    if ui
                        .checkbox(&mut enabled, "Unlock with biometrics")
                        .changed()
                    {
                        self.prefs.biometric_unlock = enabled;
                        self.save_prefs();
                        self.pin_error = None;
                        self.biometric_auto_prompted = false;
                        if enabled {
                            let _ = crate::platform::android_start_biometric();
                        }
                    }
                    ui.label(
                        RichText::new(
                            "Fingerprint or face can unlock the app; the PIN remains the fallback.",
                        )
                        .size(10.0)
                        .color(theme::MUTED),
                    );
                } else {
                    ui.label(
                        RichText::new("No enrolled biometrics are available on this device.")
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                }
            }
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            ui.label(
                RichText::new("Two-factor authentication (TOTP)")
                    .size(14.0)
                    .strong(),
            );
            let (enabled, pending) = {
                let d = self.app.data();
                let d = d.lock().unwrap();
                (d.mfa_enabled, d.totp_pending_secret.clone())
            };
            if enabled {
                ui.label("MFA is ON for this account.");
                if ui.button("Disable MFA").clicked() {
                    let app = Arc::clone(&self.app);
                    let rt_ctx = ctx.clone();
                    self.rt.spawn(async move {
                        let _ = app.mfa_disable().await;
                    });
                    let _ = rt_ctx;
                }
            } else if let Some(secret) = pending {
                ui.label(
                    "Scan this secret in your authenticator app, then enter the 6-digit code:",
                );
                ui.monospace(&secret);
                ui.add(
                    egui::TextEdit::singleline(&mut self.mfa_code)
                        .hint_text("6-digit code")
                        .desired_width(140.0),
                );
                if theme::primary_button(ui, "Verify & enable").clicked()
                    && !self.mfa_code.trim().is_empty()
                {
                    let code = self.mfa_code.clone();
                    let app = Arc::clone(&self.app);
                    self.rt.spawn(async move {
                        let _ = app.mfa_verify(code).await;
                    });
                }
            } else {
                ui.label("MFA is OFF. Enrol to require an authenticator code at sign-in.");
                if ui.button("Enrol MFA").clicked() {
                    let app = Arc::clone(&self.app);
                    self.rt.spawn(async move {
                        let _ = app.mfa_enroll().await;
                    });
                }
            }
        });
        ui.add_space(8.0);

        theme::card(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Active sessions");
                if ui.button("Refresh").clicked() {
                    self.spawn_action(PendingAction::RefreshSessions, ctx);
                }
            });
            let sessions = {
                let d = self.app.data();
                let d = d.lock().unwrap();
                d.sessions.clone()
            };
            if sessions.is_empty() {
                ui.label("No sessions loaded.");
            } else {
                for session in &sessions {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!(
                            "{} · {} · {}",
                            session.auth_method, session.id, session.last_activity
                        ));
                        if session.current {
                            ui.label(RichText::new("this device").color(theme::SUCCESS));
                        }
                        if ui
                            .button(if session.current {
                                "Sign out"
                            } else {
                                "Revoke"
                            })
                            .clicked()
                        {
                            self.spawn_action(
                                PendingAction::RevokeSession {
                                    id: session.id.clone(),
                                    current: session.current,
                                },
                                ctx,
                            );
                        }
                    });
                }
            }
            if ui.button("Sign out everywhere").clicked() {
                self.spawn_action(PendingAction::LogoutAll, ctx);
            }
        });
        ui.add_space(8.0);

        theme::card(ui, |ui| {
            theme::card_title(ui, "Help & diagnostics");
            ui.label("Sends only the category and message — no location, IP address or device ID.");
            egui::ComboBox::from_id_salt("diagnostic_category")
                .selected_text(&self.diagnostic_category)
                .show_ui(ui, |ui| {
                    for category in ["map", "routing", "connectivity", "crash", "other"] {
                        ui.selectable_value(
                            &mut self.diagnostic_category,
                            category.to_string(),
                            category,
                        );
                    }
                });
            ui.add(
                egui::TextEdit::multiline(&mut self.diagnostic_message)
                    .hint_text("What happened?")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .char_limit(2000),
            );
            if ui.button("Send report").clicked() && !self.diagnostic_message.trim().is_empty() {
                self.spawn_action(
                    PendingAction::SubmitDiagnostic {
                        category: self.diagnostic_category.clone(),
                        message: self.diagnostic_message.trim().to_string(),
                    },
                    ctx,
                );
            }
            let email_ready = !self.diagnostic_message.trim().is_empty();
            if ui
                .add_enabled(email_ready, egui::Button::new("Email report"))
                .clicked()
            {
                let summary = format!("User report: {}", self.diagnostic_category);
                let note = self.diagnostic_message.trim().to_string();
                self.open_manual_report_with_note(&summary, &note);
            }
            ui.label(
                RichText::new(
                    "Send report uses the backend; Email report opens your mail app and works without a server.",
                )
                .size(10.0),
            );
        });
        ui.add_space(8.0);

        theme::card(ui, |ui| {
            ui.label(
                RichText::new("Danger zone")
                    .size(14.0)
                    .strong()
                    .color(theme::DANGER),
            );
            ui.label(
                "Deleting your account removes your profile, friendships, shares, current \
                 location, check-ins, emergencies and sessions. This cannot be undone.",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.delete_confirm)
                    .hint_text("Type DELETE")
                    .desired_width(160.0),
            );
            let anonymous = self
                .app
                .data()
                .lock()
                .unwrap()
                .user
                .as_ref()
                .map(|u| u.auth_method == "anonymous")
                .unwrap_or(false);
            if !anonymous {
                ui.add(
                    egui::TextEdit::singleline(&mut self.delete_password)
                        .password(true)
                        .hint_text("Current password")
                        .desired_width(220.0),
                );
            }
            if ui
                .button(RichText::new("Delete my account").color(theme::DANGER))
                .clicked()
            {
                let confirm = self.delete_confirm.trim().to_string();
                if confirm != "DELETE" {
                    self.app
                        .set_error(Some("Type DELETE to confirm account deletion.".to_string()));
                } else {
                    let password = if anonymous {
                        None
                    } else {
                        Some(self.delete_password.clone())
                    };
                    self.spawn_action(PendingAction::DeleteAccount { confirm, password }, ctx);
                }
            }
        });
        ui.add_space(8.0);

        if ui.button("About & updates").clicked() {
            self.app.set_screen(crate::app::CurrentScreen::About);
        }
        ui.add_space(8.0);

        theme::card(ui, |ui| {
            theme::card_title(ui, "Map appearance");
            let (mut label, mut icon) = {
                let d = self.app.data();
                let d = d.lock().unwrap();
                (
                    d.map_label_size.unwrap_or(12.0),
                    d.map_icon_size.unwrap_or(26.0),
                )
            };
            if ui
                .add(egui::Slider::new(&mut label, 10.0..=18.0).text("Label size"))
                .changed()
            {
                self.app.data().lock().unwrap().map_label_size = Some(label);
            }
            if ui
                .add(egui::Slider::new(&mut icon, 18.0..=40.0).text("Icon size"))
                .changed()
            {
                self.app.data().lock().unwrap().map_icon_size = Some(icon);
            }
            if ui.button("Reset").clicked() {
                let data = self.app.data();
                let mut d = data.lock().unwrap();
                d.map_label_size = None;
                d.map_icon_size = None;
            }
        });
    }
}
