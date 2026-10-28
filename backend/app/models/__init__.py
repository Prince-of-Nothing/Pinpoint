"""ORM models. Importing this package registers every table on ``Base.metadata``."""

from app.models.audit import AuditEvent
from app.models.auth_session import AuthSession, RefreshToken
from app.models.checkin import CheckIn, CheckInStatus
from app.models.location import LatestLocation, LocationShare, SharePrecision
from app.models.social import Block, Friendship, FriendshipStatus, pair_key
from app.models.user import User, UserRole

__all__ = [
    "AuditEvent",
    "AuthSession",
    "Block",
    "CheckIn",
    "CheckInStatus",
    "Friendship",
    "FriendshipStatus",
    "LatestLocation",
    "LocationShare",
    "RefreshToken",
    "SharePrecision",
    "User",
    "UserRole",
    "pair_key",
]
