"""Session lifecycle: creation, refresh-token rotation with reuse detection, revocation."""

from dataclasses import dataclass
from datetime import timedelta

from fastapi import Request
from sqlalchemy import select, update
from sqlalchemy.orm import Session

from app.core.config import get_settings
from app.core.timeutils import utcnow
from app.models import AuthSession, RefreshToken, User
from app.services import audit
from app.services.tokens import create_access_token, hash_secret, new_secret


@dataclass
class IssuedTokens:
    session: AuthSession
    access_token: str
    expires_in: int
    refresh_token: str
    csrf_token: str | None


def _issue_refresh_token(db: Session, session: AuthSession) -> str:
    raw = new_secret()
    db.add(
        RefreshToken(
            session_id=session.id, token_hash=hash_secret(raw), expires_at=session.expires_at
        )
    )
    return raw


def _access_for(session: AuthSession, user: User) -> tuple[str, int]:
    return create_access_token(user_id=user.id, session_id=session.id, role=user.role)


def create_session(db: Session, user: User, request: Request, *, with_csrf: bool) -> IssuedTokens:
    settings = get_settings()
    now = utcnow()
    _enforce_session_limit(db, user.id, settings.max_sessions_per_user - 1)

    csrf_token = new_secret() if with_csrf else None
    session = AuthSession(
        user_id=user.id,
        created_at=now,
        last_seen_at=now,
        expires_at=now + timedelta(hours=settings.session_absolute_timeout_hours),
        ip_address=audit.client_ip(request),
        user_agent=(request.headers.get("user-agent") or "")[:255] or None,
        csrf_token_hash=hash_secret(csrf_token) if csrf_token else None,
    )
    db.add(session)
    db.flush()
    refresh = _issue_refresh_token(db, session)
    access, expires_in = _access_for(session, user)
    return IssuedTokens(session, access, expires_in, refresh, csrf_token)


def _enforce_session_limit(db: Session, user_id: str, keep: int) -> None:
    active = db.scalars(
        select(AuthSession)
        .where(AuthSession.user_id == user_id, AuthSession.revoked_at.is_(None))
        .order_by(AuthSession.last_seen_at.desc())
    ).all()
    for old in active[keep:]:
        revoke(db, old, "session_limit")


class RefreshError(Exception):
    def __init__(self, reason: str, session: AuthSession | None = None) -> None:
        super().__init__(reason)
        self.reason = reason
        self.session = session


def session_is_usable(session: AuthSession) -> str | None:
    """Return a reason string if the session can no longer be used, else ``None``."""
    settings = get_settings()
    now = utcnow()
    if session.revoked_at is not None:
        return "revoked"
    if session.expires_at <= now:
        return "expired"
    if now - session.last_seen_at > timedelta(minutes=settings.session_idle_timeout_minutes):
        return "idle_timeout"
    user = session.user
    if not user.is_active:
        return "user_inactive"
    return None


def rotate_refresh_token(db: Session, raw_token: str) -> tuple[AuthSession, str, str, int]:
    """
    Exchange a refresh token for a new access + refresh token pair.
    Presenting a token that was already used means it was copied: the whole
    session is revoked so both the attacker and the victim must log in again.
    """
    token = db.scalar(select(RefreshToken).where(RefreshToken.token_hash == hash_secret(raw_token)))
    if token is None:
        raise RefreshError("unknown_token")
    session = token.session
    now = utcnow()
    if token.used_at is not None:
        if session.revoked_at is None:
            revoke(db, session, "refresh_token_reuse")
        raise RefreshError("reuse_detected", session)
    if token.expires_at <= now:
        raise RefreshError("expired", session)
    problem = session_is_usable(session)
    if problem:
        if session.revoked_at is None:
            revoke(db, session, problem)
        raise RefreshError(problem, session)

    token.used_at = now
    session.last_seen_at = now
    new_raw = _issue_refresh_token(db, session)
    access, expires_in = _access_for(session, session.user)
    return session, new_raw, access, expires_in


def revoke(db: Session, session: AuthSession, reason: str) -> None:
    if session.revoked_at is None:
        session.revoked_at = utcnow()
        session.revoked_reason = reason


def revoke_all_for_user(db: Session, user_id: str, reason: str) -> None:
    db.execute(
        update(AuthSession)
        .where(AuthSession.user_id == user_id, AuthSession.revoked_at.is_(None))
        .values(revoked_at=utcnow(), revoked_reason=reason)
    )
