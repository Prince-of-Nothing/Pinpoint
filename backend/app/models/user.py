"""User accounts."""

import enum
from datetime import datetime

from sqlalchemy import Boolean, Integer, String
from sqlalchemy.orm import Mapped, mapped_column

from app.core.timeutils import utcnow
from app.db.base import Base, UTCDateTime
from app.models._mixins import created_at_column, id_column


class UserRole(enum.StrEnum):
    USER = "user"
    ADMIN = "admin"


class User(Base):
    __tablename__ = "users"

    id: Mapped[str] = id_column()
    username: Mapped[str] = mapped_column(String(30), unique=True, index=True)
    email: Mapped[str] = mapped_column(String(255), unique=True, index=True)
    password_hash: Mapped[str] = mapped_column(String(255))
    display_name: Mapped[str | None] = mapped_column(String(50))
    bio: Mapped[str | None] = mapped_column(String(160))
    role: Mapped[str] = mapped_column(String(16), default=UserRole.USER.value)

    is_active: Mapped[bool] = mapped_column(Boolean, default=True)
    failed_login_attempts: Mapped[int] = mapped_column(Integer, default=0)
    locked_until: Mapped[datetime | None] = mapped_column(UTCDateTime)

    created_at: Mapped[datetime] = created_at_column()
    updated_at: Mapped[datetime] = mapped_column(UTCDateTime, default=utcnow, onupdate=utcnow)
    last_login_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
    password_changed_at: Mapped[datetime] = mapped_column(UTCDateTime, default=utcnow)

    @property
    def is_admin(self) -> bool:
        return self.role == UserRole.ADMIN.value

    def __repr__(self) -> str:  # never include email or hashes in logs
        return f"<User id={self.id} username={self.username}>"
