//! Sharing screen
use super::theme;
use super::{PendingAction, PinpointAppUI};
use eframe::egui::{self};

impl PinpointAppUI {
    pub fn draw_sharing_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("sharing_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_sharing_body(ui, ctx));
    }

    fn draw_sharing_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Location Sharing");
        ui.add_space(4.0);
        theme::caption(ui, "Choose who sees your location, and how precisely");
        ui.add_space(10.0);

        // Create new share targeting a selected accepted friendship.
        theme::card(ui, |ui| {
            theme::card_title(ui, "Share Location");
            ui.add_space(4.0);

            let accepted: Vec<(i64, String)> = {
                let binding = self.app.data();
                let data = binding.lock().unwrap();
                data.friendships
                    .iter()
                    .filter(|f| f.status == "accepted")
                    .map(|f| (f.id, format!("{} ↔ {}", f.requester_name, f.receiver_name)))
                    .collect()
            };

            if accepted.is_empty() {
                ui.label("Add an accepted friend first.");
            } else {
                ui.label("Share with:");
                for (id, label) in &accepted {
                    let selected = self.selected_friendship == Some(*id);
                    if ui.selectable_label(selected, label).clicked() {
                        self.selected_friendship = Some(*id);
                    }
                }
            }

            ui.add_space(4.0);

            let compact = Self::is_compact(ui.ctx());
            if compact {
                ui.label("Direction:");
            }
            ui.horizontal_wrapped(|ui| {
                if !compact {
                    ui.label("Direction:");
                }
                ui.selectable_value(
                    &mut self.share_direction,
                    "to_friend".to_string(),
                    "To friend",
                );
                ui.add_space(4.0);
                ui.selectable_value(
                    &mut self.share_direction,
                    "from_friend".to_string(),
                    "From friend",
                );
                ui.add_space(4.0);
                ui.selectable_value(&mut self.share_direction, "mutual".to_string(), "Mutual");
            });

            ui.add_space(4.0);
            if compact {
                ui.label("Precision:");
            }
            ui.horizontal_wrapped(|ui| {
                if !compact {
                    ui.label("Precision:");
                }
                ui.selectable_value(
                    &mut self.share_precision,
                    "exact".to_string(),
                    "Exact (GPS coordinates)",
                );
                ui.add_space(4.0);
                ui.selectable_value(
                    &mut self.share_precision,
                    "approximate".to_string(),
                    "Approximate (neighborhood)",
                );
                ui.add_space(4.0);
                ui.selectable_value(
                    &mut self.share_precision,
                    "city_only".to_string(),
                    "City only (city level)",
                );
            });

            ui.add_space(4.0);
            if compact {
                ui.label("Expires in (hours, blank = never):");
            }
            ui.horizontal(|ui| {
                if !compact {
                    ui.label("Expires in (hours, blank = never):");
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.share_duration)
                        .hint_text("e.g. 24")
                        .desired_width(70.0),
                );
            });

            ui.add_space(12.0);
            if theme::primary_button(ui, "Create Share").clicked() {
                if let Some(friendship_id) = self.selected_friendship {
                    let dur = self.share_duration.trim().parse::<i64>().ok();
                    self.spawn_action(
                        PendingAction::CreateShareEx {
                            friendship_id,
                            direction: self.share_direction.clone(),
                            precision: self.share_precision.clone(),
                            duration_hours: dur,
                        },
                        ctx,
                    );
                } else {
                    self.app
                        .set_error(Some("Select a friend to share with first".to_string()));
                }
            }
        });

        ui.add_space(8.0);

        // Active shares
        theme::card(ui, |ui| {
            theme::card_title(ui, "Active Shares");
            ui.add_space(4.0);

            let binding = self.app.data();
            let data = binding.lock().unwrap();
            if data.shares.is_empty() {
                ui.label("No active location sharing permissions");
            } else {
                for share in &data.shares {
                    let precision_color = match share.precision_level.as_str() {
                        "exact" => theme::SUCCESS,
                        "approximate" => theme::ACCENT,
                        "city_only" => theme::DANGER,
                        _ => theme::MUTED,
                    };
                    let is_active = share.is_active;
                    let share_id = share.id;
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(
                            precision_color,
                            format!(
                                "{} sharing {} (expires: {})",
                                share.direction,
                                share.precision_level,
                                share.expires_at.clone().unwrap_or("never".to_string())
                            ),
                        );
                        let label = if is_active { "Pause" } else { "Resume" };
                        if ui.button(label).clicked() {
                            self.spawn_action(
                                PendingAction::ToggleShare {
                                    share_id,
                                    is_active: !is_active,
                                },
                                ctx,
                            );
                        }
                        if ui.button("Delete").clicked() {
                            self.spawn_action(PendingAction::DeleteShare { share_id }, ctx);
                        }
                    });
                }
            }
        });
    }
}
