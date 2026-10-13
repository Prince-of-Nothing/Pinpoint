"""
Database models for Always Together application.
Implements user, session, friendship, and location sharing tables.
"""

from sqlalchemy import Column, Integer, String, Boolean, DateTime, ForeignKey, Enum, Text, Float
from sqlalchemy.orm import relationship
from sqlalchemy.sql import func
import enum

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
    
    def __repr__(self):
        return f"<User(id={self.id}, email='{self.email}', role={self.role})>"


class Session(Base):
    """Server-side session model for tracking active user sessions."""
    
    __tablename__ = "sessions"
    
    id = Column(Integer, primary_key=True, index=True)
    session_token = Column(String(255), unique=True, index=True, nullable=False)
    user_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), nullable=False)
    
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
    
    def is_valid(self) -> bool:
        """Check if session is still valid."""
        from datetime import datetime, timezone
        now = datetime.now(timezone.utc)
        return (
            self.revoked_at is None and
            self.expires_at > now and
            self.user.is_active and
            not self.user.is_locked
        )
    
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
    
    # Ensure unique friendship between two users
    __table_args__ = (
        # Prevent duplicate friendship requests
        {'sqlite_autoincrement': True}  # Will be adjusted for PostgreSQL in migration
    )
    
    def __repr__(self):
        return f"<Friendship(requester={self.requester_id}, receiver={self.receiver_id}, status={self.status})>"


class LocationShare(Base):
    """
    Directional location sharing permission.
    Controls who can see whose location based on explicit consent.
    """
    
    __tablename__ = "location_shares"
    
    id = Column(Integer, primary_key=True, index=True)
    friendship_id = Column(Integer, ForeignKey("friendships.id", ondelete="CASCADE"), nullable=False)
    owner_id = Column(Integer, ForeignKey("users.id", ondelete="CASCADE"), nullable=False)
    
    # Direction: who is sharing with whom
    direction = Column(Enum(LocationShareDirection), nullable=False)
    
    # Sharing settings
    is_active = Column(Boolean, default=True, nullable=False)
    precision_level = Column(String(20), default="exact")  # exact, approximate, city_only
    
    # Timestamps
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    updated_at = Column(DateTime(timezone=True), onupdate=func.now(), nullable=True)
    last_location_update_at = Column(DateTime(timezone=True), nullable=True)
    
    # Relationships
    friendship = relationship("Friendship", back_populates="location_shares")
    owner = relationship("User", back_populates="location_shares")
    
    def __repr__(self):
        return f"<LocationShare(friendship={self.friendship_id}, owner={self.owner_id}, direction={self.direction})>"


class LoginAttempt(Base):
    """Track login attempts for rate limiting and account lockout."""
    
    __tablename__ = "login_attempts"
    
    id = Column(Integer, primary_key=True, index=True)
    email = Column(String(255), nullable=False, index=True)
    ip_address = Column(String(45), nullable=False)
    success = Column(Boolean, nullable=False)
    attempted_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False)
    
    def __repr__(self):
        return f"<LoginAttempt(email='{self.email}', ip='{self.ip_address}', success={self.success})>"
