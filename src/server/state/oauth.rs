//! OAuth sign-in state: the configured clients and the single-use pending
//! authorizations (state -> PKCE verifier).
//!
//! Pending states live in memory with a ten-minute expiry. A multi-replica
//! deployment needs a shared store for them; the state cookie also binds the
//! authorization to the browser, so a state from another replica simply fails
//! closed instead of being accepted.

use std::time::{Duration, Instant};

use crate::security::oauth::{authorize_url, generate_state, generate_verifier, OAuthClient};

use super::AppState;

/// How long a started authorization may wait for the provider callback.
pub const OAUTH_STATE_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct PendingOAuth {
    pub provider: String,
    pub verifier: String,
    pub started: Instant,
}

impl AppState {
    /// Register (or replace) a provider client. Startup reads the environment;
    /// tests inject a client that points at a local mock provider.
    pub fn set_oauth_client(&self, client: OAuthClient) {
        self.oauth_clients
            .lock()
            .unwrap()
            .insert(client.provider.name.to_string(), client);
    }

    /// Enabled providers, sorted so the client renders a stable order.
    pub fn oauth_providers(&self) -> Vec<String> {
        let mut names: Vec<String> = self.oauth_clients.lock().unwrap().keys().cloned().collect();
        names.sort();
        names
    }

    pub fn oauth_client(&self, provider: &str) -> Option<OAuthClient> {
        self.oauth_clients.lock().unwrap().get(provider).cloned()
    }

    /// Start an authorization: returns the opaque state and the provider URL.
    /// Unknown providers return `None` so the route can answer 404.
    pub fn begin_oauth(&self, provider: &str) -> Option<(String, String)> {
        let client = self.oauth_client(provider)?;
        let state = generate_state();
        let verifier = generate_verifier();
        let url = authorize_url(&client.provider, &client.redirect_uri, &state, &verifier);
        let mut pending = self.oauth_pending.lock().unwrap();
        pending.retain(|_, entry| entry.started.elapsed() < OAUTH_STATE_TTL);
        pending.insert(
            state.clone(),
            PendingOAuth {
                provider: provider.to_string(),
                verifier,
                started: Instant::now(),
            },
        );
        Some((state, url))
    }

    /// Consume a state exactly once. A provider mismatch, an expired entry or a
    /// replayed state all return `None`.
    pub fn take_oauth_state(&self, state: &str, provider: &str) -> Option<String> {
        let mut pending = self.oauth_pending.lock().unwrap();
        let entry = pending.remove(state)?;
        if entry.provider != provider || entry.started.elapsed() >= OAUTH_STATE_TTL {
            return None;
        }
        Some(entry.verifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::oauth::OAuthProvider;

    fn client() -> OAuthClient {
        OAuthClient {
            provider: OAuthProvider::google("client-id".into()),
            client_secret: "secret".into(),
            userinfo_url: "https://example.invalid/userinfo".into(),
            email_url: None,
            redirect_uri: "https://app.example/api/v1/auth/oauth/google/callback".into(),
        }
    }

    #[test]
    fn start_and_consume_state_once() {
        let state = AppState::new();
        state.set_oauth_client(client());
        let (token, url) = state.begin_oauth("google").expect("provider enabled");
        assert!(url.contains("code_challenge="));
        assert!(url.contains("state="));
        let verifier = state.take_oauth_state(&token, "google").expect("verifier");
        assert!(verifier.len() >= 43);
        assert!(state.take_oauth_state(&token, "google").is_none());
    }

    #[test]
    fn wrong_provider_and_unknown_provider_fail_closed() {
        let state = AppState::new();
        state.set_oauth_client(client());
        let (token, _) = state.begin_oauth("google").unwrap();
        assert!(state.take_oauth_state(&token, "github").is_none());
        assert!(state.begin_oauth("github").is_none());
    }

    #[test]
    fn providers_are_listed_only_when_configured() {
        let state = AppState::new();
        assert!(!state.oauth_providers().contains(&"google".to_string()));
        state.set_oauth_client(client());
        assert_eq!(state.oauth_providers(), vec!["google".to_string()]);
    }
}
