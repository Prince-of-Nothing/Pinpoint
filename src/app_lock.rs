//! On-device PIN lock.
//!
//! A short numeric PIN is stored as an Argon2id PHC hash in
//! `<app_data_dir>/lock.json`. Nothing leaves the device and the PIN itself is
//! never written. The UI locks on launch and when the app returns from the
//! background, hiding location data from a borrowed phone.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::security::password::{hash_password, verify_password, DEFAULT_ITERS};

const MIN_PIN: usize = 4;
const MAX_PIN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockRecord {
    pin_hash: String,
    created_at: String,
}

pub struct LockStore {
    path: Option<PathBuf>,
}

impl LockStore {
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            path: dir.map(|d| d.join("lock.json")),
        }
    }

    pub fn default_location() -> Self {
        Self::new(crate::platform::app_data_dir())
    }

    pub fn is_configured(&self) -> bool {
        self.load().is_some()
    }

    fn load(&self) -> Option<LockRecord> {
        let text = std::fs::read_to_string(self.path.as_ref()?).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Store a new PIN hash. Rejects non-numeric or out-of-range PINs.
    pub fn set_pin(&self, pin: &str) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Err("This platform has no writable app data directory.".to_string());
        };
        validate_pin(pin)?;
        let record = LockRecord {
            pin_hash: hash_password(pin, DEFAULT_ITERS),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let json = serde_json::to_string(&record).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        write_private(path, json.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn verify(&self, pin: &str) -> bool {
        let Some(record) = self.load() else {
            return false;
        };
        verify_password(pin, &record.pin_hash)
    }

    pub fn remove(&self) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn validate_pin(pin: &str) -> Result<(), String> {
    if !(MIN_PIN..=MAX_PIN).contains(&pin.chars().count())
        || !pin.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!("Use {MIN_PIN}-{MAX_PIN} digits for the PIN."));
    }
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> (LockStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "pinpoint-lock-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        (LockStore::new(Some(dir.clone())), dir)
    }

    #[test]
    fn pin_round_trips_and_rejects_wrong_pin() {
        let (store, dir) = temp_store("roundtrip");
        assert!(!store.is_configured());
        store.set_pin("4821").unwrap();
        assert!(store.is_configured());
        assert!(store.verify("4821"));
        assert!(!store.verify("0000"));
        let raw = std::fs::read_to_string(dir.join("lock.json")).unwrap();
        assert!(!raw.contains("4821"));
        assert!(raw.contains("$argon2id$"));
        store.remove().unwrap();
        assert!(!store.is_configured());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn weak_or_non_numeric_pins_are_rejected() {
        let (store, dir) = temp_store("validation");
        assert!(store.set_pin("123").is_err());
        assert!(store.set_pin("1234567890123").is_err());
        assert!(store.set_pin("12a4").is_err());
        assert!(!store.is_configured());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn removing_an_absent_lock_is_ok() {
        let (store, dir) = temp_store("absent");
        assert!(store.remove().is_ok());
        std::fs::remove_dir_all(dir).ok();
    }
}
