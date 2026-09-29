"""
Access tokens (short-lived JWTs bound to a server-side session) and opaque
random secrets (refresh and CSRF tokens) that are stored only as hashes.
"""

import hashlib
import hmac
import secrets
import uuid
from datetime import timedelta

import jwt

from app.core.config import get_settings
from app.core.timeutils import utcnow

# Token type label, not a secret
ACCESS_TOKEN_TYPE = "access"  # noqa: S105  # nosec B105


def create_access_token(*, user_id: str, session_id: str, role: str) -> tuple[str, int]:
    settings = get_settings()
    now = utcnow()
    lifetime = timedelta(minutes=settings.access_token_expire_minutes)
    claims = {
        "sub": user_id,
        "sid": session_id,
        "role": role,
        "type": ACCESS_TOKEN_TYPE,
        "jti": str(uuid.uuid4()),
        "iss": settings.jwt_issuer,
        "aud": settings.jwt_audience,
        "iat": now,
        "nbf": now,
        "exp": now + lifetime,
    }
    token = jwt.encode(claims, settings.secret_key, algorithm=settings.jwt_algorithm)
    return token, int(lifetime.total_seconds())


def decode_access_token(token: str) -> dict | None:
    """Return the claims of a valid access token, or ``None``. Algorithm is pinned (no 'none')."""
    settings = get_settings()
    try:
        claims = jwt.decode(
            token,
            settings.secret_key,
            algorithms=[settings.jwt_algorithm],
            audience=settings.jwt_audience,
            issuer=settings.jwt_issuer,
            options={"require": ["exp", "iat", "nbf", "sub", "sid", "jti", "type"]},
        )
    except jwt.PyJWTError:
        return None
    if claims.get("type") != ACCESS_TOKEN_TYPE:
        return None
    return claims


def new_secret() -> str:
    """256-bit URL-safe random value for refresh and CSRF tokens."""
    return secrets.token_urlsafe(32)


def hash_secret(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def secret_matches(value: str | None, expected_hash: str | None) -> bool:
    if not value or not expected_hash:
        return False
    return hmac.compare_digest(hash_secret(value), expected_hash)
