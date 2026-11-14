//! OS keychain integration for secure secret storage.
//!
//! Provides a unified [`Keychain`] trait with platform-specific backends:
//! - Windows: Credential Manager
//! - macOS: Keychain Services
//! - Linux: Secret Service API
//! - Fallback: [`FileKeychain`] — AES-256-GCM encrypted file in the app data
//!   directory, works on all platforms.
//!
//! Used to store E2E encryption keys, OAuth tokens, and other sensitive
//! material so users don't need to re-enter passwords every session. The
//! native backends are backed by the `keyring` crate; see
//! `docs/security/e2e-mfa-design.md` for the design note.

use aead::{Aead, KeyInit};
use anyhow::{bail, Result};
use rand::RngCore;
use std::path::PathBuf;

/// Unified keychain trait for storing and retrieving secrets.
///
/// Implementations must be thread-safe (`Send + Sync`). All operations
/// fail closed: on any error, no secret material is exposed.
pub trait Keychain: Send + Sync {
    /// Store `value` under `key_name`. If an item already exists and
    /// `overwrite` is false, returns an error.
    fn set(&self, key_name: &str, value: &[u8], overwrite: bool) -> Result<()>;
    /// Retrieve the secret value stored under `key_name`.
    fn get(&self, key_name: &str) -> Result<Vec<u8>>;
    /// Delete the secret stored under `key_name`. No error if missing.
    fn delete(&self, key_name: &str) -> Result<()>;
    /// Return `true` if an item exists under `key_name`.
    fn contains(&self, key_name: &str) -> Result<bool>;
}

/// Cross-platform file-based keychain using AES-256-GCM encryption.
///
/// Each entry is stored as a separate file in `dir`:
/// `<dir>/<key_name>.keychain` containing nonce || ciphertext.
///
/// `master_key` is used to encrypt all stored values. It should be
/// derived from a user password or generated randomly and stored
/// in the OS keychain (see [`OsKeychain`]).
pub struct FileKeychain {
    dir: PathBuf,
    master_key: [u8; 32],
}

impl FileKeychain {
    /// Create a new file keychain. `dir` is created if missing.
    pub fn new(dir: PathBuf, master_key: [u8; 32]) -> Result<Self> {
        std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!("create dir: {e}"))?;
        Ok(Self { dir, master_key })
    }

    fn entry_path(&self, key_name: &str) -> PathBuf {
        self.dir.join(format!("{}.keychain", key_name))
    }

    fn encrypt_value(&self, value: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce: hybrid_array::Array<u8, typenum::U12> = nonce_bytes.into();
        let key_arr: hybrid_array::Array<u8, typenum::U32> = self.master_key.into();
        let cipher = aes_gcm::Aes256Gcm::new(&key_arr);
        let ciphertext = cipher
            .encrypt(&nonce, value)
            .map_err(|e| anyhow::anyhow!("encrypt: {e}"))?;
        let mut out = nonce_bytes.to_vec();
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decrypt_value(&self, encrypted: &[u8]) -> Result<Vec<u8>> {
        if encrypted.len() < 12 {
            bail!("ciphertext too short");
        }
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(nonce_bytes);
        let nonce: hybrid_array::Array<u8, typenum::U12> = nonce_arr.into();
        let key_arr: hybrid_array::Array<u8, typenum::U32> = self.master_key.into();
        let cipher = aes_gcm::Aes256Gcm::new(&key_arr);
        cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt: {e}"))
    }
}

impl Keychain for FileKeychain {
    fn set(&self, key_name: &str, value: &[u8], overwrite: bool) -> Result<()> {
        let path = self.entry_path(key_name);
        if path.exists() && !overwrite {
            bail!("keychain item already exists: {key_name}");
        }
        let encrypted = self.encrypt_value(value)?;
        std::fs::write(&path, encrypted).map_err(|e| anyhow::anyhow!("write: {e}"))?;
        Ok(())
    }

    fn get(&self, key_name: &str) -> Result<Vec<u8>> {
        let path = self.entry_path(key_name);
        if !path.exists() {
            bail!("keychain item not found: {key_name}");
        }
        let encrypted = std::fs::read(&path).map_err(|e| anyhow::anyhow!("read: {e}"))?;
        self.decrypt_value(&encrypted)
    }

    fn delete(&self, key_name: &str) -> Result<()> {
        let path = self.entry_path(key_name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| anyhow::anyhow!("delete: {e}"))?;
        }
        Ok(())
    }

    fn contains(&self, key_name: &str) -> Result<bool> {
        Ok(self.entry_path(key_name).exists())
    }
}

/// Credential-store service name used for every Pinpoint entry.
#[allow(dead_code)]
const SERVICE: &str = "pinpoint";

/// OS-native keychain backend.
///
/// On Windows this wraps Credential Manager; on macOS, Keychain Services;
/// on Linux, the Secret Service API. On platforms without a native backend
/// (Android/iOS/WebAssembly) [`OsKeychain::open`] returns an error and
/// callers should fall back to [`FileKeychain`].
pub struct OsKeychain {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    service: String,
}

impl OsKeychain {
    /// Attempt to open the OS-native keychain. Returns an error when the
    /// platform store is unavailable (for example headless Linux without a
    /// Secret Service) so callers can fall back to [`FileKeychain`].
    pub fn open() -> Result<Box<dyn Keychain>> {
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
        {
            // `Entry::new` fails closed with `NoDefaultStore` when the
            // platform credential store could not be initialized.
            keyring::Entry::new(SERVICE, "probe")?;
            Ok(Box::new(Self {
                service: SERVICE.to_string(),
            }))
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            bail!("OS-native keychain not available on this platform; use FileKeychain");
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
impl Keychain for OsKeychain {
    fn set(&self, key_name: &str, value: &[u8], overwrite: bool) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, key_name)
            .map_err(|e| anyhow::anyhow!("keychain open: {e}"))?;
        if !overwrite {
            match entry.get_secret() {
                Ok(_) => bail!("keychain item already exists: {key_name}"),
                Err(keyring::Error::NoEntry) => {}
                Err(e) => return Err(anyhow::anyhow!("keychain lookup: {e}")),
            }
        }
        entry
            .set_secret(value)
            .map_err(|e| anyhow::anyhow!("keychain store: {e}"))?;
        Ok(())
    }

    fn get(&self, key_name: &str) -> Result<Vec<u8>> {
        let entry = keyring::Entry::new(&self.service, key_name)
            .map_err(|e| anyhow::anyhow!("keychain open: {e}"))?;
        entry
            .get_secret()
            .map_err(|e| anyhow::anyhow!("keychain read: {e}"))
    }

    fn delete(&self, key_name: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service, key_name)
            .map_err(|e| anyhow::anyhow!("keychain open: {e}"))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("keychain delete: {e}")),
        }
    }

    fn contains(&self, key_name: &str) -> Result<bool> {
        let entry = keyring::Entry::new(&self.service, key_name)
            .map_err(|e| anyhow::anyhow!("keychain open: {e}"))?;
        match entry.get_secret() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(anyhow::anyhow!("keychain lookup: {e}")),
        }
    }
}

/// Name of the master key entry stored in the keychain.
pub const MASTER_KEY_NAME: &str = "master_key";

/// Open the best available keychain for this platform.
///
/// Prefers the OS-native store ([`OsKeychain`]); when it is unavailable
/// (headless Linux, platforms without a native backend) and `dir` is
/// writable, falls back to [`FileKeychain`]. The fallback master key is
/// generated once and stored in `dir/master.key` with owner-only
/// permissions on Unix. Returns an error when neither is possible.
pub fn open_default(dir: Option<PathBuf>) -> Result<Box<dyn Keychain>> {
    if let Ok(kc) = OsKeychain::open() {
        return Ok(kc);
    }
    let dir = match dir {
        Some(d) => d,
        None => bail!("no OS keychain available and no writable app data dir"),
    };
    let master_key = load_or_create_master_key(&dir.join("master.key"))?;
    Ok(Box::new(FileKeychain::new(dir, master_key)?))
}

/// Load the fallback master key from `path`, generating and persisting a
/// fresh 256-bit key on first use.
fn load_or_create_master_key(path: &std::path::Path) -> Result<[u8; 32]> {
    if let Ok(bytes) = std::fs::read(path) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::write(path, key).map_err(|e| anyhow::anyhow!("write master key: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_keychain_round_trip() {
        let dir = std::env::temp_dir().join(format!("pinpoint-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let master_key = [42u8; 32];
        let kc = FileKeychain::new(dir.clone(), master_key).unwrap();

        // Not found initially.
        assert!(kc.get("nonexistent").is_err());

        // Set and get.
        kc.set("location_key", b"secret-value-123", false).unwrap();
        assert!(kc.contains("location_key").unwrap());
        let val = kc.get("location_key").unwrap();
        assert_eq!(val, b"secret-value-123");

        // Overwrite without flag fails.
        assert!(kc.set("location_key", b"new-value", false).is_err());
        // Overwrite with flag succeeds.
        kc.set("location_key", b"new-value", true).unwrap();
        let val = kc.get("location_key").unwrap();
        assert_eq!(val, b"new-value");

        // Delete.
        kc.delete("location_key").unwrap();
        assert!(!kc.contains("location_key").unwrap());
        assert!(kc.get("location_key").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_keychain_encrypts_at_rest() {
        let dir = std::env::temp_dir().join(format!("pinpoint-test-enc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let master_key = [99u8; 32];
        let kc = FileKeychain::new(dir.clone(), master_key).unwrap();

        kc.set("sensitive", b"top-secret-plaintext", false).unwrap();

        // Raw file should not contain plaintext.
        let path = kc.entry_path("sensitive");
        let raw = std::fs::read(&path).unwrap();
        let needle = b"top-secret-plaintext";
        assert!(!raw.windows(needle.len()).any(|w| w == needle));

        // But decryption recovers it.
        let val = kc.get("sensitive").unwrap();
        assert_eq!(val, b"top-secret-plaintext");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn os_keychain_round_trip() {
        // Only runs on platforms with a native store (Windows/macOS/Linux).
        let kc = match OsKeychain::open() {
            Ok(kc) => kc,
            Err(_) => return,
        };
        let name = format!("pinpoint-test-{}", std::process::id());
        let _ = kc.delete(&name);
        // Headless/sessionless Windows can return ERROR_NO_SUCH_LOGON_SESSION
        // even though `open()` succeeded; skip rather than fail the suite.
        if kc.set(&name, b"os-secret-value", false).is_err() {
            eprintln!("skipping OS keychain round trip: platform storage unavailable");
            return;
        }
        assert!(kc.contains(&name).unwrap());
        let val = kc.get(&name).unwrap();
        assert_eq!(val, b"os-secret-value");
        // Overwrite without flag fails.
        assert!(kc.set(&name, b"new", false).is_err());
        // Overwrite with flag succeeds.
        kc.set(&name, b"new", true).unwrap();
        assert_eq!(kc.get(&name).unwrap(), b"new");
        kc.delete(&name).unwrap();
        assert!(!kc.contains(&name).unwrap());
        assert!(kc.get(&name).is_err());
    }

    #[test]
    fn open_default_returns_a_usable_keychain() {
        let dir = std::env::temp_dir().join(format!("pinpoint-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let name = format!("default-{}", std::process::id());
        let kc = match open_default(Some(dir.clone())) {
            Ok(kc) => kc,
            Err(_) => return,
        };
        let _ = kc.delete(&name);
        if kc.set(&name, b"v", false).is_err() {
            eprintln!("skipping default keychain test: platform storage unavailable");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        assert_eq!(kc.get(&name).unwrap(), b"v");
        kc.delete(&name).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
