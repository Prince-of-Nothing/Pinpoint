"""
Repositories (data-access layer).

Design pattern: Repository — each repository encapsulates one aggregate's
queries so services and routers never build ad-hoc queries (testable, and the
single place where ORM details live). Repositories do NOT commit: the service
layer owns transaction boundaries.
"""

from datetime import datetime

from sqlalchemy.orm import Session as OrmSession

from app.models.user import (
    Friendship,
    FriendshipStatus,
    LoginAttempt,
    Session,
    User,
)


class UserRepository:
    """Data access for users."""

    def __init__(self, db: OrmSession):
        self._db = db

    def get_by_id(self, user_id: int) -> User | None:
        return self._db.get(User, user_id)

    def get_by_email_normalized(self, email_normalized: str) -> User | None:
        return self._db.query(User).filter(User.email_normalized == email_normalized).first()

    def add(self, user: User) -> User:
        self._db.add(user)
        self._db.flush()
        return user


class SessionRepository:
    """Data access for server-side sessions (tokens stored only as hashes)."""

    def __init__(self, db: OrmSession):
        self._db = db

    def add(self, session: Session) -> Session:
        self._db.add(session)
        self._db.flush()
        return session

    def get_by_token_hash(self, token_hash: str) -> Session | None:
        return self._db.query(Session).filter(Session.token_hash == token_hash).first()

    def get_by_id(self, session_id: int) -> Session | None:
        return self._db.get(Session, session_id)

    def revoke(self, session: Session, when: datetime) -> None:
        if session.revoked_at is None:
            session.revoked_at = when

    def revoke_by_token_hash(self, token_hash: str, when: datetime) -> bool:
        revoked = (
            self._db.query(Session)
            .filter(Session.token_hash == token_hash, Session.revoked_at.is_(None))
            .update({"revoked_at": when}, synchronize_session=False)
        )
        return revoked > 0

    def revoke_all_for_user(
        self, user_id: int, when: datetime, except_session_id: int | None = None
    ) -> int:
        query = self._db.query(Session).filter(
            Session.user_id == user_id,
            Session.revoked_at.is_(None),
        )
        if except_session_id is not None:
            query = query.filter(Session.id != except_session_id)
        return query.update({"revoked_at": when}, synchronize_session=False)


class LoginAttemptRepository:
    """Data access for login attempts (stores pseudonymised emails only)."""

    def __init__(self, db: OrmSession):
        self._db = db

    def record(self, email_hash: str, ip_address: str, success: bool) -> LoginAttempt:
        attempt = LoginAttempt(
            email_hash=email_hash,
            ip_address=ip_address,
            success=success,
        )
        self._db.add(attempt)
        self._db.flush()
        return attempt


class FriendshipRepository:
    """Data access for friendships."""

    def __init__(self, db: OrmSession):
        self._db = db

    def get_by_id(self, friendship_id: int) -> Friendship | None:
        return self._db.get(Friendship, friendship_id)

    def find_between(self, user_a: int, user_b: int) -> Friendship | None:
        """Find a friendship row in either direction between two users."""
        return (
            self._db.query(Friendship)
            .filter(
                ((Friendship.requester_id == user_a) & (Friendship.receiver_id == user_b))
                | ((Friendship.requester_id == user_b) & (Friendship.receiver_id == user_a))
            )
            .first()
        )

    def add(self, friendship: Friendship) -> Friendship:
        self._db.add(friendship)
        self._db.flush()
        return friendship

    def list_for_user(self, user_id: int) -> list[Friendship]:
        return (
            self._db.query(Friendship)
            .filter((Friendship.requester_id == user_id) | (Friendship.receiver_id == user_id))
            .order_by(Friendship.created_at.desc())
            .all()
        )

    def friend_ids_for(self, user_id: int) -> list[int]:
        """IDs of accepted, non-blocked friends of a user."""
        rows = (
            self._db.query(Friendship)
            .filter(
                Friendship.status == FriendshipStatus.ACCEPTED,
                (Friendship.requester_id == user_id) | (Friendship.receiver_id == user_id),
            )
            .all()
        )
        return [row.other_party(user_id) for row in rows]
