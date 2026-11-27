//! E2E messages screen: inbox and a 1:1 thread. The app layer decrypts; this
//! module only renders plaintext and never touches ciphertext.

use super::{theme, PendingAction, PinpointAppUI};
use crate::models::{
    format_message_time, DecryptedMessage, MessageConversation, MessagePeer, SafetyState,
};
use crate::security::message_crypto::safety_number;
use eframe::egui::{self, RichText};

impl PinpointAppUI {
    pub fn draw_messages_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pending = self.message_draft_restore.lock().unwrap().take();
        restore_message_draft(&mut self.message_draft, pending);
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
                        theme::caption(ui, format!("Last message {}", format_message_time(last)));
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
                self.message_focus_composer = false;
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
        let my_id = {
            let data = self.app.data();
            let id = data.lock().unwrap().user.as_ref().map(|user| user.id);
            id
        };
        let my_bundle = self.app.message_public_bundle();
        if let Some(peer) = peer {
            let safety = my_bundle.as_ref().and_then(|mine| {
                peer.key.as_ref().and_then(|theirs| {
                    my_id.and_then(|id| safety_number(mine, id, theirs, peer.user_id))
                })
            });
            if let Some(safety) = safety {
                let state = {
                    let data = self.app.data();
                    let data = data.lock().unwrap();
                    data.safety_state(peer.user_id, &safety)
                };
                // The number lives behind the "i" button, so the thread keeps
                // its room. The status word stays visible beside it.
                let info_id = egui::Id::new(("message_safety_info", peer.user_id));
                let mut info_open = ui
                    .ctx()
                    .data(|d| d.get_temp::<bool>(info_id))
                    .unwrap_or(false);
                ui.horizontal(|ui| {
                    let info = ui.add(egui::Button::new("i").min_size(egui::vec2(28.0, 28.0)));
                    info.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Safety number")
                    });
                    if info.on_hover_text("Safety number").clicked() {
                        info_open = !info_open;
                    }
                    match state {
                        SafetyState::Verified => {
                            ui.label(RichText::new("Verified").size(12.0).color(theme::SUCCESS));
                        }
                        SafetyState::Changed => {
                            ui.label(RichText::new("Changed").size(12.0).color(theme::ACCENT));
                        }
                        SafetyState::Unverified => {}
                    }
                });
                if info_open {
                    let mut open = true;
                    egui::Window::new("Safety number")
                        .open(&mut open)
                        .collapsible(false)
                        .resizable(false)
                        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                        .show(ui.ctx(), |ui| {
                            ui.monospace(&safety);
                            theme::caption(
                                ui,
                                "Compare this with your friend in person to rule out a key swap.",
                            );
                            ui.add_space(8.0);
                            ui.label(safety_status_text(state));
                            if can_mark_verified(state)
                                && theme::primary_button(ui, "Mark as verified").clicked()
                            {
                                self.app.verify_message_peer(peer.user_id, &safety);
                            }
                        });
                    info_open = open;
                }
                ui.data_mut(|d| d.insert_temp(info_id, info_open));
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
                                let time = format_message_time(&message.created_at);
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
                    .hint_text("Message (1-2000 characters)")
                    .char_limit(2000),
            );
            let enter = input.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if composer_should_focus(self.message_focus_composer, peer.is_some()) {
                input.request_focus();
                self.message_focus_composer = false;
            }
            if theme::primary_button(ui, "Send").clicked() || enter {
                if let Some(text) = message_to_send(&self.message_draft) {
                    self.message_draft.clear();
                    self.spawn_action(PendingAction::SendMessage { friend_id, text }, ctx);
                }
            }
        });
    }

    pub(crate) fn select_message_friend(&mut self, friend_id: i64, ctx: &egui::Context) {
        self.message_selected = Some(friend_id);
        self.message_draft.clear();
        self.message_focus_composer = true;
        {
            let data = self.app.data();
            let mut data = data.lock().unwrap();
            data.message_peer = None;
            data.message_messages.clear();
        }
        self.spawn_action(PendingAction::LoadMessageThread { friend_id }, ctx);
    }
}

/// Put a failed send back in the composer, unless the user has typed a newer
/// draft in the meantime (the newer text wins).
pub(crate) fn restore_message_draft(current: &mut String, pending: Option<String>) {
    if current.is_empty() {
        if let Some(text) = pending {
            *current = text;
        }
    }
}

/// The text the composer may send: trimmed, not empty and at most 2000 characters.
pub(crate) fn message_to_send(draft: &str) -> Option<String> {
    let text = draft.trim();
    (!text.is_empty() && text.chars().count() <= 2000).then(|| text.to_string())
}

/// The composer asks for focus once, after the opened chat's peer has loaded.
pub(crate) fn composer_should_focus(requested: bool, peer_ready: bool) -> bool {
    requested && peer_ready
}

/// One line about a friend's safety number, shown in the info window.
pub(crate) fn safety_status_text(state: SafetyState) -> &'static str {
    match state {
        SafetyState::Unverified => "Not compared with your friend yet.",
        SafetyState::Verified => "You checked this number.",
        SafetyState::Changed => "The number changed, so compare it again before you trust it.",
    }
}

/// Only a number that is not yet checked offers "Mark as verified".
pub(crate) fn can_mark_verified(state: SafetyState) -> bool {
    state != SafetyState::Verified
}

#[cfg(test)]
mod tests {
    use super::{composer_should_focus, message_to_send, restore_message_draft};
    use crate::models::SafetyState;

    #[test]
    fn empty_and_whitespace_drafts_are_not_sent() {
        assert_eq!(message_to_send(""), None);
        assert_eq!(message_to_send("  \n\t "), None);
    }

    #[test]
    fn a_normal_draft_is_trimmed_before_sending() {
        assert_eq!(
            message_to_send("  see you at 18:00 \n"),
            Some("see you at 18:00".to_string())
        );
    }

    #[test]
    fn the_2000_character_limit_is_inclusive() {
        let at_limit = "a".repeat(2000);
        assert_eq!(message_to_send(&at_limit), Some(at_limit.clone()));

        let over_limit = "a".repeat(2001);
        assert_eq!(message_to_send(&over_limit), None);
    }

    #[test]
    fn composer_focus_waits_for_the_peer_and_runs_once() {
        assert!(!composer_should_focus(false, true));
        assert!(!composer_should_focus(true, false));
        assert!(composer_should_focus(true, true));
    }

    #[test]
    fn failed_send_restores_only_an_empty_composer() {
        let mut draft = String::new();
        restore_message_draft(&mut draft, Some("lost text".into()));
        assert_eq!(draft, "lost text");

        let mut newer = "typed again".to_string();
        restore_message_draft(&mut newer, Some("old text".into()));
        assert_eq!(newer, "typed again");
        restore_message_draft(&mut newer, None);
        assert_eq!(newer, "typed again");
    }

    #[test]
    fn an_unchecked_number_says_it_has_not_been_compared() {
        assert_eq!(
            super::safety_status_text(SafetyState::Unverified),
            "Not compared with your friend yet."
        );
    }

    #[test]
    fn a_checked_number_says_so() {
        assert_eq!(
            super::safety_status_text(SafetyState::Verified),
            "You checked this number."
        );
    }

    #[test]
    fn a_changed_number_asks_for_a_new_comparison() {
        assert!(super::safety_status_text(SafetyState::Changed).contains("compare it again"));
    }

    #[test]
    fn only_a_number_not_yet_checked_offers_mark_as_verified() {
        assert!(super::can_mark_verified(SafetyState::Unverified));
        assert!(super::can_mark_verified(SafetyState::Changed));
        assert!(!super::can_mark_verified(SafetyState::Verified));
    }
}
