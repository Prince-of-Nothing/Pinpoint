//! Security module — Comprehensive security measures for Pinpoint.
//!
//! Provides:
//! - Input validation (validation.rs)
//! - Client-side rate limiting (rate_limit.rs)
//! - Encryption at rest (encryption.rs)
//! - Audit logging (audit.rs)
//! - Session management with timeout (session.rs)
//! - Request signing (signing.rs)

pub mod audit;
pub mod circuit_breaker;
pub mod encryption;
pub mod keychain;
pub mod message_crypto;
pub mod mfa;
pub mod oauth;
pub mod password;
pub mod qrcode;
pub mod rate_limit;
pub mod scrape;
pub mod session;
pub mod signing;
pub mod validation;

/// Re-export commonly used types for convenience.
pub use audit::{AuditEvent, AuditLogger};
pub use encryption::{AesGcmEncryption, EncryptionKey, KeyManager};
pub use keychain::{open_default, FileKeychain, Keychain, OsKeychain, MASTER_KEY_NAME};
pub use mfa::{generate_secret, otpauth_uri, totp, verify_totp};
pub use password::{hash_password, verify_password};
pub use rate_limit::{RateLimitResult, RateLimiter};
pub use session::{SessionConfig, SessionManager, SessionState};
pub use signing::{RequestSignature, RequestSigner, SigningKey};
pub use validation::{
    validate_email, validate_location, validate_name, validate_password, validate_search_query,
};
