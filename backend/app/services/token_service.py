"""
JWT token issuance and validation.

Design patterns applied:
- Facade: a small surface (TokenService) over the PyJWT library.
- Fixed algorithm allowlist and strict claim requirements (fail closed):
  a token missing `type`, `ver` (token version), `sid` (session id) or `jti`
  is rejected, so a refresh token can never be replayed as an access token.

Library note: PyJWT is used instead of python-jose (the latter carries known
unfixed CVEs around algorithm handling; PyJWT is actively maintained).
"""

import logging
import uuid
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Optional

import jwt

from app.core.config import get_settings

logger = logging.getLogger(__name__)

settings = get_settings()

TOKEN_TYPE_ACCESS = "access"
TOKEN_TYPE_REFRESH = "refresh"


@dataclass(frozen=True)
class TokenClaims:
    """Validated token claims (never raw user input)."""

    user_id: int
    session_id: int
    token_version: int
    token_type: str
    jti: str
    expires_at: datetime


class TokenService:
    """Facade over JWT creation and validated decoding."""

    def __init__(self, secret_key: str, algorithm: str,
                 access_ttl_minutes: int, refresh_ttl_days: int):
        self._secret = secret_key
        self._algorithm = algorithm
        self._access_ttl = timedelta(minutes=access_ttl_minutes)
        self._refresh_ttl = timedelta(days=refresh_ttl_days)

    # Issuance ---------------------------------------------------------------

    def create_access_token(self, user_id: int, session_id: int, token_version: int) -> str:
        return self._encode(
            user_id=user_id,
            session_id=session_id,
            token_version=token_version,
            token_type=TOKEN_TYPE_ACCESS,
            ttl=self._access_ttl,
        )

    def create_refresh_token(self, user_id: int, session_id: int, token_version: int) -> str:
        return self._encode(
            user_id=user_id,
            session_id=session_id,
            token_version=token_version,
            token_type=TOKEN_TYPE_REFRESH,
            ttl=self._refresh_ttl,
        )

    def _encode(self, *, user_id: int, session_id: int, token_version: int,
                token_type: str, ttl: timedelta) -> str:
        now = datetime.now(timezone.utc)
        payload = {
            "sub": str(user_id),
            "sid": session_id,
            "ver": token_version,
            "type": token_type,
            "jti": uuid.uuid4().hex,
            "iat": now,
            "nbf": now,
            "exp": now + ttl,
            "iss": "always-together-api",
        }
        return jwt.encode(payload, self._secret, algorithm=self._algorithm)

    # Validation -------------------------------------------------------------

    def decode(self, token: str, expected_type: str = TOKEN_TYPE_ACCESS) -> Optional[TokenClaims]:
        """
        Decode and validate a JWT.

        Returns None if the token is invalid, expired, signed with an unknown
        algorithm, of the wrong type, or missing required claims.
        """
        try:
            payload = jwt.decode(
                token,
                self._secret,
                algorithms=[self._algorithm],  # explicit allowlist
                options={"require": ["exp", "iat", "sub", "sid", "ver", "type", "jti"]},
                issuer="always-together-api",
            )
        except jwt.PyJWTError as exc:
            logger.info("Rejected token: %s", type(exc).__name__)
            return None

        if payload.get("type") != expected_type:
            # e.g. a refresh token presented as an access token.
            logger.info("Rejected token: unexpected type %r", payload.get("type"))
            return None

        try:
            return TokenClaims(
                user_id=int(payload["sub"]),
                session_id=int(payload["sid"]),
                token_version=int(payload["ver"]),
                token_type=str(payload["type"]),
                jti=str(payload["jti"]),
                expires_at=datetime.fromtimestamp(payload["exp"], tz=timezone.utc),
            )
        except (TypeError, ValueError):
            logger.warning("Rejected token: malformed claims")
            return None


_token_service: Optional[TokenService] = None


def get_token_service() -> TokenService:
    """Cached TokenService instance (Singleton; settings never change at runtime)."""
    global _token_service
    if _token_service is None:
        _token_service = TokenService(
            secret_key=settings.secret_key,
            algorithm=settings.algorithm,
            access_ttl_minutes=settings.access_token_expire_minutes,
            refresh_ttl_days=settings.refresh_token_expire_days,
        )
    return _token_service
