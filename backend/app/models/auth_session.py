"""Server-side sessions and rotating refresh tokens."""

from datetime import datetime

from sqlalchemy import ForeignKey, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.core.timeutils import utcnow
from app.db.base import Base, UTCDateTime
from app.models._mixins import created_at_column, id_column
from app.models.user import User


class AuthSession(Base):
    """
    One login on one device. Access tokens reference the session id (``sid``),
    so revoking the session immediately invalidates every token issued for it.
    """

    __tablename__ = "auth_sessions"

    id: Mapped[str] = id_column()
    user_id: Mapped[str] = mapped_column(ForeignKey("users.id", ondelete="CASCADE"), index=True)
    created_at: Mapped[datetime] = created_at_column()
    last_seen_at: Mapped[datetime] = mapped_column(UTCDateTime, default=utcnow)
    expires_at: Mapped[datetime] = mapped_column(UTCDateTime)
    revoked_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
    revoked_reason: Mapped[str | None] = mapped_column(String(40))
    ip_address: Mapped[str | None] = mapped_column(String(45))
    user_agent: Mapped[str | None] = mapped_column(String(255))
    # SHA-256 of the CSRF token bound to this session (cookie-based web clients)
    csrf_token_hash: Mapped[str | None] = mapped_column(String(64))

    user: Mapped[User] = relationship()


class RefreshToken(Base):
    """
    Opaque refresh token, stored only as a SHA-256 hash. Each use rotates the
    token; presenting an already-used token revokes the whole session
    (refresh-token reuse detection).
    """

    __tablename__ = "refresh_tokens"

    id: Mapped[str] = id_column()
    session_id: Mapped[str] = mapped_column(
        ForeignKey("auth_sessions.id", ondelete="CASCADE"), index=True
    )
    token_hash: Mapped[str] = mapped_column(String(64), unique=True, index=True)
    created_at: Mapped[datetime] = created_at_column()
    expires_at: Mapped[datetime] = mapped_column(UTCDateTime)
    used_at: Mapped[datetime | None] = mapped_column(UTCDateTime)

    session: Mapped[AuthSession] = relationship()
