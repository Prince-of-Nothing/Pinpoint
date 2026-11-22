//! End-to-end encryption for 1:1 text messages.
//!
//! This is an ECIES-style construction over audited primitives, not a full
//! Signal Protocol implementation:
//!
//! * each account has a long-lived X25519 key and an Ed25519 signing key;
//! * the sender generates an ephemeral X25519 key per message, derives two
//!   AES-256-GCM keys with HKDF-SHA256 (one for the recipient, one for the
//!   sender's own copy) and signs the sealed envelope with Ed25519;
//! * the server stores only the opaque envelope and cannot read the text or
//!   forge a message that verifies under the sender's key.
//!
//! Limitations are documented in ADR-0023: no double ratchet, so a stolen
//! long-lived private key can decrypt past messages; one key bundle per
//! account, so signing in on a new device rotates the key and older messages
//! stay on the old device; no groups, attachments, calls or multi-device
//! fan-out.

use aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use hybrid_array::Array;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use typenum::{U12, U32};
use x25519_dalek::{PublicKey, StaticSecret};

const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;
const TO_INFO: &[u8] = b"pinpoint-message-v1-to";
const FROM_INFO: &[u8] = b"pinpoint-message-v1-from";

/// Maximum plaintext accepted by the composer. The server enforces the
/// corresponding ciphertext ceiling independently.
pub const MAX_MESSAGE_CHARS: usize = 2000;

/// Long-lived identity keys for one account on one device.
#[derive(Clone)]
pub struct MessageIdentity {
    x25519_secret: [u8; KEY_LEN],
    ed25519_secret: [u8; KEY_LEN],
}

/// Public half uploaded to the server and fetched by friends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagePublicBundle {
    pub x25519: String,
    pub ed25519: String,
}

/// Opaque envelope stored by the server. Every field is base64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedMessage {
    pub ephemeral_public_key: String,
    pub salt_to: String,
    pub nonce_to: String,
    pub ciphertext_to: String,
    pub salt_from: String,
    pub nonce_from: String,
    pub ciphertext_from: String,
    pub signature: String,
}

impl MessageIdentity {
    /// Generate a fresh identity. The caller must persist the secret bundle in
    /// the OS keychain; losing it makes the account's messages unreadable.
    pub fn generate() -> Self {
        let x25519 = StaticSecret::random_from_rng(OsRng).to_bytes();
        let ed25519 = SigningKey::generate(&mut OsRng).to_bytes();
        Self {
            x25519_secret: x25519,
            ed25519_secret: ed25519,
        }
    }

    /// Restore an identity from the JSON stored in the keychain.
    pub fn from_secret_bundle(raw: &str) -> Option<Self> {
        let stored: SecretBundle = serde_json::from_str(raw).ok()?;
        Some(Self {
            x25519_secret: decode_fixed::<KEY_LEN>(&stored.x25519)?,
            ed25519_secret: decode_fixed::<KEY_LEN>(&stored.ed25519)?,
        })
    }

    /// JSON for the keychain. Never log or send this.
    pub fn secret_bundle(&self) -> String {
        serde_json::json!({
            "x25519": B64.encode(self.x25519_secret),
            "ed25519": B64.encode(self.ed25519_secret),
        })
        .to_string()
    }

    /// Public bundle for the server.
    pub fn public_bundle(&self) -> MessagePublicBundle {
        MessagePublicBundle {
            x25519: B64.encode(PublicKey::from(&StaticSecret::from(self.x25519_secret)).as_bytes()),
            ed25519: B64.encode(
                SigningKey::from_bytes(&self.ed25519_secret)
                    .verifying_key()
                    .as_bytes(),
            ),
        }
    }

    fn signing_key(&self) -> SigningKey {
        SigningKey::from_bytes(&self.ed25519_secret)
    }
}

impl MessagePublicBundle {
    /// Decode and validate both keys.
    pub fn parse(&self) -> Option<(PublicKey, VerifyingKey)> {
        let x = decode_fixed::<KEY_LEN>(&self.x25519)?;
        let e = decode_fixed::<KEY_LEN>(&self.ed25519)?;
        let verifying = VerifyingKey::from_bytes(&e).ok()?;
        Some((PublicKey::from(x), verifying))
    }

    /// Single-bundle fingerprint (first 16 bytes of SHA-256). Diagnostics
    /// only; the value users compare is [`safety_number`].
    pub fn fingerprint(&self) -> Option<String> {
        let (x, e) = self.parse()?;
        let mut hasher = Sha256::new();
        hasher.update(x.as_bytes());
        hasher.update(e.as_bytes());
        let digest = hasher.finalize();
        Some(
            digest
                .iter()
                .take(16)
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join(":"),
        )
    }

    pub fn is_valid(&self) -> bool {
        self.parse().is_some()
    }
}

/// Symmetric safety number for a pair of users.
///
/// Both sides hash the same bytes because the bundles are ordered by user id,
/// so Alice and Bob can compare the same string in person. The domain
/// separator keeps this distinct from the single-bundle fingerprint.
pub fn safety_number(
    a_bundle: &MessagePublicBundle,
    a_id: i64,
    b_bundle: &MessagePublicBundle,
    b_id: i64,
) -> Option<String> {
    if a_id <= 0 || b_id <= 0 || a_id == b_id {
        return None;
    }
    let (low_id, low, high_id, high) = if a_id < b_id {
        (a_id, a_bundle, b_id, b_bundle)
    } else {
        (b_id, b_bundle, a_id, a_bundle)
    };
    let (low_x, low_e) = low.parse()?;
    let (high_x, high_e) = high.parse()?;
    let mut hasher = Sha256::new();
    hasher.update(b"pinpoint-safety-v1");
    hasher.update(low_id.to_le_bytes());
    hasher.update(low_x.as_bytes());
    hasher.update(low_e.as_bytes());
    hasher.update(high_id.to_le_bytes());
    hasher.update(high_x.as_bytes());
    hasher.update(high_e.as_bytes());
    let digest = hasher.finalize();
    Some(
        digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

/// Seal `plaintext` for `recipient` and keep a second copy readable by the
/// sender. Returns `None` for an invalid recipient bundle or invalid text.
pub fn seal(
    sender: &MessageIdentity,
    recipient: &MessagePublicBundle,
    sender_id: i64,
    recipient_id: i64,
    plaintext: &str,
) -> Option<SealedMessage> {
    if sender_id <= 0 || recipient_id <= 0 || sender_id == recipient_id {
        return None;
    }
    let text = plaintext.trim();
    if text.is_empty() || text.chars().count() > MAX_MESSAGE_CHARS {
        return None;
    }
    let (recipient_x, _) = recipient.parse()?;
    let sender_x = PublicKey::from(&StaticSecret::from(sender.x25519_secret));

    let ephemeral_secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    let to_shared = ephemeral_secret.diffie_hellman(&recipient_x);
    let from_shared = ephemeral_secret.diffie_hellman(&sender_x);

    let mut salt_to = [0u8; SALT_LEN];
    let mut salt_from = [0u8; SALT_LEN];
    let mut nonce_to = [0u8; NONCE_LEN];
    let mut nonce_from = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt_to);
    OsRng.fill_bytes(&mut salt_from);
    OsRng.fill_bytes(&mut nonce_to);
    OsRng.fill_bytes(&mut nonce_from);

    let to_key = derive_key(to_shared.as_bytes(), &salt_to, TO_INFO)?;
    let from_key = derive_key(from_shared.as_bytes(), &salt_from, FROM_INFO)?;
    let cleartext = text.as_bytes();
    let ciphertext_to = encrypt(&to_key, &nonce_to, cleartext)?;
    let ciphertext_from = encrypt(&from_key, &nonce_from, cleartext)?;

    let signature = sign_envelope(
        &sender.signing_key(),
        sender_id,
        recipient_id,
        ephemeral_public.as_bytes(),
        &salt_to,
        &nonce_to,
        &ciphertext_to,
        &salt_from,
        &nonce_from,
        &ciphertext_from,
    );

    Some(SealedMessage {
        ephemeral_public_key: B64.encode(ephemeral_public.as_bytes()),
        salt_to: B64.encode(salt_to),
        nonce_to: B64.encode(nonce_to),
        ciphertext_to: B64.encode(ciphertext_to),
        salt_from: B64.encode(salt_from),
        nonce_from: B64.encode(nonce_from),
        ciphertext_from: B64.encode(ciphertext_from),
        signature: B64.encode(signature),
    })
}

/// Open a sealed message. `as_recipient` selects the recipient copy; the
/// sender passes `false` for their own copy. Signature verification happens
/// before any decryption attempt.
pub fn open(
    identity: &MessageIdentity,
    sender_bundle: &MessagePublicBundle,
    sender_id: i64,
    recipient_id: i64,
    sealed: &SealedMessage,
    as_recipient: bool,
) -> Option<String> {
    let (_, verifying) = sender_bundle.parse()?;
    let ephemeral = decode_fixed::<KEY_LEN>(&sealed.ephemeral_public_key)?;
    let salt = decode_fixed::<SALT_LEN>(if as_recipient {
        &sealed.salt_to
    } else {
        &sealed.salt_from
    })?;
    let nonce = decode_fixed::<NONCE_LEN>(if as_recipient {
        &sealed.nonce_to
    } else {
        &sealed.nonce_from
    })?;
    let ciphertext = B64
        .decode(if as_recipient {
            &sealed.ciphertext_to
        } else {
            &sealed.ciphertext_from
        })
        .ok()?;
    let signature = decode_fixed::<SIGNATURE_LEN>(&sealed.signature)?;

    let payload = signing_payload(
        sender_id,
        recipient_id,
        &ephemeral,
        &decode_fixed::<SALT_LEN>(&sealed.salt_to)?,
        &decode_fixed::<NONCE_LEN>(&sealed.nonce_to)?,
        &B64.decode(&sealed.ciphertext_to).ok()?,
        &decode_fixed::<SALT_LEN>(&sealed.salt_from)?,
        &decode_fixed::<NONCE_LEN>(&sealed.nonce_from)?,
        &B64.decode(&sealed.ciphertext_from).ok()?,
    );
    verifying
        .verify_strict(&payload, &Signature::from_bytes(&signature))
        .ok()?;

    let shared =
        StaticSecret::from(identity.x25519_secret).diffie_hellman(&PublicKey::from(ephemeral));
    let info = if as_recipient { TO_INFO } else { FROM_INFO };
    let key = derive_key(shared.as_bytes(), &salt, info)?;
    let cleartext = decrypt(&key, &nonce, &ciphertext)?;
    let text = String::from_utf8(cleartext).ok()?;
    if text.is_empty() || text.chars().count() > MAX_MESSAGE_CHARS {
        return None;
    }
    Some(text)
}

#[derive(Serialize, Deserialize)]
struct SecretBundle {
    x25519: String,
    ed25519: String,
}

fn derive_key(shared: &[u8; KEY_LEN], salt: &[u8; SALT_LEN], info: &[u8]) -> Option<[u8; KEY_LEN]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), shared);
    let mut key = [0u8; KEY_LEN];
    hk.expand(info, &mut key).ok()?;
    Some(key)
}

fn encrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], plaintext: &[u8]) -> Option<Vec<u8>> {
    let key_ga: Array<u8, U32> = (*key).into();
    let nonce_ga: Array<u8, U12> = (*nonce).into();
    Aes256Gcm::new(&key_ga).encrypt(&nonce_ga, plaintext).ok()
}

fn decrypt(key: &[u8; KEY_LEN], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let key_ga: Array<u8, U32> = (*key).into();
    let nonce_ga: Array<u8, U12> = (*nonce).into();
    Aes256Gcm::new(&key_ga).decrypt(&nonce_ga, ciphertext).ok()
}

#[allow(clippy::too_many_arguments)]
fn sign_envelope(
    signing: &SigningKey,
    sender_id: i64,
    recipient_id: i64,
    ephemeral: &[u8; KEY_LEN],
    salt_to: &[u8; SALT_LEN],
    nonce_to: &[u8; NONCE_LEN],
    ciphertext_to: &[u8],
    salt_from: &[u8; SALT_LEN],
    nonce_from: &[u8; NONCE_LEN],
    ciphertext_from: &[u8],
) -> [u8; SIGNATURE_LEN] {
    signing
        .sign(&signing_payload(
            sender_id,
            recipient_id,
            ephemeral,
            salt_to,
            nonce_to,
            ciphertext_to,
            salt_from,
            nonce_from,
            ciphertext_from,
        ))
        .to_bytes()
}

#[allow(clippy::too_many_arguments)]
fn signing_payload(
    sender_id: i64,
    recipient_id: i64,
    ephemeral: &[u8; KEY_LEN],
    salt_to: &[u8; SALT_LEN],
    nonce_to: &[u8; NONCE_LEN],
    ciphertext_to: &[u8],
    salt_from: &[u8; SALT_LEN],
    nonce_from: &[u8; NONCE_LEN],
    ciphertext_from: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 16 + 7 * (4 + 32));
    out.push(VERSION);
    out.extend_from_slice(&sender_id.to_le_bytes());
    out.extend_from_slice(&recipient_id.to_le_bytes());
    for part in [
        ephemeral.as_slice(),
        salt_to.as_slice(),
        nonce_to.as_slice(),
        ciphertext_to,
        salt_from.as_slice(),
        nonce_from.as_slice(),
        ciphertext_from,
    ] {
        out.extend_from_slice(&(part.len() as u32).to_le_bytes());
        out.extend_from_slice(part);
    }
    out
}

fn decode_fixed<const N: usize>(encoded: &str) -> Option<[u8; N]> {
    let bytes = B64.decode(encoded).ok()?;
    if bytes.len() != N {
        return None;
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (MessageIdentity, MessageIdentity) {
        (MessageIdentity::generate(), MessageIdentity::generate())
    }

    #[test]
    fn secret_and_public_bundles_round_trip() {
        let alice = MessageIdentity::generate();
        let restored = MessageIdentity::from_secret_bundle(&alice.secret_bundle()).unwrap();
        assert_eq!(alice.public_bundle(), restored.public_bundle());
        assert!(alice.public_bundle().is_valid());
        assert_eq!(alice.public_bundle().x25519.len(), 44);
        assert_eq!(alice.public_bundle().ed25519.len(), 44);
    }

    #[test]
    fn recipient_and_sender_can_both_open() {
        let (alice, bob) = pair();
        let sealed = seal(&alice, &bob.public_bundle(), 1, 2, "Meet at the fountain").unwrap();
        assert_eq!(
            open(&bob, &alice.public_bundle(), 1, 2, &sealed, true).unwrap(),
            "Meet at the fountain"
        );
        assert_eq!(
            open(&alice, &alice.public_bundle(), 1, 2, &sealed, false).unwrap(),
            "Meet at the fountain"
        );
    }

    #[test]
    fn wrong_identity_or_role_fails() {
        let (alice, bob) = pair();
        let eve = MessageIdentity::generate();
        let sealed = seal(&alice, &bob.public_bundle(), 1, 2, "secret").unwrap();
        assert!(open(&eve, &alice.public_bundle(), 1, 2, &sealed, true).is_none());
        assert!(open(&bob, &alice.public_bundle(), 1, 2, &sealed, false).is_none());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let (alice, bob) = pair();
        let mut sealed = seal(&alice, &bob.public_bundle(), 1, 2, "secret").unwrap();
        let mut bytes = B64.decode(&sealed.ciphertext_to).unwrap();
        bytes[0] ^= 0x01;
        sealed.ciphertext_to = B64.encode(bytes);
        assert!(open(&bob, &alice.public_bundle(), 1, 2, &sealed, true).is_none());
    }

    #[test]
    fn forged_signature_fails() {
        let (alice, bob) = pair();
        let mut sealed = seal(&alice, &bob.public_bundle(), 1, 2, "secret").unwrap();
        sealed.signature = B64.encode([7u8; SIGNATURE_LEN]);
        assert!(open(&bob, &alice.public_bundle(), 1, 2, &sealed, true).is_none());
    }

    #[test]
    fn sender_cannot_be_swapped() {
        let (alice, bob) = pair();
        let (carol, _) = pair();
        let sealed = seal(&alice, &bob.public_bundle(), 1, 2, "secret").unwrap();
        // Carol's bundle must not verify a message signed by Alice.
        assert!(open(&bob, &carol.public_bundle(), 1, 2, &sealed, true).is_none());
    }

    #[test]
    fn previous_sender_bundle_verifies_before_rotation() {
        let old_alice = MessageIdentity::generate();
        let bob = MessageIdentity::generate();
        let sealed = seal(&old_alice, &bob.public_bundle(), 1, 2, "before rotation").unwrap();
        let new_alice = MessageIdentity::generate();
        // The new bundle is the current one the server returns; it must not
        // verify the old message, while the cached previous bundle must.
        assert!(open(&bob, &new_alice.public_bundle(), 1, 2, &sealed, true).is_none());
        assert_eq!(
            open(&bob, &old_alice.public_bundle(), 1, 2, &sealed, true).unwrap(),
            "before rotation"
        );
    }

    #[test]
    fn invalid_text_is_rejected() {
        let (alice, bob) = pair();
        assert!(seal(&alice, &bob.public_bundle(), 1, 2, "   ").is_none());
        assert!(seal(
            &alice,
            &bob.public_bundle(),
            1,
            2,
            &"x".repeat(MAX_MESSAGE_CHARS + 1)
        )
        .is_none());
        assert!(seal(&alice, &bob.public_bundle(), 1, 1, "self").is_none());
    }

    #[test]
    fn fingerprint_is_stable_and_distinct() {
        let (alice, bob) = pair();
        assert_eq!(
            alice.public_bundle().fingerprint().unwrap(),
            alice.public_bundle().fingerprint().unwrap()
        );
        assert_ne!(
            alice.public_bundle().fingerprint().unwrap(),
            bob.public_bundle().fingerprint().unwrap()
        );
    }

    #[test]
    fn safety_number_is_symmetric_and_pair_bound() {
        let (alice, bob) = pair();
        let (carol, _) = pair();
        let ab = safety_number(&alice.public_bundle(), 1, &bob.public_bundle(), 2).unwrap();
        let ba = safety_number(&bob.public_bundle(), 2, &alice.public_bundle(), 1).unwrap();
        assert_eq!(ab, ba, "both users must see the same number");
        assert_eq!(ab.len(), 47);
        let ac = safety_number(&alice.public_bundle(), 1, &carol.public_bundle(), 3).unwrap();
        assert_ne!(ab, ac);
        assert!(safety_number(&alice.public_bundle(), 1, &bob.public_bundle(), 1).is_none());
        assert!(safety_number(&alice.public_bundle(), 0, &bob.public_bundle(), 2).is_none());
    }

    #[test]
    fn safety_vector_matches_the_web_implementation() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../web/app/test/safety-vector.json"))
                .expect("safety fixture");
        let alice: MessagePublicBundle = serde_json::from_value(fixture["alice"].clone()).unwrap();
        let bob: MessagePublicBundle = serde_json::from_value(fixture["bob"].clone()).unwrap();
        let expected = fixture["expected"].as_str().unwrap();
        assert_eq!(
            safety_number(
                &alice,
                fixture["alice_id"].as_i64().unwrap(),
                &bob,
                fixture["bob_id"].as_i64().unwrap(),
            )
            .as_deref(),
            Some(expected)
        );
        assert_eq!(
            safety_number(
                &bob,
                fixture["bob_id"].as_i64().unwrap(),
                &alice,
                fixture["alice_id"].as_i64().unwrap(),
            )
            .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn js_sealed_message_opens_in_rust() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../web/app/test/message-js-vector.json"))
            .expect("JS cross-language fixture");
    let bob = MessageIdentity::from_secret_bundle(
        &serde_json::json!({
            "x25519": B64.encode([3u8; 32]),
            "ed25519": B64.encode([4u8; 32]),
        })
        .to_string(),
    )
    .unwrap();
    let sender: MessagePublicBundle = serde_json::from_value(fixture["js_public"].clone()).unwrap();
    let sealed: SealedMessage = serde_json::from_value(fixture["sealed"].clone()).unwrap();
    let sender_id = fixture["sender_id"].as_i64().unwrap();
    let recipient_id = fixture["recipient_id"].as_i64().unwrap();
    assert_eq!(
        open(&bob, &sender, sender_id, recipient_id, &sealed, true).as_deref(),
        Some("web-to-rust")
    );
}
