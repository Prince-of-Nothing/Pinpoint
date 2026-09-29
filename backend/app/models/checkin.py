"""Safety check-ins."""

import enum
from datetime import datetime

from sqlalchemy import ForeignKey, Integer, String
from sqlalchemy.orm import Mapped, mapped_column

from app.db.base import Base, UTCDateTime
from app.models._mixins import created_at_column, id_column


class CheckInStatus(enum.StrEnum):
    ACTIVE = "active"
    COMPLETED = "completed"
    CANCELLED = "cancelled"


class CheckIn(Base):
    """
    "I will check in by ``due_at``." Overdue and missed states are derived
    from the clock when read, so no background worker is needed in the MVP.
    """

    __tablename__ = "check_ins"

    id: Mapped[str] = id_column()
    user_id: Mapped[str] = mapped_column(ForeignKey("users.id", ondelete="CASCADE"), index=True)
    note: Mapped[str | None] = mapped_column(String(140))
    due_at: Mapped[datetime] = mapped_column(UTCDateTime)
    grace_minutes: Mapped[int] = mapped_column(Integer, default=15)
    status: Mapped[str] = mapped_column(String(16), default=CheckInStatus.ACTIVE.value)
    created_at: Mapped[datetime] = created_at_column()
    completed_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
    cancelled_at: Mapped[datetime | None] = mapped_column(UTCDateTime)
