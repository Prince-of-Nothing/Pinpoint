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
    /// Consecutive wrong attempts; persisted so force-stop does not reset it.
    #[serde(default)]
    failures: u32,
    /// Unix epoch milliseconds before which no attempt is accepted.
    #[serde(default)]
    locked_until_ms: i64,
}

/// Outcome of a PIN attempt, including the persisted lockout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockAttempt {
    Unlocked,
    Wrong { failures: u32, retry_after_ms: u64 },
    LockedOut { retry_after_ms: u64 },
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
        // A present-but-unreadable file must fail closed (locked), not open.
        self.path.as_ref().is_some_and(|path| path.exists())
    }

    fn load(&self) -> Option<LockRecord> {
        let text = std::fs::read_to_string(self.path.as_ref()?).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Store a new PIN hash. Rejects non-numeric or out-of-range PINs and
    /// clears any persisted failure state.
    pub fn set_pin(&self, pin: &str) -> Result<(), String> {
        validate_pin(pin)?;
        let record = LockRecord {
            pin_hash: hash_password(pin, DEFAULT_ITERS),
            created_at: chrono::Utc::now().to_rfc3339(),
            failures: 0,
            locked_until_ms: 0,
        };
        self.save(&record)
    }

    fn save(&self, record: &LockRecord) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Err("This platform has no writable app data directory.".to_string());
        };
        let json = serde_json::to_string(record).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        write_private(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            // std::fs::rename does not replace an existing file on Windows.
            let _ = std::fs::remove_file(path);
        }
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn verify(&self, pin: &str) -> bool {
        let Some(record) = self.load() else {
            return false;
        };
        verify_password(pin, &record.pin_hash)
    }

    /// Check a PIN and update the persisted failure/lockout state.
    pub fn attempt(&self, pin: &str) -> LockAttempt {
        self.attempt_at(pin, now_ms())
    }

    fn attempt_at(&self, pin: &str, now_ms: i64) -> LockAttempt {
        let Some(mut record) = self.load() else {
            // Present-but-corrupt state stays locked without overwriting it.
            return LockAttempt::Wrong {
                failures: 1,
                retry_after_ms: 1_000,
            };
        };
        if record.locked_until_ms > now_ms {
            return LockAttempt::LockedOut {
                retry_after_ms: (record.locked_until_ms - now_ms) as u64,
            };
        }
        if verify_password(pin, &record.pin_hash) {
            if record.failures != 0 || record.locked_until_ms != 0 {
                record.failures = 0;
                record.locked_until_ms = 0;
                if let Err(error) = self.save(&record) {
                    tracing::warn!(%error, "app lock: could not clear failure state");
                }
            }
            return LockAttempt::Unlocked;
        }
        record.failures = record.failures.saturating_add(1);
        let delay_ms = 1_000u64 << record.failures.min(5);
        record.locked_until_ms = now_ms.saturating_add(delay_ms as i64);
        if let Err(error) = self.save(&record) {
            tracing::warn!(%error, "app lock: could not persist failure state");
        }
        LockAttempt::Wrong {
            failures: record.failures,
            retry_after_ms: delay_ms,
        }
    }

    /// Remaining persisted lockout, if any (used at startup).
    pub fn lockout_remaining(&self) -> Option<std::time::Duration> {
        let now = now_ms();
        let record = self.load()?;
        (record.locked_until_ms > now)
            .then(|| std::time::Duration::from_millis((record.locked_until_ms - now) as u64))
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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
    file.write_all(bytes)?;
    file.sync_all()
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

    #[test]
    fn corrupt_lock_file_fails_closed() {
        let (store, dir) = temp_store("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lock.json"), b"{not json").unwrap();
        assert!(store.is_configured(), "damaged lock must stay locked");
        assert!(!store.verify("1234"));
        assert!(!store.verify(""));
        assert!(matches!(store.attempt("1234"), LockAttempt::Wrong { .. }));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn lockout_survives_reload_and_expires() {
        let (store, dir) = temp_store("lockout");
        store.set_pin("1234").unwrap();
        let t0 = 1_700_000_000_000;
        assert_eq!(
            store.attempt_at("0000", t0),
            LockAttempt::Wrong {
                failures: 1,
                retry_after_ms: 2_000
            }
        );
        // Simulate a force-stop: a fresh store reads the persisted lockout.
        let reloaded = LockStore::new(Some(dir.clone()));
        assert_eq!(
            reloaded.attempt_at("1234", t0 + 1_000),
            LockAttempt::LockedOut {
                retry_after_ms: 1_000
            }
        );
        // After the lockout the correct PIN succeeds and clears the state.
        assert_eq!(
            reloaded.attempt_at("1234", t0 + 2_001),
            LockAttempt::Unlocked
        );
        let raw = std::fs::read_to_string(dir.join("lock.json")).unwrap();
        let record: LockRecord = serde_json::from_str(&raw).unwrap();
        assert_eq!(record.failures, 0);
        assert_eq!(record.locked_until_ms, 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn failure_count_keeps_growing_across_reloads() {
        let (store, dir) = temp_store("failures");
        store.set_pin("1234").unwrap();
        let t0 = 1_700_000_000_000;
        assert_eq!(
            store.attempt_at("0000", t0),
            LockAttempt::Wrong {
                failures: 1,
                retry_after_ms: 2_000
            }
        );
        let reloaded = LockStore::new(Some(dir.clone()));
        assert_eq!(
            reloaded.attempt_at("0000", t0 + 3_000),
            LockAttempt::Wrong {
                failures: 2,
                retry_after_ms: 4_000
            }
        );
        // A correct PIN after the second lockout resets the counter.
        assert_eq!(
            reloaded.attempt_at("1234", t0 + 7_001),
            LockAttempt::Unlocked
        );
        assert_eq!(
            reloaded.attempt_at("0000", t0 + 7_002),
            LockAttempt::Wrong {
                failures: 1,
                retry_after_ms: 2_000
            }
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn lockout_remaining_reads_the_persisted_deadline() {
        let (store, dir) = temp_store("remaining");
        let now = now_ms();
        let record = LockRecord {
            pin_hash: hash_password("1234", DEFAULT_ITERS),
            created_at: chrono::Utc::now().to_rfc3339(),
            failures: 3,
            locked_until_ms: now + 5_000,
        };
        store.save(&record).unwrap();
        let remaining = store.lockout_remaining().expect("lockout should remain");
        assert!(remaining.as_millis() > 3_000 && remaining.as_millis() <= 5_000);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn changing_the_pin_replaces_the_old_hash() {
        let (store, dir) = temp_store("change");
        store.set_pin("1234").unwrap();
        store.set_pin("5678").unwrap();
        assert!(store.verify("5678"));
        assert!(!store.verify("1234"));
        std::fs::remove_dir_all(dir).ok();
    }
}
