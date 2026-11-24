//! OAuth2 (authorization code + PKCE) helpers.
//!
//! Secrets never ship in the binary: client IDs come from env, client
//! secrets (if any) are exchanged server-side only. PKCE (S256) is
//! mandatory for public clients. State is opaque + CSRF-bound.

use base64::Engine;
use sha2::{Digest, Sha256};

/// A configured OAuth2 provider.
#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: &'static str,
    pub authorize_url: String,
    pub token_url: String,
    pub scopes: &'static [&'static str],
    pub client_id: String,
}

impl OAuthProvider {
    pub fn google(client_id: String) -> Self {
        Self {
            name: "google",
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: &["openid", "email", "profile"],
            client_id,
        }
    }
    pub fn github(client_id: String) -> Self {
        Self {
            name: "github",
            authorize_url: "https://github.com/login/oauth/authorize".into(),
            token_url: "https://github.com/login/oauth/access_token".into(),
            scopes: &["read:user", "user:email"],
            client_id,
        }
    }
}

/// The remote endpoints for a provider. Kept separately from the client
/// credentials so a request URL never shares a container with a secret.
#[derive(Debug, Clone)]
pub struct OAuthEndpoints {
    pub userinfo_url: String,
    pub email_url: Option<String>,
}

/// A configured client: provider metadata, the userinfo endpoint and the
/// redirect URI registered with the provider. The client secret is kept in
/// `AppState` separately so endpoint URLs never travel with it.
#[derive(Debug, Clone)]
pub struct OAuthClient {
    pub provider: OAuthProvider,
    pub userinfo_url: String,
    /// GitHub exposes verified addresses on a second endpoint; `None` for OIDC
    /// providers that return `email` and `email_verified` in the userinfo.
    pub email_url: Option<String>,
    pub redirect_uri: String,
}

impl OAuthClient {
    pub fn endpoints(&self) -> OAuthEndpoints {
        OAuthEndpoints {
            userinfo_url: self.userinfo_url.clone(),
            email_url: self.email_url.clone(),
        }
    }

    /// Google OIDC client from env. Returns `None` when the client ID or the
    /// secret is missing, so the routes stay disabled on a stock deployment.
    pub fn google_from_env() -> Option<(Self, String)> {
        let client_id = std::env::var("PINPOINT_OAUTH_GOOGLE_CLIENT_ID")
            .ok()?
            .trim()
            .to_string();
        let client_secret = std::env::var("PINPOINT_OAUTH_GOOGLE_CLIENT_SECRET")
            .ok()?
            .trim()
            .to_string();
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        let public = std::env::var("PINPOINT_PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let public = public.trim_end_matches('/');
        let mut provider = OAuthProvider::google(client_id);
        if let Ok(url) = std::env::var("PINPOINT_OAUTH_GOOGLE_AUTHORIZE_URL") {
            provider.authorize_url = url;
        }
        if let Ok(url) = std::env::var("PINPOINT_OAUTH_GOOGLE_TOKEN_URL") {
            provider.token_url = url;
        }
        let userinfo_url = std::env::var("PINPOINT_OAUTH_GOOGLE_USERINFO_URL")
            .unwrap_or_else(|_| "https://openidconnect.googleapis.com/v1/userinfo".to_string());
        let client = Self {
            provider,
            userinfo_url,
            email_url: None,
            redirect_uri: format!("{public}/api/v1/auth/oauth/google/callback"),
        };
        assert_secure_endpoints(&client);
        Some((client, client_secret))
    }

    /// GitHub OAuth client from env. GitHub does not put a verified email in
    /// `/user`, so the callback reads `/user/emails` and takes the primary
    /// verified address.
    pub fn github_from_env() -> Option<(Self, String)> {
        let client_id = std::env::var("PINPOINT_OAUTH_GITHUB_CLIENT_ID")
            .ok()?
            .trim()
            .to_string();
        let client_secret = std::env::var("PINPOINT_OAUTH_GITHUB_CLIENT_SECRET")
            .ok()?
            .trim()
            .to_string();
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        let public = std::env::var("PINPOINT_PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        let public = public.trim_end_matches('/');
        let mut provider = OAuthProvider::github(client_id);
        if let Ok(url) = std::env::var("PINPOINT_OAUTH_GITHUB_AUTHORIZE_URL") {
            provider.authorize_url = url;
        }
        if let Ok(url) = std::env::var("PINPOINT_OAUTH_GITHUB_TOKEN_URL") {
            provider.token_url = url;
        }
        let userinfo_url = std::env::var("PINPOINT_OAUTH_GITHUB_USERINFO_URL")
            .unwrap_or_else(|_| "https://api.github.com/user".to_string());
        let email_url = std::env::var("PINPOINT_OAUTH_GITHUB_EMAILS_URL")
            .unwrap_or_else(|_| "https://api.github.com/user/emails".to_string());
        let client = Self {
            provider,
            userinfo_url,
            email_url: Some(email_url),
            redirect_uri: format!("{public}/api/v1/auth/oauth/github/callback"),
        };
        assert_secure_endpoints(&client);
        Some((client, client_secret))
    }
}

/// Provider endpoints must be HTTPS. Loopback HTTP is allowed for local
/// development and the mock providers used by the tests, mirroring the
/// transport policy in ADR-0006.
pub fn is_secure_provider_url(raw: &str) -> bool {
    if let Some(rest) = raw.strip_prefix("https://") {
        return !rest.is_empty();
    }
    let Some(rest) = raw.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let hostname = host.split(':').next().unwrap_or("");
    matches!(hostname, "127.0.0.1" | "localhost" | "[::1]")
}

/// Fail fast on a misconfigured provider endpoint rather than sending the
/// client secret or a bearer token over cleartext.
fn assert_secure_endpoints(client: &OAuthClient) {
    let urls = [
        &client.provider.authorize_url,
        &client.provider.token_url,
        &client.userinfo_url,
    ];
    for url in urls {
        assert!(
            is_secure_provider_url(url),
            "OAuth provider endpoints must use HTTPS (loopback HTTP is allowed for tests): {url}"
        );
    }
    if let Some(email_url) = &client.email_url {
        assert!(
            is_secure_provider_url(email_url),
            "OAuth email endpoint must use HTTPS (loopback HTTP is allowed for tests): {email_url}"
        );
    }
}

/// Generate a PKCE code verifier (43-128 chars, URL-safe).
pub fn generate_verifier() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// S256 code challenge: BASE64URL(SHA256(verifier)).
pub fn challenge_s256(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// Validate a redirect URI against an allowlist (exact match, no open redirect).
pub fn validate_redirect_uri(uri: &str, allowlist: &[&str]) -> bool {
    !uri.is_empty() && allowlist.contains(&uri)
}

/// Build the authorization URL.
pub fn authorize_url(
    provider: &OAuthProvider,
    redirect_uri: &str,
    state: &str,
    verifier: &str,
) -> String {
    let challenge = challenge_s256(verifier);
    let scope = provider.scopes.join(" ");
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        provider.authorize_url,
        urlenc(&provider.client_id),
        urlenc(redirect_uri),
        urlenc(&scope),
        urlenc(state),
        urlenc(&challenge),
    )
}

fn urlenc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Opaque state token (CSRF-bound).
pub fn generate_state() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_s256() {
        let v = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let c = challenge_s256(v);
        // RFC 7636 appendix B expected value.
        assert_eq!(c, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn verifier_is_url_safe() {
        let v = generate_verifier();
        assert!(v.len() >= 43);
        assert!(!v.contains('+') && !v.contains('/'));
    }

    #[test]
    fn state_is_unique() {
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn redirect_uri_allowlist_is_exact() {
        let allow = ["https://app.example.com/cb"];
        assert!(validate_redirect_uri("https://app.example.com/cb", &allow));
        assert!(!validate_redirect_uri(
            "https://evil.example.com/cb",
            &allow
        ));
        assert!(!validate_redirect_uri("", &allow));
        // Open-redirect attempt via suffix.
        assert!(!validate_redirect_uri(
            "https://app.example.com/cb.evil",
            &allow
        ));
    }

    #[test]
    fn authorize_url_contains_pkce_and_state() {
        let p = OAuthProvider::google("client123".into());
        let u = authorize_url(&p, "https://app/cb", "STATE", "VERIFIER");
        assert!(u.contains("code_challenge="));
        assert!(u.contains("code_challenge_method=S256"));
        assert!(u.contains("state=STATE"));
        assert!(u.contains("client_id=client123"));
    }

    #[test]
    fn provider_scopes_non_empty() {
        assert!(!OAuthProvider::google("x".into()).scopes.is_empty());
        assert!(!OAuthProvider::github("x".into()).scopes.is_empty());
    }

    #[test]
    fn provider_urls_must_be_https_or_loopback() {
        assert!(is_secure_provider_url(
            "https://accounts.google.com/o/oauth2/v2/auth"
        ));
        assert!(is_secure_provider_url("http://127.0.0.1:9099/token"));
        assert!(is_secure_provider_url("http://localhost:9099/token"));
        assert!(is_secure_provider_url("http://user@127.0.0.1:9099/token"));
        assert!(!is_secure_provider_url("http://example.com/token"));
        assert!(!is_secure_provider_url("http://127.0.0.1.evil.com/token"));
        assert!(!is_secure_provider_url("ftp://example.com/token"));
        assert!(!is_secure_provider_url("https://"));
    }
}
