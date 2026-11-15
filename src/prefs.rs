//! On-device UI preferences that are not tied to an account.
//!
//! Stored as `<app_data_dir>/prefs.json`. Nothing here leaves the device; it
//! exists so choices such as the launch-time update check survive a restart.
//! A missing or corrupt file falls back to the defaults (update check on).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How long to wait between automatic launch-time update checks.
pub const UPDATE_CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPrefs {
    /// Contact GitHub's public releases feed at most once a day at launch.
    /// Users can turn this off in Profile & settings.
    #[serde(default = "default_true")]
    pub update_check_on_launch: bool,
    /// Release tag the user dismissed from the in-app banner. Empty = none.
    #[serde(default)]
    pub dismissed_update_tag: String,
    /// Unix seconds of the last update check (launch or manual).
    #[serde(default)]
    pub last_update_check_unix: u64,
    /// Offer the platform biometric prompt on the app lock screen. Off by
    /// default; the PIN always remains the fallback.
    #[serde(default)]
    pub biometric_unlock: bool,
    /// Foreground-service update interval in seconds (15-3600).
    #[serde(default = "default_background_interval")]
    pub background_interval_secs: u64,
}

fn default_background_interval() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

impl Default for LocalPrefs {
    fn default() -> Self {
        Self {
            update_check_on_launch: true,
            dismissed_update_tag: String::new(),
            last_update_check_unix: 0,
            biometric_unlock: false,
            background_interval_secs: 60,
        }
    }
}

pub struct PrefsStore {
    path: Option<PathBuf>,
}

impl PrefsStore {
    /// `None` on targets without a writable app dir; the store then keeps the
    /// defaults every launch.
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self {
            path: dir.map(|d| d.join("prefs.json")),
        }
    }

    /// Default store under [`crate::platform::app_data_dir`].
    pub fn default_location() -> Self {
        Self::new(crate::platform::app_data_dir())
    }

    /// Read preferences, falling back to defaults when the file is missing or
    /// unreadable. A corrupt file must not disable the check silently.
    pub fn load(&self) -> LocalPrefs {
        let Some(path) = self.path.as_ref() else {
            return LocalPrefs::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return LocalPrefs::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    /// Persist preferences atomically (temp file then rename).
    pub fn save(&self, prefs: &LocalPrefs) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Err("This platform has no writable app data directory.".to_string());
        };
        let json = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
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
}

impl Default for PrefsStore {
    fn default() -> Self {
        Self::default_location()
    }
}

/// Current Unix time in seconds; 0 when the clock is before the epoch.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether a launch-time check is due. `last == 0` means never checked.
pub fn update_check_due(last_check_unix: u64, now_unix: u64) -> bool {
    last_check_unix == 0
        || now_unix < last_check_unix
        || now_unix - last_check_unix >= UPDATE_CHECK_INTERVAL_SECS
}

#[cfg(test)]
mod tests {
    use super::{update_check_due, LocalPrefs, PrefsStore, UPDATE_CHECK_INTERVAL_SECS};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "pinpoint-prefs-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn defaults_enable_launch_check() {
        let prefs = LocalPrefs::default();
        assert!(prefs.update_check_on_launch);
        assert!(prefs.dismissed_update_tag.is_empty());
    }

    #[test]
    fn round_trips_and_persists_opt_out() {
        let dir = temp_dir("round-trip");
        let store = PrefsStore::new(Some(dir.clone()));
        assert!(store.load().update_check_on_launch);

        let prefs = LocalPrefs {
            update_check_on_launch: false,
            dismissed_update_tag: "v1.3.13".to_string(),
            last_update_check_unix: 1_700_000_000,
            biometric_unlock: true,
            background_interval_secs: 120,
        };
        store.save(&prefs).expect("save");
        assert_eq!(store.load(), prefs);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn biometric_unlock_defaults_off() {
        assert!(!LocalPrefs::default().biometric_unlock);
    }

    #[test]
    fn corrupt_or_missing_file_falls_back_to_defaults() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("prefs.json"), "{not json").unwrap();
        let store = PrefsStore::new(Some(dir.clone()));
        assert_eq!(store.load(), LocalPrefs::default());
        std::fs::remove_dir_all(dir).ok();

        let missing = PrefsStore::new(Some(temp_dir("missing")));
        assert_eq!(missing.load(), LocalPrefs::default());
    }

    #[test]
    fn unwritable_store_reports_an_error() {
        let store = PrefsStore::new(None);
        assert!(store.save(&LocalPrefs::default()).is_err());
    }

    #[test]
    fn launch_check_is_throttled() {
        assert!(update_check_due(0, 1_000));
        assert!(!update_check_due(1_000, 1_000));
        assert!(!update_check_due(
            1_000,
            1_000 + UPDATE_CHECK_INTERVAL_SECS - 1
        ));
        assert!(update_check_due(1_000, 1_000 + UPDATE_CHECK_INTERVAL_SECS));
        // A clock that moved backwards must not block the check forever.
        assert!(update_check_due(2_000, 1_000));
    }
}
