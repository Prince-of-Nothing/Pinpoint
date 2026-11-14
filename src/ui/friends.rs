//! Friends screen
use super::{PendingAction, PinpointAppUI};
use crate::models::Friendship;
use crate::ui::share_sheet::ShareSheet;
use eframe::egui::{self, Direction, RichText};

impl PinpointAppUI {
    pub fn draw_friends_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.draw_main_toolbar(ui);
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(Direction::LeftToRight),
                |ui| {
                    ui.heading("Friends Management");
                },
            );
        });

        let (my_id, friendships, shares) = {
            let binding = self.app.data();
            let data = binding.lock().unwrap();
            (
                data.user.as_ref().map(|u| u.id),
                data.friendships.clone(),
                data.shares.clone(),
            )
        };

        // Incoming requests (I am the receiver) — the only ones I can answer.
        ui.group(|ui| {
            ui.label(RichText::new("Received Requests").size(14.0).strong());
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
                    ui.horizontal(|ui| {
                        ui.label(name);
                        ui.add_space(4.0);
                        if ui.button("Accept").clicked() {
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
        ui.group(|ui| {
            ui.label(RichText::new("Sent Requests").size(14.0).strong());
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
                    ui.label(format!("{} (awaiting response)", req.receiver_name));
                }
            }
        });

        ui.add_space(8.0);

        // Accepted friends, with per-friend sharing and removal.
        ui.group(|ui| {
            ui.label(RichText::new("Friends").size(14.0).strong());
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
                    ui.horizontal(|ui| {
                        ui.label(&name);
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
                }
            }
        });

        ui.add_space(8.0);

        // Add new friend
        ui.group(|ui| {
            ui.label(RichText::new("Add Friend").size(14.0).strong());
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.friend_handle)
                    .hint_text("Username (calm-otter-42) or email")
                    .desired_width(220.0),
            );
            ui.label("Usernames are exact-match; badges shown here are handles.");
            ui.add_space(8.0);
            if ui.button("Send Request").clicked() && !self.friend_handle.trim().is_empty() {
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
