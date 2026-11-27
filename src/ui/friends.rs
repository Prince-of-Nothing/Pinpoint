//! Friends screen
use super::theme;
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use crate::models::{FriendLocation, Friendship};
use crate::ui::share_sheet::ShareSheet;
use eframe::egui::{self};

impl PinpointAppUI {
    pub fn draw_friends_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::ScrollArea::vertical()
            .id_salt("friends_screen")
            .auto_shrink([false, false])
            .show(ui, |ui| self.draw_friends_body(ui, ctx));
    }

    fn draw_friends_body(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Friends Management");
        ui.add_space(4.0);
        theme::caption(ui, "Requests, accepted friends and sharing");
        ui.add_space(10.0);

        let (my_id, friendships, shares, locations) = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            (
                data.user.as_ref().map(|u| u.id),
                data.friendships.clone(),
                data.shares.clone(),
                data.locations.clone(),
            )
        };
        let now = chrono::Utc::now().timestamp();

        // Incoming requests (I am the receiver) — the only ones I can answer.
        theme::card(ui, |ui| {
            theme::card_title(ui, "Received Requests");
            ui.add_space(4.0);

            let received: Vec<Friendship> = friendships
                .iter()
                .filter(|f| f.status == "pending" && Some(f.receiver_id) == my_id)
                .cloned()
                .collect();

            if received.is_empty() {
                ui.label("No pending friend requests");
            } else {
                for req in &received {
                    let id = req.id;
                    let name = req.requester_name.clone();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(name);
                        ui.add_space(4.0);
                        if theme::primary_button(ui, "Accept").clicked() {
                            self.spawn_action(
                                PendingAction::RespondFriendship {
                                    id,
                                    action: "accept".to_string(),
                                },
                                ctx,
                            );
                        }
                        if ui.button("Reject").clicked() {
                            self.spawn_action(
                                PendingAction::RespondFriendship {
                                    id,
                                    action: "reject".to_string(),
                                },
                                ctx,
                            );
                        }
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Outgoing requests (I am the requester) — no actions available.
        theme::card(ui, |ui| {
            theme::card_title(ui, "Sent Requests");
            ui.add_space(4.0);

            let sent: Vec<Friendship> = friendships
                .iter()
                .filter(|f| f.status == "pending" && Some(f.requester_id) == my_id)
                .cloned()
                .collect();

            if sent.is_empty() {
                ui.label("No sent requests");
            } else {
                for req in &sent {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} (awaiting response)", req.receiver_name));
                        if ui.button("Cancel").clicked() {
                            self.spawn_action(PendingAction::DeleteFriendship { id: req.id }, ctx);
                        }
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Accepted friends, with per-friend sharing and removal.
        theme::card(ui, |ui| {
            theme::card_title(ui, "Friends");
            ui.add_space(4.0);
            let accepted: Vec<Friendship> = friendships
                .iter()
                .filter(|f| f.status == "accepted")
                .cloned()
                .collect();
            if accepted.is_empty() {
                ui.label("No accepted friends yet.");
            } else {
                for friend in &accepted {
                    let name = if Some(friend.requester_id) == my_id {
                        friend.receiver_name.clone()
                    } else {
                        friend.requester_name.clone()
                    };
                    let friend_id = if Some(friend.requester_id) == my_id {
                        friend.receiver_id
                    } else {
                        friend.requester_id
                    };
                    let sharing = friend_sharing_line(
                        locations.iter().find(|loc| loc.user_id == friend_id),
                        now,
                    );
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(&name);
                            if ui.button("Message").clicked() {
                                self.select_message_friend(friend_id, ctx);
                                self.app.set_screen(CurrentScreen::Messages);
                            }
                            if ui.button("Sharing").clicked() {
                                self.share_sheet =
                                    Some(ShareSheet::for_friendship(friend, my_id, &shares));
                            }
                            if ui.button("Remove").clicked() {
                                self.spawn_action(
                                    PendingAction::DeleteFriendship { id: friend.id },
                                    ctx,
                                );
                            }
                        });
                        theme::caption(ui, sharing);
                    });
                }
            }
        });

        ui.add_space(8.0);

        // Add new friend
        theme::card(ui, |ui| {
            theme::card_title(ui, "Add Friend");
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.friend_handle)
                    .hint_text("Username (calm-otter-42) or email")
                    .desired_width(220.0),
            );
            ui.label("Usernames are exact-match; badges shown here are handles.");
            ui.add_space(8.0);
            if theme::primary_button(ui, "Send Request").clicked()
                && !self.friend_handle.trim().is_empty()
            {
                self.spawn_action(
                    PendingAction::SendFriendRequest {
                        handle: self.friend_handle.clone(),
                    },
                    ctx,
                );
            }
        });
    }
}

/// One line under a friend's name: what they share with you, and how old it
/// is. A friend who shares nothing says so, so the privacy state is always
/// visible.
pub(crate) fn friend_sharing_line(share: Option<&FriendLocation>, now_unix: i64) -> String {
    let Some(share) = share else {
        return "Not sharing location with you".to_string();
    };
    let precision = match share.precision_level.as_str() {
        "exact" => "Exact",
        "approximate" => "Approximate",
        "city_only" => "City only",
        _ => "Unknown precision",
    };
    let age = match crate::models::age_secs(&share.updated_at, now_unix) {
        Some(secs) if secs < 60 => "live now".to_string(),
        Some(secs) if secs < 3600 => format!("{} min ago", secs / 60),
        Some(secs) if secs < 86_400 => format!("{} h ago", secs / 3600),
        Some(secs) => format!("{} d ago", secs / 86_400),
        None => "time unknown".to_string(),
    };
    format!("{precision} · {age}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_795_000_000;

    fn share(precision: &str, age_secs: i64) -> FriendLocation {
        let at = chrono::DateTime::from_timestamp(NOW - age_secs, 0)
            .unwrap()
            .to_rfc3339();
        FriendLocation {
            user_id: 7,
            user_name: "Sam".into(),
            latitude: 47.0,
            longitude: 28.8,
            precision_level: precision.into(),
            updated_at: at,
            accuracy: None,
        }
    }

    #[test]
    fn friend_who_shares_nothing_is_marked_not_sharing() {
        assert_eq!(
            friend_sharing_line(None, NOW),
            "Not sharing location with you"
        );
    }

    #[test]
    fn a_location_updated_just_now_reads_live_now() {
        let line = friend_sharing_line(Some(&share("exact", 10)), NOW);
        assert_eq!(line, "Exact · live now");
    }

    #[test]
    fn a_location_a_few_minutes_old_reads_in_minutes() {
        let line = friend_sharing_line(Some(&share("approximate", 5 * 60)), NOW);
        assert_eq!(line, "Approximate · 5 min ago");
    }

    #[test]
    fn an_old_city_only_location_reads_in_hours() {
        let line = friend_sharing_line(Some(&share("city_only", 2 * 3600 + 30)), NOW);
        assert_eq!(line, "City only · 2 h ago");
    }

    #[test]
    fn a_location_with_no_readable_time_says_so() {
        let mut loc = share("exact", 0);
        loc.updated_at = "not a time".into();
        assert_eq!(friend_sharing_line(Some(&loc), NOW), "Exact · time unknown");
    }
}
