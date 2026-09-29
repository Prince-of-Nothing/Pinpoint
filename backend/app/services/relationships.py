"""Friendship/block lookups shared by every authorization decision."""

from sqlalchemy import or_, select, update
from sqlalchemy.orm import Session

from app.core.timeutils import utcnow
from app.models import Block, Friendship, FriendshipStatus, LocationShare, User, pair_key


def active_user_by_username(db: Session, username: str) -> User | None:
    return db.scalar(
        select(User).where(User.username == username.strip().lower(), User.is_active.is_(True))
    )


def is_blocked_either_way(db: Session, a: str, b: str) -> bool:
    return (
        db.scalar(
            select(Block.id).where(
                or_(
                    (Block.blocker_id == a) & (Block.blocked_id == b),
                    (Block.blocker_id == b) & (Block.blocked_id == a),
                )
            )
        )
        is not None
    )


def friendship_between(db: Session, a: str, b: str) -> Friendship | None:
    return db.scalar(select(Friendship).where(Friendship.pair_key == pair_key(a, b)))


def are_friends(db: Session, a: str, b: str) -> bool:
    friendship = friendship_between(db, a, b)
    return friendship is not None and friendship.status == FriendshipStatus.ACCEPTED.value


def revoke_shares_between(db: Session, a: str, b: str, reason: str) -> int:
    """Revoke every active location share between two users, in both directions."""
    result = db.execute(
        update(LocationShare)
        .where(
            or_(
                (LocationShare.owner_id == a) & (LocationShare.recipient_id == b),
                (LocationShare.owner_id == b) & (LocationShare.recipient_id == a),
            ),
            LocationShare.revoked_at.is_(None),
        )
        .values(revoked_at=utcnow(), revoked_reason=reason)
    )
    return result.rowcount or 0
