//! Per-friend sharing sheet (web `web/app/sharing.js` parity).
//!
//! The native sharing screen remains the global list; this sheet edits the
//! directional share for one accepted friendship and reuses the existing
//! share instead of creating duplicates.

use super::theme;
use crate::models::{Friendship, LocationShare};
use eframe::egui::{self, RichText};

#[derive(Debug, Clone, PartialEq)]
pub struct ShareSheet {
    pub friendship_id: i64,
    pub friend_name: String,
    pub share_id: Option<i64>,
    pub enabled: bool,
    pub precision: String,
    /// Optional owner-visible name; empty means no label.
    pub label: String,
    /// `None` means "keep the existing expiry"; `Some(0)` means no expiry.
    pub duration_hours: Option<i64>,
    /// True when an existing share has a future expiry, so the sheet can
    /// offer "Keep current" instead of silently resetting the clock.
    pub keep_current: bool,
    pub incoming: String,
    pub open: bool,
    /// The sheet asks for focus on the sharing toggle once after opening.
    pub focus_enabled: bool,
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
        // Only a future expiry can be kept; a legacy expired share needs a new
        // duration. A share with no expiry keeps "until off" explicitly.
        let keep_current = outgoing
            .and_then(|share| share.expires_at.as_deref())
            .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
            .is_some_and(|expiry| expiry.with_timezone(&chrono::Utc) > chrono::Utc::now());
        let duration_hours = match outgoing {
            Some(share) if share.expires_at.is_none() => Some(0),
            Some(_) if keep_current => None,
            _ => Some(24),
        };
        Self {
            friendship_id: friendship.id,
            friend_name,
            share_id: outgoing.map(|share| share.id),
            enabled: outgoing.map(|share| share.is_active).unwrap_or(false),
            precision: outgoing
                .map(|share| share.precision_level.clone())
                .unwrap_or_else(|| "exact".to_string()),
            label: outgoing
                .and_then(|share| share.label.clone())
                .unwrap_or_default(),
            duration_hours,
            keep_current,
            incoming: incoming_text,
            open: true,
            focus_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShareSheetAction {
    None,
    Close,
    Save,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShareSheetOutcome {
    pub action: ShareSheetAction,
    pub close_id: Option<egui::Id>,
    pub save_id: Option<egui::Id>,
    pub stop_id: Option<egui::Id>,
}

/// The fields a Save or Stop action sends for one friendship's share.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShareSaveRequest {
    pub friendship_id: i64,
    pub share_id: Option<i64>,
    pub enabled: bool,
    pub precision: String,
    pub duration_hours: Option<i64>,
    pub label: String,
}

/// Build the save request for the sheet; Save passes `sheet.enabled` and Stop
/// passes `false`, so both carry the same fields.
pub(crate) fn share_save_request(sheet: &ShareSheet, enabled: bool) -> ShareSaveRequest {
    ShareSaveRequest {
        friendship_id: sheet.friendship_id,
        share_id: sheet.share_id,
        enabled,
        precision: sheet.precision.clone(),
        duration_hours: sheet.duration_hours,
        label: sheet.label.clone(),
    }
}

fn save_share_action(request: ShareSaveRequest) -> super::PendingAction {
    super::PendingAction::SaveShare {
        friendship_id: request.friendship_id,
        share_id: request.share_id,
        enabled: request.enabled,
        precision: request.precision,
        duration_hours: request.duration_hours,
        label: request.label,
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
        let blocked =
            self.crash_dialog.is_some() || self.update_dialog_open || self.recovery_code_visible();
        if super::escape_dismisses(ctx.input(|i| i.key_pressed(egui::Key::Escape)), blocked) {
            self.share_sheet = None;
            return;
        }
        let compact = Self::is_compact(ctx);
        match Self::draw_share_sheet_view(ctx, &mut sheet, compact).action {
            ShareSheetAction::None => self.share_sheet = Some(sheet),
            ShareSheetAction::Close => self.share_sheet = None,
            ShareSheetAction::Save => {
                let request = share_save_request(&sheet, sheet.enabled);
                self.spawn_action(save_share_action(request), ctx);
                self.share_sheet = None;
            }
            ShareSheetAction::Stop => {
                if sheet.share_id.is_some() {
                    let request = share_save_request(&sheet, false);
                    self.spawn_action(save_share_action(request), ctx);
                }
                self.share_sheet = None;
            }
        }
    }

    pub(crate) fn draw_share_sheet_view(
        ctx: &egui::Context,
        sheet: &mut ShareSheet,
        compact: bool,
    ) -> ShareSheetOutcome {
        let panel = theme::PANEL.gamma_multiply(0.98);
        let border = theme::BORDER;
        let muted = theme::MUTED;
        let accent = theme::ACCENT;
        let mut save = false;
        let mut stop = false;
        let mut close = false;
        let mut close_id = None;
        let mut save_id = None;
        let mut stop_id = None;

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
                                    let close_button = ui.button("×");
                                    if close_button.clicked() {
                                        close = true;
                                    }
                                    close_id = Some(close_button.id);
                                    ui.memory_mut(|memory| {
                                        memory.interested_in_focus(
                                            close_button.id,
                                            close_button.layer_id,
                                        );
                                    });
                                },
                            );
                        });
                        ui.label(
                            RichText::new("Only what you allow below is shared with this friend.")
                                .size(11.0)
                                .color(muted),
                        );
                        ui.add_space(6.0);
                        let enabled =
                            ui.checkbox(&mut sheet.enabled, "Share my location with this friend");
                        if take_sheet_focus(sheet) {
                            enabled.request_focus();
                        }
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
                        ui.label(RichText::new("Label (optional)").strong());
                        ui.add(
                            egui::TextEdit::singleline(&mut sheet.label)
                                .id(egui::Id::new("share_label"))
                                .hint_text("e.g. Work trip")
                                .char_limit(120)
                                .desired_width(260.0),
                        );
                        ui.add_space(4.0);
                        ui.label(RichText::new("For how long").strong());
                        ui.horizontal_wrapped(|ui| {
                            if sheet.keep_current
                                && ui
                                    .add(egui::Button::selectable(
                                        sheet.duration_hours.is_none(),
                                        "Keep current",
                                    ))
                                    .clicked()
                            {
                                sheet.duration_hours = None;
                            }
                            for (hours, label) in [
                                (0, "Until off"),
                                (1, "1 h"),
                                (8, "8 h"),
                                (24, "24 h"),
                                (168, "7 days"),
                            ] {
                                if ui
                                    .add(egui::Button::selectable(
                                        sheet.duration_hours == Some(hours),
                                        label,
                                    ))
                                    .clicked()
                                {
                                    sheet.duration_hours = Some(hours);
                                }
                            }
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(&sheet.incoming).size(11.0).color(muted));
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            let save_button = ui.button("Save sharing");
                            if save_button.clicked() {
                                save = true;
                            }
                            save_id = Some(save_button.id);
                            let stop_button = ui.add_enabled(
                                sheet.share_id.is_some(),
                                egui::Button::new("Stop sharing"),
                            );
                            if stop_button.clicked() {
                                stop = true;
                            }
                            stop_id = Some(stop_button.id);
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

        ShareSheetOutcome {
            action: if close {
                ShareSheetAction::Close
            } else if save {
                ShareSheetAction::Save
            } else if stop {
                ShareSheetAction::Stop
            } else {
                ShareSheetAction::None
            },
            close_id,
            save_id,
            stop_id,
        }
    }
}

/// Consume the share sheet's one-shot focus request.
pub(crate) fn take_sheet_focus(sheet: &mut ShareSheet) -> bool {
    std::mem::take(&mut sheet.focus_enabled)
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
            expires_at: Some("2099-01-01T00:00:00Z".into()),
            label: None,
        }
    }

    fn sheet(share_id: Option<i64>, label: &str) -> ShareSheet {
        ShareSheet {
            friendship_id: 7,
            friend_name: "Bob".into(),
            share_id,
            enabled: true,
            precision: "approximate".into(),
            label: label.into(),
            duration_hours: Some(24),
            keep_current: false,
            incoming: String::new(),
            open: true,
            focus_enabled: true,
        }
    }

    #[test]
    fn save_request_carries_every_sheet_field() {
        let sheet = sheet(Some(9), "Home");
        assert_eq!(
            share_save_request(&sheet, sheet.enabled),
            ShareSaveRequest {
                friendship_id: 7,
                share_id: Some(9),
                enabled: true,
                precision: "approximate".into(),
                duration_hours: Some(24),
                label: "Home".into(),
            }
        );
    }

    #[test]
    fn stop_request_only_turns_the_same_share_off() {
        let sheet = sheet(Some(9), "Home");
        let stop = share_save_request(&sheet, false);
        assert!(!stop.enabled);
        assert_eq!(
            ShareSaveRequest {
                enabled: true,
                ..stop
            },
            share_save_request(&sheet, true)
        );
    }

    #[test]
    fn new_share_saves_without_an_id_or_label() {
        let sheet = sheet(None, "");
        let save = share_save_request(&sheet, sheet.enabled);
        assert_eq!(save.share_id, None);
        assert_eq!(save.label, "");
        assert!(save.enabled);
        // The app only sends Stop when a share id exists; the request itself
        // still carries no id for a new share.
        let stop = share_save_request(&sheet, false);
        assert_eq!(stop.share_id, None);
        assert!(!stop.enabled);
    }

    #[test]
    fn existing_outgoing_share_keeps_its_expiry_by_default() {
        let shares = vec![share(9, 7, "to_friend", true)];
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &shares);
        assert_eq!(sheet.share_id, Some(9));
        assert!(sheet.enabled);
        assert_eq!(sheet.precision, "approximate");
        assert_eq!(sheet.duration_hours, None, "saving must not extend sharing");
        assert!(sheet.keep_current);
        assert!(sheet.incoming.contains("does not currently share"));
    }

    #[test]
    fn no_share_defaults_to_off_exact_and_a_day() {
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[]);
        assert_eq!(sheet.share_id, None);
        assert!(!sheet.enabled);
        assert_eq!(sheet.precision, "exact");
        assert_eq!(sheet.duration_hours, Some(24));
        assert!(!sheet.keep_current);
    }

    #[test]
    fn existing_share_without_expiry_stays_until_off() {
        let mut existing = share(9, 7, "to_friend", true);
        existing.expires_at = None;
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[existing]);
        assert_eq!(sheet.duration_hours, Some(0));
        assert!(!sheet.keep_current);
    }

    #[test]
    fn expired_share_defaults_to_a_fresh_day() {
        let mut existing = share(9, 7, "to_friend", true);
        existing.expires_at = Some("2000-01-01T00:00:00Z".into());
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[existing]);
        assert_eq!(sheet.duration_hours, Some(24));
        assert!(!sheet.keep_current);
    }

    #[test]
    fn existing_share_loads_its_label() {
        let mut existing = share(9, 7, "to_friend", true);
        existing.label = Some("Work trip".into());
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[existing]);
        assert_eq!(sheet.label, "Work trip");
    }

    #[test]
    fn incoming_share_is_described() {
        let shares = vec![share(10, 7, "from_friend", true)];
        let sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &shares);
        assert!(sheet.incoming.contains("approximate"));
    }

    #[test]
    fn a_new_share_sheet_requests_focus_once() {
        let mut sheet = ShareSheet::for_friendship(&friendship(7), Some(1), &[]);
        assert!(take_sheet_focus(&mut sheet));
        assert!(!take_sheet_focus(&mut sheet));
    }
}
