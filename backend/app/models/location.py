"""Directional location shares and latest-known locations."""

import enum
from datetime import datetime

from sqlalchemy import Boolean, Float, ForeignKey, Index, Integer, String
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.core.timeutils import utcnow
from app.db.base import Base, UTCDateTime
from app.models._mixins import created_at_column, id_column
from app.models.user import User


class SharePrecision(enum.StrEnum):
    EXACT = "exact"
    APPROXIMATE = "approximate"


class LocationShare(Base):
    """
    One-way consent: ``owner`` lets ``recipient`` see the owner's location at
    the chosen precision until ``expires_at`` or revocation. The recipient
    gains nothing in the opposite direction.
    """

    __tablename__ = "location_shares"
    __table_args__ = (Index("ix_location_shares_owner_recipient", "owner_id", "recipient_id"),)

    id: Mapped[str] = id_column()
    owner_id: Mapped[str] = mapped_column(ForeignKey("users.id", ondelete="CASCADE"), index=True)
    recipient_id: Mapped[str] = mapped_column(
        ForeignKey("users.id", ondelete="CASCADE"), index=True
    )
    precision: Mapped[str] = mapped_column(String(16), default=SharePrecision.APPROXIMATE.value)
    purpose: Mapped[str | None] = mapped_column(String(100))
    expires_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
    created_at: Mapped[datetime] = created_at_column()
    updated_at: Mapped[datetime] = mapped_column(UTCDateTime, default=utcnow, onupdate=utcnow)
    revoked_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
    revoked_reason: Mapped[str | None] = mapped_column(String(40))

    owner: Mapped[User] = relationship(foreign_keys=[owner_id])
    recipient: Mapped[User] = relationship(foreign_keys=[recipient_id])

    def is_active(self, now: datetime) -> bool:
        return self.revoked_at is None and (self.expires_at is None or self.expires_at > now)


class LatestLocation(Base):
    """
    Only the most recent position per user is stored (no history), which
    limits what a database compromise can reveal.
    """

    __tablename__ = "latest_locations"

    user_id: Mapped[str] = mapped_column(
        ForeignKey("users.id", ondelete="CASCADE"), primary_key=True
    )
    latitude: Mapped[float] = mapped_column(Float)
    longitude: Mapped[float] = mapped_column(Float)
    accuracy_m: Mapped[float | None] = mapped_column(Float)
    battery_pct: Mapped[int | None] = mapped_column(Integer)
    is_charging: Mapped[bool | None] = mapped_column(Boolean)
    recorded_at: Mapped[datetime] = mapped_column(UTCDateTime)
    received_at: Mapped[datetime] = mapped_column(UTCDateTime, default=utcnow)
