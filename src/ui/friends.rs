//! Friends screen
use super::theme;
use super::{PendingAction, PinpointAppUI};
use crate::app::CurrentScreen;
use crate::models::Friendship;
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
                    ui.label(format!("{} (awaiting response)", req.receiver_name));
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
                    ui.horizontal_wrapped(|ui| {
                        ui.label(&name);
                        if ui.button("Message").clicked() {
                            let friend_id = if Some(friend.requester_id) == my_id {
                                friend.receiver_id
                            } else {
                                friend.requester_id
                            };
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
