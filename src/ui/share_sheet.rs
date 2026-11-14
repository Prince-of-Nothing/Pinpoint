//! Per-friend sharing sheet (web `web/app/sharing.js` parity).
//!
//! The native sharing screen remains the global list; this sheet edits the
//! directional share for one accepted friendship and reuses the existing
//! share instead of creating duplicates.

use crate::models::{Friendship, LocationShare};
use eframe::egui::{self, Color32, RichText};

#[derive(Debug, Clone, PartialEq)]
pub struct ShareSheet {
    pub friendship_id: i64,
    pub friend_name: String,
    pub share_id: Option<i64>,
    pub enabled: bool,
    pub precision: String,
    pub duration_hours: i64,
    pub incoming: String,
    pub open: bool,
}

impl ShareSheet {
    pub fn for_friendship(
        friendship: &Friendship,
        me_id: Option<i64>,
        shares: &[LocationShare],
    ) -> Self {
        let friend_name = if Some(friendship.requester_id) == me_id {
            friendship.receiver_name.clone()
        } else {
            friendship.requester_name.clone()
        };
        let outgoing = shares.iter().find(|share| {
            share.friendship_id == friendship.id
                && matches!(share.direction.as_str(), "to_friend" | "mutual")
        });
        let incoming = shares.iter().find(|share| {
            share.friendship_id == friendship.id
                && matches!(share.direction.as_str(), "from_friend" | "mutual")
                && share.is_active
        });
        let incoming_text = match incoming {
            Some(share) => format!(
                "This friend shares their {} location with you.",
                match share.precision_level.as_str() {
                    "exact" => "exact",
                    "approximate" => "approximate",
                    _ => "city-level",
                }
            ),
            None => "This friend does not currently share a location with you.".to_string(),
        };
        Self {
            friendship_id: friendship.id,
            friend_name,
            share_id: outgoing.map(|share| share.id),
            enabled: outgoing.map(|share| share.is_active).unwrap_or(false),
            precision: outgoing
                .map(|share| share.precision_level.clone())
                .unwrap_or_else(|| "exact".to_string()),
            duration_hours: if outgoing
                .and_then(|share| share.expires_at.as_ref())
                .is_some()
            {
                24
            } else {
                0
            },
            incoming: incoming_text,
            open: true,
        }
    }
}

impl super::PinpointAppUI {
    pub(crate) fn draw_share_sheet(&mut self, ctx: &egui::Context) {
        let Some(mut sheet) = self.share_sheet.clone() else {
            return;
        };
        if !sheet.open {
            self.share_sheet = None;
            return;
        }
        let compact = Self::is_compact(ctx);
        let panel = Color32::from_rgba_unmultiplied(15, 23, 42, 248);
        let border = Color32::from_rgb(31, 41, 55);
        let muted = Color32::from_rgb(156, 163, 175);
        let accent = Color32::from_rgb(52, 211, 153);
        let mut save = false;
        let mut stop = false;
        let mut close = false;

        egui::Area::new(egui::Id::new("share_sheet"))
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::CENTER_BOTTOM,
                egui::vec2(0.0, if compact { -70.0 } else { -12.0 }),
            )
            .show(ctx, |ui| {
                ui.set_width(if compact {
                    ctx.screen_rect().width() - 24.0
                } else {
                    380.0
                });
                egui::Frame::new()
                    .fill(panel)
                    .stroke(egui::Stroke::new(1.0_f32, border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("Share with {}", sheet.friend_name))
                                    .size(17.0)
                                    .strong(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("×").clicked() {
                                        close = true;
                                    }
                                },
                            );
                        });
                        ui.label(
                            RichText::new("Only what you allow below is shared with this friend.")
                                .size(11.0)
                                .color(muted),
                        );
                        ui.add_space(6.0);
                        ui.checkbox(&mut sheet.enabled, "Share my location with this friend");
                        ui.add_space(4.0);
                        ui.label(RichText::new("Precision").strong());
                        ui.horizontal_wrapped(|ui| {
                            for (value, label) in [
                                ("exact", "Exact"),
                                ("approximate", "Approximate"),
                                ("city_only", "City only"),
                            ] {
                                if ui
                                    .add(egui::Button::selectable(sheet.precision == value, label))
                                    .clicked()
                                {
                                    sheet.precision = value.to_string();
                                }
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(RichText::new("For how long").strong());
                        ui.horizontal_wrapped(|ui| {
                            for (hours, label) in [
                                (0, "Until off"),
                                (1, "1 h"),
                                (8, "8 h"),
                                (24, "24 h"),
                                (168, "7 days"),
                            ] {
                                if ui
                                    .add(egui::Button::selectable(
                                        sheet.duration_hours == hours,
                                        label,
                                    ))
                                    .clicked()
                                {
                                    sheet.duration_hours = hours;
                                }
                            }
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(&sheet.incoming).size(11.0).color(muted));
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Save sharing").clicked() {
                                save = true;
                            }
                            if ui
                                .add_enabled(
                                    sheet.share_id.is_some(),
                                    egui::Button::new("Stop sharing"),
                                )
                                .clicked()
                            {
                                stop = true;
                            }
                            if ui.button("Close").clicked() {
                                close = true;
                            }
                        });
                        ui.label(
                            RichText::new(format!(
                                "Currently: {}",
                                if sheet.enabled { "sharing" } else { "paused" }
                            ))
                            .size(10.0)
                            .color(accent),
                        );
                    });
            });

        if close {
            self.share_sheet = None;
            return;
        }
        if save {
            self.spawn_action(
                super::PendingAction::SaveShare {
                    friendship_id: sheet.friendship_id,
                    share_id: sheet.share_id,
                    enabled: sheet.enabled,
                    precision: sheet.precision.clone(),
                    duration_hours: sheet.duration_hours,
                },
                ctx,
            );
            self.share_sheet = None;
            return;
        }
        if stop {
            if let Some(share_id) = sheet.share_id {
                self.spawn_action(
                    super::PendingAction::SaveShare {
                        friendship_id: sheet.friendship_id,
                        share_id: Some(share_id),
                        enabled: false,
                        precision: sheet.precision.clone(),
                        duration_hours: sheet.duration_hours,
                    },
                    ctx,
                );
            }
            self.share_sheet = None;
            return;
        }
        self.share_sheet = Some(sheet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn friendship(id: i64) -> Friendship {
        Friendship {
            id,
            requester_id: 1,
            receiver_id: 2,
            requester_name: "Alice".into(),
            receiver_name: "Bob".into(),
            status: "accepted".into(),
            created_at: None,
            responded_at: None,
        }
    }

    fn share(id: i64, friendship_id: i64, direction: &str, active: bool) -> LocationShare {
        LocationShare {
            id,
            friendship_id,
            owner_id: 1,
            direction: direction.into(),
            precision_level: "approximate".into(),
            is_active: active,
            last_location_update_at: None,
            expires_at: Some("2026-11-15T00:00:00Z".into()),
            label: None,
        }
    }

    #[test]
    fn existing_outgoing_share_loads_id_precision_and_duration() {
        let shares = vec![share(9, 7, "to_friend", true)];
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &shares);
        assert_eq!(sheet.share_id, Some(9));
        assert!(sheet.enabled);
        assert_eq!(sheet.precision, "approximate");
        assert_eq!(sheet.duration_hours, 24);
        assert!(sheet.incoming.contains("does not currently share"));
    }

    #[test]
    fn no_share_defaults_to_off_and_exact() {
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[]);
        assert_eq!(sheet.share_id, None);
        assert!(!sheet.enabled);
        assert_eq!(sheet.precision, "exact");
        assert_eq!(sheet.duration_hours, 0);
    }

    #[test]
    fn incoming_share_is_described() {
        let shares = vec![share(10, 7, "from_friend", true)];
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &shares);
        assert!(sheet.incoming.contains("approximate"));
    }
}
