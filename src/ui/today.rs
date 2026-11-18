//! Today screen: the owner's private 24-hour trail and private place notes.
//!
//! Both lists come from owner-only endpoints. The native client sends text
//! notes only; photos are available in the web client because egui has no
//! image decoder / picker in this build.

use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self, RichText};

fn short_time(raw: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(raw)
        .map(|at| at.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_else(|_| raw.to_string())
}

impl PinpointAppUI {
    pub fn draw_today_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (history, notes, center_lat, center_lon) = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            (
                data.history.clone(),
                data.notes.clone(),
                data.map_center_lat,
                data.map_center_lon,
            )
        };

        ui.heading("Today");
        theme::caption(
            ui,
            "Your private 24-hour trail and notes. Only you can see them.",
        );
        ui.add_space(10.0);

        egui::ScrollArea::vertical()
            .id_salt("today_screen")
            .show(ui, |ui| {
                theme::full_card(ui, |ui| {
                    ui.horizontal(|ui| {
                        theme::card_title(ui, "Private trail");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::primary_button(ui, "Refresh").clicked() {
                                self.spawn_action(PendingAction::RefreshToday, ctx);
                            }
                        });
                    });
                    ui.add_space(4.0);
                    if history.is_empty() {
                        theme::caption(
                            ui,
                            "No points yet. Publish your location on the Map screen to add one.",
                        );
                    } else {
                        theme::caption(
                            ui,
                            format!(
                                "{} point(s) in the last 24 hours, newest first.",
                                history.len()
                            ),
                        );
                        for point in history.iter().rev().take(24) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(short_time(&point.recorded_at))
                                        .strong()
                                        .color(theme::TEXT),
                                );
                                theme::caption(
                                    ui,
                                    format!("{:.5}, {:.5}", point.latitude, point.longitude),
                                );
                            });
                        }
                    }
                });

                ui.add_space(8.0);

                theme::full_card(ui, |ui| {
                    theme::card_title(ui, "Add a private note");
                    ui.add_space(4.0);
                    ui.label("Label (optional)");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.note_label)
                            .hint_text("Cafe, viewpoint, parking...")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);
                    ui.label("Message");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.note_message)
                            .hint_text("What happened here?")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    theme::caption(ui, "Saved at the map centre. Photos are web-only for now.");
                    ui.horizontal(|ui| {
                        if theme::primary_button(ui, "Add note here").clicked() {
                            let label = self.note_label.trim().to_string();
                            let message = self.note_message.trim().to_string();
                            if message.is_empty() {
                                self.app
                                    .set_error(Some("Write a message for the note.".to_string()));
                            } else {
                                self.spawn_action(
                                    PendingAction::CreateNote {
                                        latitude: center_lat,
                                        longitude: center_lon,
                                        label,
                                        message,
                                    },
                                    ctx,
                                );
                                self.note_label.clear();
                                self.note_message.clear();
                            }
                        }
                        theme::caption(ui, format!("Map centre: {center_lat:.5}, {center_lon:.5}"));
                    });
                });

                ui.add_space(8.0);

                theme::full_card(ui, |ui| {
                    theme::card_title(ui, "Private notes");
                    ui.add_space(4.0);
                    if notes.is_empty() {
                        theme::caption(ui, "No notes yet.");
                        return;
                    }
                    for note in notes.iter().rev().take(24) {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(if note.label.is_empty() {
                                        "Private note"
                                    } else {
                                        note.label.as_str()
                                    })
                                    .strong()
                                    .color(theme::TEXT),
                                );
                                ui.label(&note.message);
                                theme::caption(
                                    ui,
                                    format!(
                                        "{} · {:.5}, {:.5}",
                                        short_time(&note.created_at),
                                        note.latitude,
                                        note.longitude
                                    ),
                                );
                                if note.photo.is_some() {
                                    theme::caption(
                                        ui,
                                        "Photo attached (view it in the web client).",
                                    );
                                }
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                if ui.button("Delete").clicked() {
                                    self.spawn_action(
                                        PendingAction::DeleteNote { id: note.id },
                                        ctx,
                                    );
                                }
                            });
                        });
                        ui.separator();
                    }
                });
            });
    }
}
