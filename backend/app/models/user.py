"""
Database models for Always Together application.
Implements user, session, friendship, and location sharing tables.

Security-relevant design decisions:
- Session tokens are never stored in plaintext; only a SHA-256 hash is
  persisted (see app.services.security.hash_token).
- Login attempts store a pseudonymized (hashed) email, not the raw address
  (privacy-preserving audit trail, SC-08 / GDPR data minimisation).
- User.token_version lets the server invalidate every outstanding JWT for a
  user (password change, account recovery) without a per-token denylist.
- Friendships carry integrity constraints: no self-friendship, at most one
  friendship row per user pair.
"""

from datetime import datetime, timezone
import enum

from sqlalchemy import (
    Boolean,
    CheckConstraint,
    Column,
    DateTime,
    Enum,
    Float,
    ForeignKey,
    Index,
    Integer,
    String,
    Text,
    UniqueConstraint,
)
from sqlalchemy.orm import relationship
from sqlalchemy.sql import func

from app.db import Base


class UserRole(str, enum.Enum):
    """User role enumeration."""
    RESIDENT = "resident"
    ADMINISTRATOR = "administrator"


class FriendshipStatus(str, enum.Enum):
    """Friendship request status."""
    PENDING = "pending"
    ACCEPTED = "accepted"
    REJECTED = "rejected"
    BLOCKED = "blocked"


class LocationShareDirection(str, enum.Enum):
    """Direction of location sharing permission."""
    TO_FRIEND = "to_friend"  # I share my location with friend
    FROM_FRIEND = "from_friend"  # Friend shares their location with me
    MUTUAL = "mutual"  # Both share with each other


class User(Base):
    """User account model with authentication and profile data."""

    __tablename__ = "users"

    id = Column(Integer, primary_key=True, index=True)
    email = Column(String(255), unique=True, index=True, nullable=False)
    email_normalized = Column(String(255), unique=True, index=True, nullable=False)
    password_hash = Column(String(255), nullable=False)
    full_name = Column(String(100), nullable=False)
    role = Column(Enum(UserRole), default=UserRole.RESIDENT, nullable=False)

    # Account security
    is_active = Column(Boolean, default=True, nullable=False)
    is_locked = Column(Boolean, default=False, nullable=False)
    locked_at = Column(DateTime(timezone=True), nullable=True)
    failed_login_attempts = Column(Integer, default=0, nullable=False)
    must_change_password = Column(Boolean, default=False, nullable=False)
    # Bumped to invalidate all previously issued JWTs for this user.
    token_version = Column(Integer, default=1, nullable=False)

    # Timestamps
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    updated_at = Column(DateTime(timezone=True), onupdate=func.now(), nullable=True)
    last_login_at = Column(DateTime(timezone=True), nullable=True)

    # Relationships
    sessions = relationship("Session", back_populates="user", cascade="all, delete-orphan")
    friendships_as_requester = relationship(
        "Friendship",
        foreign_keys="Friendship.requester_id",
        back_populates="requester",
        cascade="all, delete-orphan"
    )
    friendships_as_receiver = relationship(
        "Friendship",
        foreign_keys="Friendship.receiver_id",
        back_populates="receiver",
        cascade="all, delete-orphan"
    )
    location_shares = relationship("LocationShare", back_populates="owner", cascade="all, delete-orphan")
    location_update = relationship(
        "LocationUpdate", back_populates="owner", uselist=False, cascade="all, delete-orphan"
    )

    def __repr__(self):
        return f"<User(id={self.id}, email='{self.email}', role={self.role})>"


class Session(Base):
    """
    Server-side session model for tracking active user sessions.

    `token_hash` stores SHA-256(raw cookie value): a database leak does not
    hand the attacker usable session cookies.
    """

    __tablename__ = "sessions"

    id = Column(Integer, primary_key=True, index=True)
    token_hash = Column(String(64), unique=True, index=True, nullable=False)
    user_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), index=True, nullable=False)

    # Session metadata
    ip_address = Column(String(45), nullable=True)  # IPv6 max length
    user_agent = Column(Text, nullable=True)

    # Timestamps
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    expires_at = Column(DateTime(timezone=True), nullable=False)
    last_activity_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    revoked_at = Column(DateTime(timezone=True), nullable=True)

    # Relationships
    user = relationship("User", back_populates="sessions")

    def is_valid(self, now: datetime | None = None) -> bool:
        """Check if session is still valid (not revoked, not expired, owner usable)."""
        from app.core.timeutils import ensure_utc, utcnow

        now = now or utcnow()
        expires_at = ensure_utc(self.expires_at)
        if self.revoked_at is not None or expires_at is None or expires_at <= now:
            return False
        owner = self.user
        return bool(owner and owner.is_active and not owner.is_locked)

    def __repr__(self):
        return f"<Session(id={self.id}, user_id={self.user_id}, expires={self.expires_at})>"


class Friendship(Base):
    """Friendship relationship between users with directional permissions."""

    __tablename__ = "friendships"

    id = Column(Integer, primary_key=True, index=True)
    requester_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), nullable=False)
    receiver_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), nullable=False)
    status = Column(Enum(FriendshipStatus), default=FriendshipStatus.PENDING, nullable=False)

    # Timestamps
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    updated_at = Column(DateTime(timezone=True), onupdate=func.now(), nullable=True)
    responded_at = Column(DateTime(timezone=True), nullable=True)

    # Relationships
    requester = relationship("User", foreign_keys=[requester_id], back_populates="friendships_as_requester")
    receiver = relationship("User", foreign_keys=[receiver_id], back_populates="friendships_as_receiver")
    location_shares = relationship("LocationShare", back_populates="friendship", cascade="all, delete-orphan")

    __table_args__ = (
        # One row per unordered user pair (duplicate A→B / B→A prevention).
        UniqueConstraint("requester_id", "receiver_id", name="uq_friendship_pair"),
        # Nobody can befriend themselves.
        CheckConstraint("requester_id <> receiver_id", name="ck_friendship_not_self"),
        Index("ix_friendships_receiver_status", "receiver_id", "status"),
    )

    def other_party(self, user_id: int) -> int:
        """Return the counterparty user id for a participant of this friendship."""
        return self.receiver_id if self.requester_id == user_id else self.requester_id

    def __repr__(self):
        return f"<Friendship(requester={self.requester_id}, receiver={self.receiver_id}, status={self.status})>"


class LocationShare(Base):
    """
    Directional location sharing permission.
    Controls who can see whose location based on explicit consent.

    The recipient is implied by direction + the linked friendship:
    TO_FRIEND/MUTUAL means the owner shares with the friendship counterparty.
    """

    __tablename__ = "location_shares"

    id = Column(Integer, primary_key=True, index=True)
    friendship_id = Column(Integer, ForeignKey("friendships.id", ondelete="CASCADE"), nullable=False)
    owner_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), index=True, nullable=False)

    # Direction: who is sharing with whom
    direction = Column(Enum(LocationShareDirection), nullable=False)

    # Sharing settings
    is_active = Column(Boolean, default=True, nullable=False)
    precision_level = Column(String(20), default="exact")  # exact, approximate, city_only
    purpose = Column(String(32), default="safety")  # safety, meetup, navigation, check_in
    # Optional absolute expiry; enforcement also covers "no expiry yet" shares.
    expires_at = Column(DateTime(timezone=True), nullable=True)
    revoked_at = Column(DateTime(timezone=True), nullable=True)

    # Timestamps
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    updated_at = Column(DateTime(timezone=True), onupdate=func.now(), nullable=True)
    last_location_update_at = Column(DateTime(timezone=True), nullable=True)

    # Relationships
    friendship = relationship("Friendship", back_populates="location_shares")
    owner = relationship("User", back_populates="location_shares")

    __table_args__ = (
        Index("ix_location_shares_owner_active", "owner_id", "is_active"),
    )

    def is_currently_valid(self, now: datetime | None = None) -> bool:
        """Active, not revoked, not expired."""
        from app.core.timeutils import ensure_utc, utcnow

        now = now or utcnow()
        if not self.is_active or self.revoked_at is not None:
            return False
        expires_at = ensure_utc(self.expires_at)
        return expires_at is None or expires_at > now

    def __repr__(self):
        return f"<LocationShare(friendship={self.friendship_id}, owner={self.owner_id}, direction={self.direction})>"


class LoginAttempt(Base):
    """
    Track login attempts for rate limiting and account lockout.

    Only a hashed email is stored: the table supports lockout forensics
    without becoming a database of who tried to log in.
    """

    __tablename__ = "login_attempts"

    id = Column(Integer, primary_key=True, index=True)
    email_hash = Column(String(64), nullable=False, index=True)
    ip_address = Column(String(45), nullable=False)
    success = Column(Boolean, nullable=False)
    attempted_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)

    def __repr__(self):
        return f"<LoginAttempt(email_hash='{self.email_hash[:12]}...', ip='{self.ip_address}', success={self.success})>"
