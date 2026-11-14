//! Auth screens (login / register)
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use eframe::egui::{self};

impl PinpointAppUI {
    pub fn draw_login_form(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let compact = Self::is_compact(ctx);
        ui.vertical_centered(|ui| {
            if !compact {
                ui.heading("Pinpoint");
                ui.label("Private location sharing for trusted friends");
            }
            ui.small(format!("Backend: {}", self.app.backend_url()));
            ui.add_space(if compact { 10.0 } else { 20.0 });

            ui.group(|ui| {
                ui.label("Sign in");
                ui.add_space(8.0);
                let field_width = if compact { 300.0 } else { 250.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_email)
                        .hint_text("Email")
                        .desired_width(field_width),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_password)
                        .password(true)
                        .hint_text("Password")
                        .desired_width(field_width),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.login_totp)
                        .hint_text("Authenticator code (if MFA is on)")
                        .desired_width(field_width),
                );
                ui.add_space(16.0);
                if ui.button("Sign in").clicked()
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
                if ui.button("Register").clicked() {
                    self.app.set_screen(CurrentScreen::Register);
                }
            });

            ui.add_space(8.0);
            let field_width = if compact { 300.0 } else { 250.0 };
            ui.horizontal_wrapped(|ui| {
                if ui.button("Continue anonymously").clicked() {
                    self.spawn_action(PendingAction::AnonymousSignIn, ctx);
                }
                if ui.button("Recover with code").clicked() {
                    self.show_recover_form = !self.show_recover_form;
                }
            });
            if self.show_recover_form {
                ui.add_space(8.0);
                ui.group(|ui| {
                    ui.label("Recover anonymous account");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.recover_code_input)
                            .hint_text("Recovery code")
                            .desired_width(field_width),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Recover").clicked()
                            && !self.recover_code_input.trim().is_empty()
                        {
                            let code = self.recover_code_input.clone();
                            self.spawn_action(PendingAction::RecoverAnonymous { code }, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_recover_form = false;
                        }
                    });
                });
            }
        });
    }

    /// One-time recovery-code panel shown after anonymous sign-in/recovery.
    pub(crate) fn draw_recovery_code(&mut self, ctx: &egui::Context) {
        let Some(code) = self.app.data().lock().unwrap().recovery_code.clone() else {
            return;
        };
        let mut copy = false;
        let mut dismiss = false;
        egui::Area::new(egui::Id::new("recovery_code_panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 32.0).min(420.0));
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(15, 23, 42))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(250, 204, 21),
                    ))
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
                            if ui.button("I saved it").clicked() {
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
        let compact = Self::is_compact(ctx);
        ui.vertical_centered(|ui| {
            if !compact {
                ui.heading("Create Account");
            }
            ui.add_space(if compact { 10.0 } else { 20.0 });

            ui.group(|ui| {
                ui.label("New to Pinpoint?");
                ui.add_space(10.0);
                let field_width = if compact { 300.0 } else { 200.0 };
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_email)
                        .hint_text("Email")
                        .desired_width(field_width),
                );
                ui.add_space(5.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_password)
                        .password(true)
                        .hint_text("Password")
                        .desired_width(field_width),
                );
                ui.add_space(5.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.register_full_name)
                        .hint_text("Full Name")
                        .desired_width(field_width),
                );
                ui.add_space(10.0);

                ui.horizontal_wrapped(|ui| {
                    if ui.button("Create Account").clicked()
                        && !self.register_email.is_empty()
                        && !self.register_password.is_empty()
                    {
                        self.spawn_action(
                            PendingAction::Register {
                                email: self.register_email.clone(),
                                password: self.register_password.clone(),
                                full_name: self.register_full_name.clone(),
                            },
                            ctx,
                        );
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
