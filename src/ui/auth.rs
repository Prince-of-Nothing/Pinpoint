//! Auth screens (login / register)
use super::theme;
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use eframe::egui::{self};

impl PinpointAppUI {
    pub fn draw_login_form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.app.is_backend_unavailable() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Offline mode");
                ui.label(
                    "No backend is configured for this build. The map is \
                     available, but sign-in is disabled.",
                );
            });
            ui.add_space(8.0);
            theme::card(ui, |ui| self.draw_server_settings(ui));
            return;
        }
        let compact = Self::is_compact(ctx);
        ui.vertical_centered(|ui| {
            if !compact {
                ui.heading("Pinpoint");
                ui.label("Private location sharing for trusted friends");
            }
            ui.add_space(if compact { 10.0 } else { 20.0 });

            theme::card(ui, |ui| {
                ui.label("Sign in");
                ui.add_space(8.0);
                let field_width = if compact { 300.0 } else { 250.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_email)
                        .hint_text("Email")
                        .char_limit(254)
                        .desired_width(field_width),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_password)
                        .password(true)
                        .hint_text("Password")
                        .char_limit(128)
                        .desired_width(field_width),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_totp)
                        .hint_text("Authenticator code (if MFA is on)")
                        .char_limit(6)
                        .desired_width(field_width),
                );
                ui.add_space(16.0);
                if theme::primary_button(ui, "Sign in").clicked()
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
                if ui.button("Register").clicked() {
                    self.app.set_screen(CurrentScreen::Register);
                }
            });

            ui.add_space(8.0);
            let field_width = if compact { 300.0 } else { 250.0 };
            let now = std::time::Instant::now();
            let anonymous_busy = self.app.is_loading()
                || self
                    .anonymous_busy_until
                    .map(|until| until > now)
                    .unwrap_or(false);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!anonymous_busy, egui::Button::new("Continue anonymously"))
                    .clicked()
                {
                    self.anonymous_busy_until = Some(now + std::time::Duration::from_secs(5));
                    self.spawn_action(PendingAction::AnonymousSignIn, ctx);
                }
                if ui.button("Recover with code").clicked() {
                    self.show_recover_form = !self.show_recover_form;
                }
            });
            if self.show_recover_form {
                ui.add_space(8.0);
                theme::card(ui, |ui| {
                    ui.label("Recover anonymous account");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.recover_code_input)
                            .hint_text("Recovery code")
                            .char_limit(64)
                            .desired_width(field_width),
                    );
                    ui.horizontal(|ui| {
                        if theme::primary_button(ui, "Recover").clicked()
                            && !self.recover_code_input.trim().is_empty()
                        {
                            let code = std::mem::take(&mut self.recover_code_input);
                            self.spawn_action(PendingAction::RecoverAnonymous { code }, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_recover_form = false;
                            self.recover_code_input.clear();
                        }
                    });
                });
            }
            ui.add_space(8.0);
            ui.collapsing("Server", |ui| self.draw_server_settings(ui));
        });
    }

    /// One-time recovery-code panel shown after anonymous sign-in/recovery.
    pub(crate) fn draw_recovery_code(&mut self, ctx: &egui::Context) {
        let Some(code) = self.app.data().lock().unwrap().recovery_code.clone() else {
            return;
        };
        let blocked = self.update_dialog_open || self.crash_dialog.is_some();
        if super::escape_dismisses(ctx.input(|i| i.key_pressed(egui::Key::Escape)), blocked) {
            self.app.data().lock().unwrap().recovery_code = None;
            self.show_recover_form = false;
            return;
        }
        let mut copy = false;
        let mut dismiss = false;
        egui::Area::new(egui::Id::new("recovery_code_panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 32.0).min(420.0));
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .stroke(egui::Stroke::new(1.0_f32, theme::ACCENT))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Save your recovery code")
                                .size(18.0)
                                .strong(),
                        );
                        ui.label(
                            "This anonymous account exists only on this device session. \
                             The code is shown once and is the only way to restore it.",
                        );
                        ui.add_space(6.0);
                        ui.monospace(&code);
                        ui.add_space(8.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Copy code").clicked() {
                                copy = true;
                            }
                            if theme::primary_button(ui, "I saved it").clicked() {
                                dismiss = true;
                            }
                        });
                    });
            });
        if copy {
            ctx.copy_text(code);
        }
        if dismiss {
            self.app.data().lock().unwrap().recovery_code = None;
            self.show_recover_form = false;
        }
    }

    pub fn draw_register(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if self.app.is_backend_unavailable() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Offline mode");
                ui.label(
                    "No backend is configured for this build. The map is \
                     available, but registration is disabled.",
                );
            });
            ui.add_space(8.0);
            theme::card(ui, |ui| self.draw_server_settings(ui));
            return;
        }
        let compact = Self::is_compact(ctx);
        ui.vertical_centered(|ui| {
            if !compact {
                ui.heading("Create Account");
            }
            ui.add_space(if compact { 10.0 } else { 20.0 });

            theme::card(ui, |ui| {
                ui.label("New to Pinpoint?");
                ui.add_space(10.0);
                let field_width = if compact { 300.0 } else { 200.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_email)
                        .hint_text("Email")
                        .char_limit(254)
                        .desired_width(field_width),
                );
                ui.add_space(5.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_password)
                        .password(true)
                        .hint_text("Password")
                        .char_limit(128)
                        .desired_width(field_width),
                );
                ui.add_space(5.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_full_name)
                        .hint_text("Full Name")
                        .char_limit(100)
                        .desired_width(field_width),
                );
                ui.add_space(10.0);

                ui.horizontal_wrapped(|ui| {
                    if theme::primary_button(ui, "Create Account").clicked() {
                        // Validate with the server's own rules before sending;
                        // the server counts characters, not UTF-8 bytes.
                        let check = crate::security::validation::validate_registration_form(
                            &self.register_email,
                            &self.register_password,
                        );
                        if !check.is_valid() {
                            self.app.set_error(Some(check.errors.join("; ")));
                        } else {
                            let password = std::mem::take(&mut self.register_password);
                            self.spawn_action(
                                PendingAction::Register {
                                    email: self.register_email.clone(),
                                    password,
                                    full_name: self.register_full_name.clone(),
                                },
                                ctx,
                            );
                        }
                    }
                    ui.add_space(10.0);
                    if ui.button("Sign in instead").clicked() {
                        self.app.set_screen(CurrentScreen::Login);
                    }
                });
            });
        });
    }
}
