"""
Low-level security primitives: password hashing, token/session generation,
email normalisation, and PII pseudonymisation helpers.

Higher-level flows (login, password change) live in AuthService; token
issuance/validation lives in TokenService. This module stays a leaf so it can
be reused without dragging in business logic (Single Responsibility).
"""

import hashlib
import hmac
import secrets
from functools import lru_cache

from passlib.context import CryptContext

from app.core.config import get_settings
from app.services.password_policy import DEFAULT_PASSWORD_POLICY

settings = get_settings()

# Password hashing context with bcrypt (slow hash + per-password salt).
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# bcrypt silently truncates inputs at 72 bytes; reject longer material early
# (also enforced by the password policy's MaxLengthRule).
MAX_PASSWORD_BYTES = 72


def verify_password(plain_password: str, hashed_password: str) -> bool:
    """
    Verify a plain password against a hash.
    Uses constant-time comparison internally to prevent timing attacks.
    """
    try:
        return pwd_context.verify(plain_password[:MAX_PASSWORD_BYTES], hashed_password)
    except (ValueError, TypeError):
        # Malformed hash in the DB must fail closed, not crash the request.
        return False


def get_password_hash(password: str) -> str:
    """
    Generate a secure password hash.
    Each call produces a different hash due to a random salt.
    """
    return pwd_context.hash(password[:MAX_PASSWORD_BYTES])


@lru_cache
def get_dummy_password_hash() -> str:
    """
    Null Object for anti-enumeration (timing equalisation).

    When a login references an unknown or inactive account, the verifier still
    performs one bcrypt comparison against this throwaway hash so the response
    timing does not reveal whether the account exists.
    """
    return pwd_context.hash(secrets.token_urlsafe(32))


def generate_session_token() -> str:
    """Generate a cryptographically secure session token (256 bits)."""
    return secrets.token_urlsafe(32)


def hash_token(raw_token: str) -> str:
    """
    One-way hash for tokens stored at rest (sessions).

    A plain SHA-256 is correct here: session tokens carry 256 bits of entropy,
    so they are not vulnerable to dictionary attacks (unlike passwords).
    """
    return hashlib.sha256(raw_token.encode("utf-8")).hexdigest()


def pseudonymize_email(email: str) -> str:
    """
    Keyed pseudonym of an email for audit/forensics storage.

    Keyed (not plain SHA-256) so a leaked table cannot be reversed with a
    precomputed dictionary of email addresses.
    """
    normalized = normalize_email(email)
    return hmac.new(
        settings.secret_key.encode("utf-8"),
        normalized.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()


def hash_ip(ip_address: str) -> str:
    """Keyed hash of an IP address for privacy-preserving rate-limit/audit keys."""
    return hmac.new(
        settings.secret_key.encode("utf-8"),
        ip_address.encode("utf-8"),
        hashlib.sha256,
    ).hexdigest()[:32]


def generate_csrf_token() -> str:
    """Generate a CSRF token."""
    return secrets.token_urlsafe(32)


def verify_csrf_token(token: str, expected: str) -> bool:
    """Verify CSRF token using constant-time comparison."""
    if not token or not expected:
        return False
    return secrets.compare_digest(token.encode("utf-8"), expected.encode("utf-8"))


def normalize_email(email: str) -> str:
    """
    Normalize email address for consistent storage and comparison.
    Converts to lowercase and strips whitespace.
    """
    return email.strip().lower()


def is_weak_password(password: str) -> tuple[bool, list[str]]:
    """
    Check if password meets the configured policy.
    Back-compatible wrapper over the PasswordPolicy chain (Strategy).
    Returns (is_weak, list_of_issues).
    """
    issues = DEFAULT_PASSWORD_POLICY.validate(password)
    return len(issues) > 0, issues
