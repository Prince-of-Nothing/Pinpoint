//! Security — Encryption at rest.
//!
//! Encrypts sensitive data (bookmarks) before writing to disk so that
//! a stolen device or backup cannot reveal user data without the
//! encryption key.

use base64::engine::{general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rand::Rng;
use std::sync::Mutex;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use hybrid_array::Array as HybridArray;
use typenum::U12;
use zeroize::Zeroize;

/// Error type for encryption operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionError {
    KeyNotFound,
    EncryptionFailed(String),
    DecryptionFailed(String),
    InvalidKey,
}

impl std::fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncryptionError::KeyNotFound => write!(f, "Encryption key not found"),
            EncryptionError::EncryptionFailed(msg) => {
                write!(f, "Encryption failed: {msg}")
            }
            EncryptionError::DecryptionFailed(msg) => {
                write!(f, "Decryption failed: {msg}")
            }
            EncryptionError::InvalidKey => write!(f, "Invalid encryption key"),
        }
    }
}

impl std::error::Error for EncryptionError {}

/// Result type for encryption operations.
pub type EncryptionResult<T> = Result<T, EncryptionError>;

/// An encryption key (256-bit AES key).
#[derive(Clone)]
pub struct EncryptionKey {
    key: [u8; 32],
}

impl EncryptionKey {
    /// Creates a new random encryption key.
    pub fn new() -> Self {
        Self {
            key: rand::thread_rng().gen(),
        }
    }

    /// Creates a key from raw bytes (exactly 32 bytes).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EncryptionError> {
        let key = <[u8; 32]>::try_from(bytes).map_err(|_| EncryptionError::InvalidKey)?;
        Ok(Self { key })
    }

    /// Returns the raw key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

impl Default for EncryptionKey {
    fn default() -> Self {
        Self::new()
    }
}

impl Zeroize for EncryptionKey {
    fn zeroize(&mut self) {
        self.key.zeroize();
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Encrypts and decrypts data using AES-256-GCM.
pub struct AesGcmEncryption {
    key: EncryptionKey,
}

impl std::fmt::Debug for AesGcmEncryption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AesGcmEncryption")
            .field("key", &"[redacted]")
            .finish()
    }
}

impl AesGcmEncryption {
    pub fn new(key: EncryptionKey) -> Self {
        Self { key }
    }

    /// Encrypts a byte slice and returns the ciphertext (nonce || ciphertext).
    pub fn encrypt(&self, plaintext: &[u8]) -> EncryptionResult<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.key.key)
            .map_err(|_| EncryptionError::EncryptionFailed("Invalid key length".to_string()))?;
        let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
        let nonce: HybridArray<u8, U12> = nonce_bytes.into();

        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| EncryptionError::EncryptionFailed("AES-GCM encrypt failed".to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = nonce.as_slice().to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypts ciphertext (nonce || ciphertext) and returns plaintext.
    pub fn decrypt(&self, ciphertext: &[u8]) -> EncryptionResult<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(EncryptionError::DecryptionFailed(
                "Ciphertext too short".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext_bytes) = ciphertext.split_at(12);
        let nonce: HybridArray<u8, U12> = nonce_bytes
            .try_into()
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid nonce length".to_string()))?;
        let cipher = Aes256Gcm::new_from_slice(&self.key.key)
            .map_err(|_| EncryptionError::EncryptionFailed("Invalid key length".to_string()))?;

        cipher
            .decrypt(&nonce, ciphertext_bytes)
            .map_err(|_| EncryptionError::DecryptionFailed("AES-GCM decrypt failed".to_string()))
    }

    /// Encrypts a string and returns base64-encoded ciphertext.
    pub fn encrypt_string(&self, plaintext: &str) -> EncryptionResult<String> {
        let ciphertext = self.encrypt(plaintext.as_bytes())?;
        Ok(BASE64_STANDARD.encode(&ciphertext))
    }

    /// Decrypts base64-encoded ciphertext and returns plaintext string.
    pub fn decrypt_string(&self, ciphertext: &str) -> EncryptionResult<String> {
        let ciphertext_bytes = BASE64_STANDARD
            .decode(ciphertext)
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid base64".to_string()))?;
        let plaintext = self.decrypt(&ciphertext_bytes)?;
        String::from_utf8(plaintext)
            .map_err(|_| EncryptionError::DecryptionFailed("Invalid UTF-8".to_string()))
    }
}

/// Manages encryption keys with in-memory storage and optional persistence
/// via an OS keychain / [`crate::security::keychain::FileKeychain`].
pub struct KeyManager {
    current_key: Mutex<Option<EncryptionKey>>,
    key_store: Mutex<std::collections::HashMap<String, EncryptionKey>>,
    keychain: Option<Box<dyn crate::security::Keychain>>,
}

impl KeyManager {
    pub fn new() -> Self {
        Self {
            current_key: Mutex::new(None),
            key_store: Mutex::new(std::collections::HashMap::new()),
            keychain: None,
        }
    }

    /// Create a `KeyManager` that persists its active key through `keychain`.
    pub fn with_keychain(keychain: Box<dyn crate::security::Keychain>) -> Self {
        Self {
            current_key: Mutex::new(None),
            key_store: Mutex::new(std::collections::HashMap::new()),
            keychain: Some(keychain),
        }
    }

    /// Load the persisted key from the keychain, or generate and persist a
    /// new one when none exists (or the keychain is unavailable). Returns
    /// the active key, if any.
    pub fn load_or_generate(&self) -> Option<EncryptionKey> {
        let kc = self.keychain.as_ref()?;
        const KEY_NAME: &str = crate::security::keychain::MASTER_KEY_NAME;
        if let Ok(bytes) = kc.get(KEY_NAME) {
            if bytes.len() == 32 {
                if let Ok(key) = EncryptionKey::from_bytes(&bytes) {
                    *self.current_key.lock().unwrap() = Some(key.clone());
                    return Some(key);
                }
            }
        }
        let key = self.generate_key();
        let _ = kc.set(KEY_NAME, key.as_bytes(), true);
        Some(key)
    }

    /// Generates a new random key and sets it as current.
    pub fn generate_key(&self) -> EncryptionKey {
        let key = EncryptionKey::new();
        let key_id = key_id_from_bytes(&key.as_bytes()[..]);
        {
            let mut store = self.key_store.lock().unwrap();
            store.insert(key_id.clone(), key.clone());
        }
        {
            let mut current = self.current_key.lock().unwrap();
            *current = Some(key.clone());
        }
        if let Some(kc) = &self.keychain {
            let _ = kc.set(
                crate::security::keychain::MASTER_KEY_NAME,
                key.as_bytes(),
                true,
            );
        }
        key
    }

    /// Sets the current key by ID.
    pub fn set_current_key(&self, key_id: &str) -> Result<(), EncryptionError> {
        let store = self.key_store.lock().unwrap();
        let key = store
            .get(key_id)
            .cloned()
            .ok_or(EncryptionError::KeyNotFound)?;
        {
            let mut current = self.current_key.lock().unwrap();
            *current = Some(key);
        }
        Ok(())
    }

    /// Returns the current encryption key.
    pub fn current_key(&self) -> Option<EncryptionKey> {
        self.current_key.lock().unwrap().clone()
    }

    /// Stores a key with the given ID.
    pub fn store_key(&self, key_id: &str, key: EncryptionKey) {
        self.key_store
            .lock()
            .unwrap()
            .insert(key_id.to_string(), key);
    }

    /// Persist a named secret in the backing OS/file keychain (no-op if no keychain).
    pub fn persist_secret(&self, name: &str, value: &[u8]) {
        if let Some(kc) = &self.keychain {
            let _ = kc.set(name, value, true);
        }
    }

    /// Load a named secret from the backing OS/file keychain, if present.
    pub fn load_secret(&self, name: &str) -> Option<Vec<u8>> {
        self.keychain.as_ref()?.get(name).ok()
    }

    /// Delete a named secret from the backing OS/file keychain.
    pub fn delete_secret(&self, name: &str) {
        if let Some(kc) = &self.keychain {
            let _ = kc.delete(name);
        }
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Derives a key ID from key bytes (first 16 bytes as hex).
fn key_id_from_bytes(bytes: &[u8]) -> String {
    let len = bytes.len().min(16);
    bytes[..len].iter().map(|b| format!("{:02x}", b)).collect()
}

/// Encrypts bookmark data before writing to disk.
pub struct EncryptedBookmarkStore {
    encryption: AesGcmEncryption,
}

impl EncryptedBookmarkStore {
    pub fn new(key: EncryptionKey) -> Self {
        Self {
            encryption: AesGcmEncryption::new(key),
        }
    }

    /// Encrypts bookmark JSON string.
    pub fn encrypt_bookmarks(&self, plaintext: &str) -> EncryptionResult<String> {
        self.encryption.encrypt_string(plaintext)
    }

    /// Decrypts bookmark ciphertext string.
    pub fn decrypt_bookmarks(&self, ciphertext: &str) -> EncryptionResult<String> {
        self.encryption.decrypt_string(ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = EncryptionKey::new();
        let encryption = AesGcmEncryption::new(key.clone());

        let plaintext = "Secret bookmark data";
        let encrypted = encryption.encrypt_string(plaintext).unwrap();
        assert_ne!(encrypted, plaintext);

        let decrypted = encryption.decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_key_fails_decryption() {
        let key1 = EncryptionKey::new();
        let key2 = EncryptionKey::new();
        let encryption1 = AesGcmEncryption::new(key1);
        let encryption2 = AesGcmEncryption::new(key2);

        let encrypted = encryption1.encrypt_string("Secret").unwrap();
        assert!(encryption2.decrypt_string(&encrypted).is_err());
    }

    #[test]
    fn key_manager_generates_and_stores() {
        let km = KeyManager::new();
        let key = km.generate_key();
        let key_id = key_id_from_bytes(&key.as_bytes()[..]);

        assert!(km.set_current_key(&key_id).is_ok());
        assert!(km.current_key().is_some());

        // Wrong key ID should fail
        assert!(km.set_current_key("nonexistent").is_err());
    }

    #[test]
    fn encrypted_bookmark_store() {
        let key = EncryptionKey::new();
        let store = EncryptedBookmarkStore::new(key);

        let bookmarks = r#"[{"id":"1","name":"Home","latitude":41.8902,"longitude":-87.6323}]"#;
        let encrypted = store.encrypt_bookmarks(bookmarks).unwrap();
        let decrypted = store.decrypt_bookmarks(&encrypted).unwrap();
        assert_eq!(decrypted, bookmarks);
    }

    #[test]
    fn validation_integration() {
        use crate::security::validation::*;

        // Valid email
        assert!(validate_email("alice@example.com").is_valid());

        // Invalid email
        assert!(!validate_email("invalid").is_valid());

        // Valid location
        assert!(validate_location(41.8902, -87.6323, Some(8.0)).is_valid());

        // Invalid location
        assert!(!validate_location(200.0, -87.6323, None).is_valid());
    }

    #[test]
    fn key_manager_persists_with_file_keychain() {
        let dir = std::env::temp_dir().join(format!("pinpoint-km-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let kc =
            Box::new(crate::security::keychain::FileKeychain::new(dir.clone(), [7u8; 32]).unwrap());
        let km = KeyManager::with_keychain(kc);
        let key = km
            .load_or_generate()
            .expect("load_or_generate returns a key");
        // A second manager over the same keychain loads the same key.
        let kc2 =
            Box::new(crate::security::keychain::FileKeychain::new(dir.clone(), [7u8; 32]).unwrap());
        let km2 = KeyManager::with_keychain(kc2);
        let key2 = km2
            .load_or_generate()
            .expect("load_or_generate returns a key");
        assert_eq!(key.as_bytes(), key2.as_bytes());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn key_manager_without_keychain_generates_in_memory() {
        let km = KeyManager::new();
        assert!(km.current_key().is_none());
        let key = km.generate_key();
        assert!(km.current_key().is_some());
        assert_eq!(km.current_key().unwrap().as_bytes(), key.as_bytes());
    }
}
