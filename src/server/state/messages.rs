//! E2E message domain: public key bundles, opaque envelopes and read state.
//!
//! The server never decrypts a message. It validates that both users are
//! accepted friends, stores the signed envelope and enforces a unique
//! signature so a captured envelope cannot be replayed as a new message.

use super::{AppState, Store};
use crate::models::{Message, MessageConversation, MessagePeer, MessageThread, User};
use crate::security::message_crypto::MessagePublicBundle;
use chrono::Utc;

/// Upper bound accepted from the wire. The client limits text to 2,000 chars;
/// base64 + GCM overhead stays far below this.
pub const MAX_ENVELOPE_FIELD_BYTES: usize = 16 * 1024;

impl AppState {
    pub(super) async fn load_messages_domain(&self, store: &mut Store) {
        let Some(pool) = &self.pg else { return };
        let keys: Vec<(i64, String)> =
            sqlx::query_as("SELECT user_id, bundle FROM pinpoint_message_keys ORDER BY user_id")
                .fetch_all(pool)
                .await
                .unwrap_or_default();
        for (user_id, bundle) in keys {
            store.message_keys.insert(user_id, bundle);
        }

        // Demo scale: the store is the read model for threads and unread
        // counts. Move to per-query reads before a large public deployment.
        let rows: Vec<(
            i64,
            i64,
            i64,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT id, sender_id, recipient_id, ephemeral_public_key, salt_to, nonce_to, ciphertext_to, salt_from, nonce_from, ciphertext_from, signature, created_at, read_at FROM pinpoint_messages ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        for (
            id,
            sender_id,
            recipient_id,
            ephemeral_public_key,
            salt_to,
            nonce_to,
            ciphertext_to,
            salt_from,
            nonce_from,
            ciphertext_from,
            signature,
            created_at,
            read_at,
        ) in rows
        {
            store.next_message_id = store.next_message_id.max(id + 1);
            store.messages.push(Message {
                id,
                sender_id,
                recipient_id,
                ephemeral_public_key,
                salt_to,
                nonce_to,
                ciphertext_to,
                salt_from,
                nonce_from,
                ciphertext_from,
                signature,
                created_at,
                read_at,
            });
        }
    }

    pub fn message_key(&self, user_id: i64) -> Option<String> {
        self.store
            .lock()
            .unwrap()
            .message_keys
            .get(&user_id)
            .cloned()
    }

    /// Store or rotate a user's public bundle. Returns false on storage error.
    pub async fn set_message_key(&self, user_id: i64, bundle: String) -> bool {
        let now = Utc::now().to_rfc3339();
        if let Some(pool) = &self.pg {
            let result = sqlx::query(
                "INSERT INTO pinpoint_message_keys (user_id, bundle, updated_at) VALUES ($1,$2,$3) ON CONFLICT (user_id) DO UPDATE SET bundle = EXCLUDED.bundle, updated_at = EXCLUDED.updated_at",
            )
            .bind(user_id)
            .bind(&bundle)
            .bind(&now)
            .execute(pool)
            .await;
            if result.is_err() {
                return false;
            }
        }
        self.store
            .lock()
            .unwrap()
            .message_keys
            .insert(user_id, bundle);
        true
    }

    /// True when the two users have an accepted friendship (either direction).
    pub fn are_accepted_friends(&self, a: i64, b: i64) -> bool {
        if a == b {
            return false;
        }
        self.store.lock().unwrap().friendships.iter().any(|f| {
            f.status == "accepted"
                && ((f.requester_id == a && f.receiver_id == b)
                    || (f.requester_id == b && f.receiver_id == a))
        })
    }

    /// Store a sealed envelope. Duplicate signatures are rejected.
    pub async fn send_message(&self, mut message: Message) -> Option<Message> {
        if let Some(pool) = &self.pg {
            let row: Result<(i64,), _> = sqlx::query_as(
                "INSERT INTO pinpoint_messages (sender_id, recipient_id, ephemeral_public_key, salt_to, nonce_to, ciphertext_to, salt_from, nonce_from, ciphertext_from, signature, created_at, read_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id",
            )
            .bind(message.sender_id)
            .bind(message.recipient_id)
            .bind(&message.ephemeral_public_key)
            .bind(&message.salt_to)
            .bind(&message.nonce_to)
            .bind(&message.ciphertext_to)
            .bind(&message.salt_from)
            .bind(&message.nonce_from)
            .bind(&message.ciphertext_from)
            .bind(&message.signature)
            .bind(&message.created_at)
            .bind(&message.read_at)
            .fetch_one(pool)
            .await;
            match row {
                Ok((id,)) => message.id = id,
                Err(_) => return None,
            }
        } else {
            let mut store = self.store.lock().unwrap();
            if store
                .messages
                .iter()
                .any(|m| m.signature == message.signature)
            {
                return None;
            }
            message.id = store.next_message_id;
            store.next_message_id += 1;
            store.messages.push(message.clone());
            return Some(message);
        }
        self.store.lock().unwrap().messages.push(message.clone());
        Some(message)
    }

    /// Thread between two users, oldest first. `before_id` pages older rows.
    pub fn message_thread(
        &self,
        me: i64,
        friend: i64,
        before_id: Option<i64>,
        limit: usize,
    ) -> Vec<Message> {
        let store = self.store.lock().unwrap();
        let mut rows: Vec<Message> = store
            .messages
            .iter()
            .filter(|m| {
                (m.sender_id == me && m.recipient_id == friend)
                    || (m.sender_id == friend && m.recipient_id == me)
            })
            .filter(|m| before_id.is_none_or(|before| m.id < before))
            .cloned()
            .collect();
        rows.sort_by_key(|m| m.id);
        let take = limit.clamp(1, 100);
        if rows.len() > take {
            rows = rows.split_off(rows.len() - take);
        }
        rows
    }

    pub fn message_peer(&self, me: i64, friend: i64) -> Option<MessagePeer> {
        let store = self.store.lock().unwrap();
        let friendship = store.friendships.iter().find(|f| {
            f.status == "accepted"
                && ((f.requester_id == me && f.receiver_id == friend)
                    || (f.requester_id == friend && f.receiver_id == me))
        })?;
        let user = store.find_user_by_id(friend);
        let name = if friendship.requester_id == friend {
            friendship.requester_name.clone()
        } else {
            friendship.receiver_name.clone()
        };
        let display = user
            .map(|u| u.full_name.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| crate::server::usernames::username_for(friend));
        let key = store
            .message_keys
            .get(&friend)
            .and_then(|raw| serde_json::from_str::<MessagePublicBundle>(raw).ok());
        Some(MessagePeer {
            user_id: friend,
            full_name: if display.is_empty() { name } else { display },
            username: crate::server::usernames::username_for(friend),
            key,
        })
    }

    pub fn message_thread_for(
        &self,
        me: i64,
        friend: i64,
        before_id: Option<i64>,
        limit: usize,
    ) -> Option<MessageThread> {
        let peer = self.message_peer(me, friend)?;
        let messages = self.message_thread(me, friend, before_id, limit);
        Some(MessageThread { peer, messages })
    }

    /// Accepted friends with unread counts and last activity. No plaintext.
    pub fn message_conversations(&self, me: i64) -> Vec<MessageConversation> {
        let store = self.store.lock().unwrap();
        let mut out = Vec::new();
        for friendship in store.friendships.iter().filter(|f| f.status == "accepted") {
            let friend_id = if friendship.requester_id == me {
                friendship.receiver_id
            } else if friendship.receiver_id == me {
                friendship.requester_id
            } else {
                continue;
            };
            let name = if friendship.requester_id == friend_id {
                friendship.requester_name.clone()
            } else {
                friendship.receiver_name.clone()
            };
            let full_name = store
                .find_user_by_id(friend_id)
                .map(User::display_name)
                .filter(|name| !name.is_empty())
                .unwrap_or(name);
            let unread = store
                .messages
                .iter()
                .filter(|m| m.sender_id == friend_id && m.recipient_id == me && m.read_at.is_none())
                .count() as i64;
            let last_message_at = store
                .messages
                .iter()
                .filter(|m| {
                    (m.sender_id == me && m.recipient_id == friend_id)
                        || (m.sender_id == friend_id && m.recipient_id == me)
                })
                .map(|m| m.created_at.clone())
                .max();
            out.push(MessageConversation {
                user_id: friend_id,
                full_name,
                username: crate::server::usernames::username_for(friend_id),
                unread,
                last_message_at,
            });
        }
        out.sort_by(|a, b| {
            b.last_message_at
                .cmp(&a.last_message_at)
                .then_with(|| a.full_name.cmp(&b.full_name))
        });
        out
    }

    /// Mark every message from `friend` to `me` as read.
    pub async fn mark_messages_read(&self, me: i64, friend: i64) -> usize {
        let now = Utc::now().to_rfc3339();
        if let Some(pool) = &self.pg {
            let result = sqlx::query(
                "UPDATE pinpoint_messages SET read_at=$1 WHERE recipient_id=$2 AND sender_id=$3 AND read_at IS NULL",
            )
            .bind(&now)
            .bind(me)
            .bind(friend)
            .execute(pool)
            .await;
            if result.is_err() {
                return 0;
            }
        }
        let mut store = self.store.lock().unwrap();
        let mut changed = 0;
        for message in store.messages.iter_mut() {
            if message.recipient_id == me
                && message.sender_id == friend
                && message.read_at.is_none()
            {
                message.read_at = Some(now.clone());
                changed += 1;
            }
        }
        changed
    }

    /// Account deletion: remove keys and both message directions.
    pub async fn delete_user_messages(&self, user_id: i64) {
        if let Some(pool) = &self.pg {
            let _ =
                sqlx::query("DELETE FROM pinpoint_messages WHERE sender_id=$1 OR recipient_id=$1")
                    .bind(user_id)
                    .execute(pool)
                    .await;
            let _ = sqlx::query("DELETE FROM pinpoint_message_keys WHERE user_id=$1")
                .bind(user_id)
                .execute(pool)
                .await;
        }
        let mut store = self.store.lock().unwrap();
        store.message_keys.remove(&user_id);
        store
            .messages
            .retain(|m| m.sender_id != user_id && m.recipient_id != user_id);
    }
}

impl User {
    fn display_name(&self) -> String {
        if self.full_name.trim().is_empty() {
            self.username.clone()
        } else {
            self.full_name.clone()
        }
    }
}
