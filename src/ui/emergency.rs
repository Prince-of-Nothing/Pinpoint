//! Emergency screen
use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, Color32, RichText};

impl PinpointAppUI {
    pub fn draw_emergency_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("emergency_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_emergency_body(ui, ctx));
    }

    fn draw_emergency_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Emergency");
        ui.add_space(4.0);
        theme::caption(ui, "Alerts are visible to every accepted friend.");
        ui.add_space(10.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Send alert");
            ui.colored_label(
                theme::DANGER,
                RichText::new("Notifies all accepted friends and activates every share.")
                    .size(13.0),
            );
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.emergency_lat)
                        .hint_text("Latitude")
                        .desired_width(110.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.emergency_lon)
                        .hint_text("Longitude")
                        .desired_width(110.0),
                );
            });
            ui.add_space(4.0);
            let alert = egui::Button::new(
                RichText::new("Send emergency alert")
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(theme::DANGER)
            .stroke(egui::Stroke::NONE);
            if ui.add(alert).clicked() {
                if let (Ok(lat), Ok(lon)) = (
                    self.emergency_lat.trim().parse::<f64>(),
                    self.emergency_lon.trim().parse::<f64>(),
                ) {
                    self.spawn_action(PendingAction::TriggerEmergency { lat, lon }, ctx);
                } else {
                    self.app
                        .set_error(Some("Enter valid latitude/longitude".to_string()));
                }
            }
        });
        ui.add_space(10.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Alerts");
            let data = self.app.data();
            let d = data.lock().unwrap();
            if d.emergencies.is_empty() {
                theme::caption(ui, "No alerts.");
            }
            for e in &d.emergencies {
                ui.label(format!(
                    "{} ({:.4}, {:.4}) · {} · resolved: {}",
                    e.user_name, e.latitude, e.longitude, e.created_at, e.resolved
                ));
            }
        });
    }
}
