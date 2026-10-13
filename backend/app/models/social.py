"""Friendships and blocks."""

import enum
from datetime import datetime

from sqlalchemy import ForeignKey, String, UniqueConstraint
from sqlalchemy.orm import Mapped, mapped_column, relationship

from app.db.base import Base, UTCDateTime
from app.models._mixins import created_at_column, id_column
from app.models.user import User


class FriendshipStatus(enum.StrEnum):
    PENDING = "pending"
    ACCEPTED = "accepted"


def pair_key(user_a: str, user_b: str) -> str:
    """Order-independent key so only one friendship row can exist per pair."""
    low, high = sorted((user_a, user_b))
    return f"{low}:{high}"


class Friendship(Base):
    """
    Mutual friendship. Declined, cancelled and removed friendships are deleted
    (and audited). A friendship never grants location access by itself.
    """

    __tablename__ = "friendships"

    id: Mapped[str] = id_column()
    requester_id: Mapped[str] = mapped_column(
        ForeignKey("users.id", ondelete="CASCADE"), index=True
    )
    addressee_id: Mapped[str] = mapped_column(
        ForeignKey("users.id", ondelete="CASCADE"), index=True
    )
    pair_key: Mapped[str] = mapped_column(String(73), unique=True)
    status: Mapped[str] = mapped_column(String(16), default=FriendshipStatus.PENDING.value)
    created_at: Mapped[datetime] = created_at_column()
    responded_at: Mapped[datetime | None] = mapped_column(UTCDateTime)

    requester: Mapped[User] = relationship(foreign_keys=[requester_id])
    addressee: Mapped[User] = relationship(foreign_keys=[addressee_id])

    def other_party(self, user_id: str) -> User:
        return self.addressee if self.requester_id == user_id else self.requester


class Block(Base):
    __tablename__ = "blocks"
    __table_args__ = (UniqueConstraint("blocker_id", "blocked_id", name="uq_block_pair"),)

    id: Mapped[str] = id_column()
    blocker_id: Mapped[str] = mapped_column(ForeignKey("users.id", ondelete="CASCADE"), index=True)
    blocked_id: Mapped[str] = mapped_column(ForeignKey("users.id", ondelete="CASCADE"), index=True)
    created_at: Mapped[datetime] = created_at_column()

    blocked: Mapped[User] = relationship(foreign_keys=[blocked_id])
