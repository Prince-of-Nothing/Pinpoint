//! E2E messages screen: inbox and a 1:1 thread. The app layer decrypts; this
//! module only renders plaintext and never touches ciphertext.

use super::{theme, PendingAction, PinpointAppUI};
use crate::models::{DecryptedMessage, MessageConversation, MessagePeer, SafetyState};
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_messages_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.message_loaded {
            self.message_loaded = true;
            self.spawn_action(PendingAction::RefreshMessages, ctx);
        }
        let now = std::time::Instant::now();
        if self
            .message_poll_at
            .is_none_or(|last| now.duration_since(last).as_secs() >= 5)
        {
            self.message_poll_at = Some(now);
            // An open thread must show incoming messages without a manual
            // re-entry; the inbox only needs its own refresh when closed.
            match self.message_selected {
                Some(friend_id) => {
                    self.spawn_action(PendingAction::LoadMessageThread { friend_id }, ctx);
                }
                None => {
                    self.spawn_action(PendingAction::RefreshMessages, ctx);
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(5));

        let (conversations, peer, messages, unread) = {
            let data = self.app.data();
            let data = data.lock().unwrap();
            (
                data.message_conversations.clone(),
                data.message_peer.clone(),
                data.message_messages.clone(),
                data.unread_messages,
            )
        };

        if Self::is_compact(ctx) {
            if self.message_selected.is_none() {
                self.draw_message_inbox(ui, ctx, &conversations, unread);
            } else {
                self.draw_message_thread(ui, ctx, true, peer.as_ref(), &messages);
            }
        } else {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(300.0, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.draw_message_inbox(ui, ctx, &conversations, unread),
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.draw_message_thread(ui, ctx, false, peer.as_ref(), &messages),
                );
            });
        }
    }

    fn draw_message_inbox(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        conversations: &[MessageConversation],
        unread: i64,
    ) {
        ui.horizontal(|ui| {
            ui.heading("Messages");
            if unread > 0 {
                ui.label(
                    RichText::new(format!("{unread} unread"))
                        .size(12.0)
                        .color(theme::ACCENT),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Refresh").clicked() {
                    self.spawn_action(PendingAction::RefreshMessages, ctx);
                }
            });
        });
        theme::caption(
            ui,
            "End-to-end encrypted 1:1 text. The server stores ciphertext only.",
        );
        ui.add_space(6.0);
        if conversations.is_empty() {
            ui.label("No friends yet. Add a friend to start an encrypted conversation.");
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("message_inbox")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for conversation in conversations {
                    let label = if conversation.unread > 0 {
                        format!("{}  ({})", conversation.full_name, conversation.unread)
                    } else {
                        conversation.full_name.clone()
                    };
                    let button =
                        egui::Button::new(RichText::new(label).size(14.0).color(theme::TEXT))
                            .fill(if conversation.unread > 0 {
                                theme::PANEL_2
                            } else {
                                theme::PANEL
                            })
                            .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                            .corner_radius(egui::CornerRadius::same(10))
                            .min_size(egui::vec2(ui.available_width(), 44.0));
                    if ui.add(button).clicked() {
                        self.select_message_friend(conversation.user_id, ctx);
                    }
                    if let Some(last) = &conversation.last_message_at {
                        theme::caption(ui, format!("Last message {last}"));
                    }
                    ui.add_space(4.0);
                }
            });
    }

    fn draw_message_thread(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        compact: bool,
        peer: Option<&MessagePeer>,
        messages: &[DecryptedMessage],
    ) {
        ui.horizontal_wrapped(|ui| {
            if compact && ui.button("Back").clicked() {
                self.message_selected = None;
            }
            match peer {
                Some(peer) => {
                    ui.heading(&peer.full_name);
                }
                None => {
                    ui.heading("Conversation");
                }
            }
        });
        if peer.is_none() {
            theme::caption(ui, "Loading or select a friend.");
        } else if peer.is_some_and(|peer| peer.key.is_none()) {
            theme::caption(ui, "Encryption key not published yet.");
        }
        if let Some(fingerprint) = peer
            .and_then(|peer| peer.key.as_ref())
            .and_then(|key| key.fingerprint())
        {
            let stored = peer.map(|peer| {
                let data = self.app.data();
                let data = data.lock().unwrap();
                data.message_verified.get(&peer.user_id).cloned()
            });
            let stored = stored.flatten();
            let state = match stored.as_deref() {
                Some(value) if value == fingerprint => SafetyState::Verified,
                Some(_) => SafetyState::Changed,
                None => SafetyState::Unverified,
            };
            ui.horizontal_wrapped(|ui| {
                ui.collapsing("Safety number", |ui| {
                    ui.monospace(&fingerprint);
                    theme::caption(
                        ui,
                        "Compare this with your friend in person to rule out a key swap.",
                    );
                });
                match state {
                    SafetyState::Verified => {
                        ui.label(RichText::new("Verified").size(12.0).color(theme::SUCCESS));
                    }
                    SafetyState::Changed => {
                        ui.label(RichText::new("Changed").size(12.0).color(theme::ACCENT));
                    }
                    SafetyState::Unverified => {}
                }
                if state != SafetyState::Verified {
                    if let Some(peer) = peer {
                        if ui.button("Mark as verified").clicked() {
                            self.app.verify_message_peer(peer.user_id, &fingerprint);
                        }
                    }
                }
            });
            if state == SafetyState::Changed {
                theme::caption(
                    ui,
                    "The safety number changed. Compare again before trusting it.",
                );
            }
        }
        ui.separator();
        if messages.is_empty() {
            ui.label("No messages yet. Say hello.");
        }
        egui::ScrollArea::vertical()
            .id_salt("message_thread")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height((ui.available_height() - 64.0).max(160.0))
            .show(ui, |ui| {
                for message in messages {
                    let text = message
                        .text
                        .as_deref()
                        .unwrap_or("Unable to decrypt this message (the key changed).");
                    let fill = if message.mine {
                        theme::ACCENT
                    } else {
                        theme::PANEL_2
                    };
                    let ink = if message.mine {
                        theme::ACCENT_INK
                    } else {
                        theme::TEXT
                    };
                    let layout = if message.mine {
                        egui::Layout::right_to_left(egui::Align::TOP)
                    } else {
                        egui::Layout::left_to_right(egui::Align::TOP)
                    };
                    ui.with_layout(layout, |ui| {
                        egui::Frame::new()
                            .fill(fill)
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                ui.set_max_width((ui.available_width() * 0.72).max(180.0));
                                ui.label(RichText::new(text).size(14.0).color(ink));
                                let time = message
                                    .created_at
                                    .get(11..16)
                                    .unwrap_or(message.created_at.as_str());
                                ui.label(RichText::new(time).size(10.0).color(if message.mine {
                                    theme::ACCENT_INK
                                } else {
                                    theme::MUTED
                                }));
                                if message.mine && message.read_at.is_some() {
                                    ui.label(
                                        RichText::new("Read").size(10.0).color(theme::ACCENT_INK),
                                    );
                                }
                            });
                    });
                    ui.add_space(4.0);
                }
            });
        ui.separator();
        let Some(friend_id) = self.message_selected else {
            return;
        };
        if peer.is_none() {
            return;
        }
        ui.horizontal(|ui| {
            let send_width = 74.0;
            let input_width = (ui.available_width() - send_width - 8.0).max(120.0);
            let input = ui.add_sized(
                [input_width, 44.0],
                egui::TextEdit::singleline(&mut self.message_draft)
                    .hint_text("Message (1-2000 characters)"),
            );
            let enter = input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if theme::primary_button(ui, "Send").clicked() || enter {
                let text = self.message_draft.trim().to_string();
                if !text.is_empty() && text.chars().count() <= 2000 {
                    self.message_draft.clear();
                    self.spawn_action(PendingAction::SendMessage { friend_id, text }, ctx);
                }
            }
        });
    }

    pub(crate) fn select_message_friend(&mut self, friend_id: i64, ctx: &egui::Context) {
        self.message_selected = Some(friend_id);
        self.message_draft.clear();
        {
            let data = self.app.data();
            let mut data = data.lock().unwrap();
            data.message_peer = None;
            data.message_messages.clear();
        }
        self.spawn_action(PendingAction::LoadMessageThread { friend_id }, ctx);
    }
}
