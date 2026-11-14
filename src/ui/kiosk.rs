//! Kiosk / check-in screen for Pinpoint (portable webapp logic for egui).

use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, Direction, RichText};

impl PinpointAppUI {
    pub fn draw_kiosk_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading("Kiosk & Share Check-In");
                },
            );
        });
        ui.add_space(8.0);
        ui.label(format!("Backend: {}", self.app.backend_url()));
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(
                RichText::new("Share Link / Check-In Token")
                    .size(14.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.kiosk_token_input)
                        .hint_text("Enter share token or check-in code")
                        .desired_width(280.0),
                );
                if ui.button("Resolve Token").clicked() {
                    let token = self.kiosk_token_input.trim().to_string();
                    if !token.is_empty() {
                        self.spawn_action(PendingAction::KioskResolveToken { token }, ctx);
                    } else {
                        self.app.set_error(Some("Enter a token to resolve.".into()));
                    }
                }
            });
        });

        let qr_token = self.kiosk_token_input.trim().to_string();
        if !qr_token.is_empty() {
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(RichText::new("Scan to open this kiosk").size(12.0).strong());
                let link = format!("pinpoint://kiosk?token={qr_token}");
                super::qr::draw_qr(ui, &link, 180.0);
                ui.label(RichText::new(link).size(10.0));
            });
        }

        ui.add_space(12.0);

        let resolved = self.app.data().lock().unwrap().kiosk_resolved.clone();

        if let Some(resolved) = resolved {
            ui.group(|ui| {
                ui.label(RichText::new("Resolved Share Link").size(14.0).strong());
                ui.add_space(4.0);
                ui.label(format!("Token: {}", resolved.token));
                if let Some(label) = &resolved.label {
                    ui.label(format!("Label: {label}"));
                }
                if let Some(exp) = &resolved.expires_at {
                    ui.label(format!("Expires: {exp}"));
                }
                if resolved.consent_required {
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.kiosk_consent_accepted,
                        "I consent to viewing location & submitting check-ins for this share link",
                    );
                } else {
                    self.kiosk_consent_accepted = true;
                }

                if !resolved.consent_required || self.kiosk_consent_accepted {
                    ui.add_space(8.0);
                    if let Some(loc) = &resolved.location {
                        ui.label(RichText::new("Shared Location:").strong());
                        ui.label(format!("User: {}", loc.user_name));
                        ui.label(format!(
                            "Coordinates: {:.5}, {:.5}",
                            loc.latitude, loc.longitude
                        ));
                        ui.label(format!("Precision: {}", loc.precision_level));
                        ui.label(format!("Updated: {}", loc.updated_at));
                        if ui.button("Show on Map").clicked() {
                            let data_handle = self.app.data();
                            let mut data = data_handle.lock().unwrap();
                            data.map_center_lat = loc.latitude;
                            data.map_center_lon = loc.longitude;
                            drop(data);
                            self.app.set_screen(crate::app::CurrentScreen::Map);
                        }
                    } else {
                        ui.label("No active location payload associated with this token.");
                    }
                }
            });

            ui.add_space(12.0);

            if !resolved.consent_required || self.kiosk_consent_accepted {
                ui.group(|ui| {
                    ui.label(RichText::new("Submit Kiosk Check-In").size(14.0).strong());
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.kiosk_note_input)
                            .hint_text("Optional check-in note (e.g. Arrived at station)")
                            .desired_width(320.0),
                    );
                    ui.add_space(8.0);
                    if ui.button("Submit Check-In").clicked() {
                        let token = resolved.token.clone();
                        let note = (!self.kiosk_note_input.trim().is_empty())
                            .then(|| self.kiosk_note_input.trim().to_string());
                        let req = crate::webapp::kiosk::KioskCheckInRequest {
                            token,
                            note,
                            latitude: None,
                            longitude: None,
                        };
                        self.spawn_action(PendingAction::KioskSubmitCheckIn { request: req }, ctx);
                    }
                });
            }
        }
    }
}
