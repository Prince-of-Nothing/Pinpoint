//! Trips and check-ins screen
use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self};

impl PinpointAppUI {
    pub fn draw_trips_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("trips_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_trips_body(ui, ctx));
    }

    fn draw_trips_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Trips & check-ins");
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "New check-in");
            ui.add(
                egui::TextEdit::singleline(&mut self.checkin_lat)
                    .hint_text("Latitude")
                    .desired_width(110.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.checkin_lon)
                    .hint_text("Longitude")
                    .desired_width(110.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.checkin_note)
                    .hint_text("Note")
                    .desired_width(220.0),
            );
            for k in ["manual", "scheduled", "trip"] {
                ui.selectable_value(&mut self.checkin_kind, k.to_string(), k);
            }
            if theme::primary_button(ui, "Check in").clicked() {
                if let (Ok(lat), Ok(lon)) = (
                    self.checkin_lat.trim().parse::<f64>(),
                    self.checkin_lon.trim().parse::<f64>(),
                ) {
                    self.spawn_action(
                        PendingAction::CreateCheckIn {
                            lat,
                            lon,
                            note: self.checkin_note.clone(),
                            kind: self.checkin_kind.clone(),
                        },
                        ctx,
                    );
                } else {
                    self.app
                        .set_error(Some("Enter valid latitude/longitude".to_string()));
                }
            }
        });
        ui.add_space(8.0);
        theme::card(ui, |ui| {
            theme::card_title(ui, "Recent check-ins");
            let data = self.app.data();
            let d = data.lock().unwrap();
            if d.check_ins.is_empty() {
                ui.label("No check-ins yet.");
            }
            for c in &d.check_ins {
                ui.label(format!(
                    "[{}] {} ({:.4}, {:.4}) — {} @ {}",
                    c.kind, c.user_name, c.latitude, c.longitude, c.note, c.created_at
                ));
            }
        });
    }
}
