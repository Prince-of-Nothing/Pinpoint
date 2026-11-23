//! Per-client throttling for the unauthenticated public endpoints (auth and
//! diagnostic reports).
//!
//! The key is an HMAC of the client address with a per-process random key; the
//! raw address is never stored. `X-Forwarded-For` (first hop) or `X-Real-IP`
//! is trusted only because every deployment puts the service behind a proxy
//! that sets or overwrites it; a directly exposed client could spoof it.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Instant;

use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;

/// Fixed window for one auth bucket.
#[derive(Debug, Clone, Copy)]
pub struct AuthLimit {
    pub max: u32,
    pub window_secs: u64,
}

/// Login is the most shared endpoint (households, campuses); account lockout
/// still stops a per-account brute force, so the IP backstop stays generous.
const LOGIN_LIMIT: AuthLimit = AuthLimit {
    max: 30,
    window_secs: 300,
};
const REGISTER_LIMIT: AuthLimit = AuthLimit {
    max: 10,
    window_secs: 3600,
};
const ANONYMOUS_LIMIT: AuthLimit = AuthLimit {
    max: 20,
    window_secs: 600,
};
/// Authenticated writes: high enough for normal use, low enough to blunt a
/// scripted spam loop. Per account, not per IP.
const WRITE_LIMIT: AuthLimit = AuthLimit {
    max: 120,
    window_secs: 60,
};
const DIAGNOSTIC_LIMIT: AuthLimit = AuthLimit {
    max: 10,
    window_secs: 3600,
};
/// Map tiles: 20/s per client. Cached tiles are cheap, but an upstream miss
/// must not become a free scraping proxy.
const TILE_LIMIT: AuthLimit = AuthLimit {
    max: 1200,
    window_secs: 60,
};
/// Search/reverse mirror Nominatim's 1/s courtesy limit; route mirrors OSRM's 2/s.
const MAP_SEARCH_LIMIT: AuthLimit = AuthLimit {
    max: 60,
    window_secs: 60,
};
const MAP_ROUTE_LIMIT: AuthLimit = AuthLimit {
    max: 120,
    window_secs: 60,
};

/// Longest bucket, used to bound the cleanup window.
const MAX_WINDOW_SECS: u64 = 3600;

/// Public endpoints that get a per-client throttle.
pub fn auth_limit(path: &str) -> Option<AuthLimit> {
    match path {
        "/api/v1/auth/login" => Some(LOGIN_LIMIT),
        "/api/v1/auth/register" => Some(REGISTER_LIMIT),
        "/api/v1/auth/anonymous" | "/api/v1/auth/anonymous/recover" => Some(ANONYMOUS_LIMIT),
        "/api/v1/diagnostics/report" => Some(DIAGNOSTIC_LIMIT),
        _ => None,
    }
}

/// Map proxy paths with a per-client throttle. The bucket is stable, so every
/// tile path shares one counter instead of one bucket per tile.
pub fn map_limit(path: &str) -> Option<(&'static str, AuthLimit)> {
    if path.starts_with("/map/tiles/") {
        Some(("tiles", TILE_LIMIT))
    } else if matches!(path, "/map/search" | "/map/reverse") {
        Some(("map-search", MAP_SEARCH_LIMIT))
    } else if path == "/map/route" {
        Some(("map-route", MAP_ROUTE_LIMIT))
    } else {
        None
    }
}

/// First `X-Forwarded-For` hop, or `X-Real-IP`, as a parsed IP address.
pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
        })?;
    raw.parse::<IpAddr>().ok().map(|ip| ip.to_string())
}

/// Sliding counters keyed by a keyed digest of the client address.
#[derive(Debug)]
pub struct AuthRateLimiter {
    key: [u8; 32],
    windows: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthRateLimiter {
    pub fn new() -> Self {
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        Self {
            key,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// Record one attempt and report whether it fits `limit`. The bucket is
    /// the request path, so login and register do not consume each other.
    pub fn allow(&self, bucket: &str, client: &str, limit: AuthLimit) -> bool {
        let key = self.digest(bucket, client);
        let now = Instant::now();
        let mut windows = self.windows.lock().unwrap();
        let stale = MAX_WINDOW_SECS.saturating_mul(2);
        windows.retain(|_, (start, _)| now.duration_since(*start).as_secs() < stale);
        let entry = windows.entry(key).or_insert((now, 0));
        if now.duration_since(entry.0).as_secs() >= limit.window_secs {
            *entry = (now, 0);
        }
        if entry.1 >= limit.max {
            return false;
        }
        entry.1 += 1;
        true
    }

    /// Cap authenticated unsafe requests per account. The HMAC key is
    /// unnecessary for an opaque account id, but it keeps one keying path.
    pub fn allow_write(&self, user_id: i64) -> bool {
        self.allow("write", &format!("user:{user_id}"), WRITE_LIMIT)
    }

    fn digest(&self, bucket: &str, client: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC accepts any key length");
        mac.update(bucket.as_bytes());
        mac.update(b":");
        mac.update(client.as_bytes());
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_counts_per_bucket_and_client() {
        let limiter = AuthRateLimiter::new();
        let limit = AuthLimit {
            max: 2,
            window_secs: 60,
        };
        assert!(limiter.allow("/login", "203.0.113.7", limit));
        assert!(limiter.allow("/login", "203.0.113.7", limit));
        assert!(!limiter.allow("/login", "203.0.113.7", limit));
        assert!(limiter.allow("/register", "203.0.113.7", limit));
        assert!(limiter.allow("/login", "203.0.113.8", limit));
    }

    #[test]
    fn write_cap_is_per_user() {
        let limiter = AuthRateLimiter::new();
        for _ in 0..120 {
            assert!(limiter.allow_write(7));
        }
        assert!(!limiter.allow_write(7));
        assert!(limiter.allow_write(8));
    }

    #[test]
    fn diagnostics_reports_have_a_client_limit() {
        assert!(auth_limit("/api/v1/diagnostics/report").is_some());
        assert!(auth_limit("/api/v1/locations/me").is_none());
    }

    #[test]
    fn map_limits_cover_proxy_paths() {
        assert!(map_limit("/map/tiles/10/1/2").is_some());
        assert!(map_limit("/map/search").is_some());
        assert!(map_limit("/map/route").is_some());
        assert!(map_limit("/map/capabilities").is_none());
    }

    #[test]
    fn client_ip_uses_the_first_forwarded_hop() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers).as_deref(), Some("203.0.113.7"));

        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.9".parse().unwrap());
        assert_eq!(client_ip(&headers).as_deref(), Some("203.0.113.9"));
        assert_eq!(client_ip(&HeaderMap::new()), None);
    }

    #[test]
    fn raw_ip_is_not_stored_in_the_key() {
        let limiter = AuthRateLimiter::new();
        let limit = AuthLimit {
            max: 1,
            window_secs: 60,
        };
        assert!(limiter.allow("/login", "203.0.113.7", limit));
        let windows = limiter.windows.lock().unwrap();
        assert!(windows.keys().all(|key| !key.contains("203.0.113.7")));
    }
}
