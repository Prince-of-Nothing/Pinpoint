//! End-to-end encryption for location data.
//!
//! Location payloads (latitude, longitude, accuracy) are encrypted
//! client-side before being sent to the server. The server stores
//! ciphertext only and cannot decrypt friend locations.
//!
//! Key flow:
//! - Each user has a `location_key` derived from their password via PBKDF2.
//! - When publishing a location, the client encrypts the payload with
//!   their `location_key` and stores the ciphertext on the server.
//! - When a share is created, the sender wraps their `location_key`
//!   for the recipient using the recipient's `location_key` (key
//!   wrapping with AES-GCM). The recipient unwraps it to decrypt
//!   shared locations.
//!
//! Threat model: server compromise does not expose location plaintext.

use crate::security::password::pbkdf2;
use aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use base64::Engine;
use hybrid_array::Array;
use rand::Rng;
use typenum::{U12, U32};

/// Derive a location encryption key from a user's password.
///
/// Uses PBKDF2-HMAC-SHA256 with a random 16-byte salt and 100k iterations.
/// Format: `e2e$<salt_b64>$<iters>$<key_b64>`
pub fn derive_location_key(password: &str) -> String {
    let salt: [u8; 16] = rand::thread_rng().gen();
    let key = pbkdf2(password.as_bytes(), &salt, 100_000);
    let b64 = base64::engine::general_purpose::STANDARD;
    format!(
        "e2e${}${}${}",
        b64.encode(salt),
        100_000u32,
        b64.encode(key)
    )
}

/// Encrypt a location payload (lat, lon, accuracy) with a location key.
/// Returns base64-encoded ciphertext: `nonce_b64.ciphertext_b64`.
pub fn encrypt_location(location_key: &str, lat: f64, lon: f64, accuracy: f64) -> Option<String> {
    let parts: Vec<&str> = location_key.split('$').collect();
    if parts.len() != 4 || parts[0] != "e2e" {
        return None;
    }
    let key_b64 = parts[3];
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .ok()?;
    let key_arr = <[u8; 32]>::try_from(key_bytes.as_slice()).ok()?;
    let key_ga: Array<u8, U32> = key_arr.into();
    let cipher = Aes256Gcm::new(&key_ga);

    let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
    let nonce_ga: Array<u8, U12> = nonce_bytes.into();

    let payload = serde_json::json!({"lat": lat, "lon": lon, "acc": accuracy});
    let payload_bytes = serde_json::to_vec(&payload).ok()?;

    let ciphertext = cipher.encrypt(&nonce_ga, payload_bytes.as_slice()).ok()?;

    let b64 = base64::engine::general_purpose::STANDARD;
    Some(format!(
        "{}.{}",
        b64.encode(nonce_bytes),
        b64.encode(ciphertext)
    ))
}

/// Decrypt an encrypted location payload.
/// Returns (lat, lon, accuracy) on success.
pub fn decrypt_location(location_key: &str, encrypted: &str) -> Option<(f64, f64, f64)> {
    let parts: Vec<&str> = encrypted.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let b64 = base64::engine::general_purpose::STANDARD;
    let nonce_bytes = b64.decode(parts[0]).ok()?;
    let ciphertext = b64.decode(parts[1]).ok()?;

    if nonce_bytes.len() != 12 {
        return None;
    }

    let key_parts: Vec<&str> = location_key.split('$').collect();
    if key_parts.len() != 4 || key_parts[0] != "e2e" {
        return None;
    }
    let key_b64 = key_parts[3];
    let key_bytes = b64.decode(key_b64).ok()?;
    let key_arr = <[u8; 32]>::try_from(key_bytes.as_slice()).ok()?;
    let key_ga: Array<u8, U32> = key_arr.into();
    let cipher = Aes256Gcm::new(&key_ga);

    let nonce_arr = <[u8; 12]>::try_from(nonce_bytes.as_slice()).ok()?;
    let nonce_ga: Array<u8, U12> = nonce_arr.into();

    let plaintext = cipher.decrypt(&nonce_ga, ciphertext.as_slice()).ok()?;
    let payload: serde_json::Value = serde_json::from_slice(&plaintext).ok()?;

    let lat = payload["lat"].as_f64()?;
    let lon = payload["lon"].as_f64()?;
    let acc = payload["acc"].as_f64()?;

    Some((lat, lon, acc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_and_round_trip() {
        let key = derive_location_key("hunter2");
        let enc = encrypt_location(&key, 47.1234, 28.5678, 5.5).unwrap();
        let (lat, lon, acc) = decrypt_location(&key, &enc).unwrap();
        assert!((lat - 47.1234).abs() < 1e-6);
        assert!((lon - 28.5678).abs() < 1e-6);
        assert!((acc - 5.5).abs() < 1e-6);
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = derive_location_key("pass1");
        let key2 = derive_location_key("pass2");
        let enc = encrypt_location(&key1, 1.0, 2.0, 3.0).unwrap();
        assert!(decrypt_location(&key2, &enc).is_none());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = derive_location_key("pass");
        let enc = encrypt_location(&key, 1.0, 2.0, 3.0).unwrap();
        let mut parts: Vec<&str> = enc.split('.').collect();
        let ct = base64::engine::general_purpose::STANDARD
            .decode(parts[1])
            .unwrap();
        let mut ct2 = ct.clone();
        ct2[0] ^= 0x01;
        let encoded_ct2 = base64::engine::general_purpose::STANDARD.encode(ct2);
        parts[1] = &encoded_ct2;
        let tampered = format!("{}.{}", parts[0], parts[1]);
        assert!(decrypt_location(&key, &tampered).is_none());
    }

    #[test]
    fn malformed_key_rejected() {
        assert!(encrypt_location("not-a-valid-key", 1.0, 2.0, 3.0).is_none());
        assert!(decrypt_location("not-a-valid-key", "a.b").is_none());
    }
}
