"""
Location-related repositories.

Privacy note: LocationUpdateRepository only ever stores the *latest* point per
owner (upsert). No history is accumulated — data minimisation by design.
"""

from datetime import datetime

from sqlalchemy.orm import Session as OrmSession

from app.models.location import LocationUpdate
from app.models.user import LocationShare


class LocationShareRepository:
    """Data access for directional location-share consent records."""

    def __init__(self, db: OrmSession):
        self._db = db

    def get_by_id(self, share_id: int) -> LocationShare | None:
        return self._db.get(LocationShare, share_id)

    def add(self, share: LocationShare) -> LocationShare:
        self._db.add(share)
        self._db.flush()
        return share

    def find_between(self, owner_id: int, other_id: int) -> LocationShare | None:
        """Active share owned by owner_id whose friendship links to other_id."""
        return self._match_counterparty(owner_id, other_id)

    def _match_counterparty(self, owner_id: int, other_id: int) -> LocationShare | None:
        from app.models.user import Friendship

        return (
            self._db.query(LocationShare)
            .join(Friendship, LocationShare.friendship_id == Friendship.id)
            .filter(
                LocationShare.owner_id == owner_id,
                LocationShare.is_active.is_(True),
                LocationShare.revoked_at.is_(None),
                (Friendship.requester_id == other_id) | (Friendship.receiver_id == other_id),
            )
            .first()
        )

    def get_active_between(self, owner_id: int, viewer_id: int) -> LocationShare | None:
        return self._match_counterparty(owner_id, viewer_id)

    def list_active_incoming_for(self, viewer_id: int) -> list[LocationShare]:
        """All active shares owned by someone else and directed at viewer."""
        from app.models.user import Friendship

        return (
            self._db.query(LocationShare)
            .join(Friendship, LocationShare.friendship_id == Friendship.id)
            .filter(
                LocationShare.is_active.is_(True),
                LocationShare.revoked_at.is_(None),
                (Friendship.requester_id == viewer_id) | (Friendship.receiver_id == viewer_id),
            )
            .all()
        )

    def list_owned_by(self, owner_id: int) -> list[LocationShare]:
        return (
            self._db.query(LocationShare)
            .filter(LocationShare.owner_id == owner_id)
            .order_by(LocationShare.created_at.desc())
            .all()
        )

    def revoke(self, share: LocationShare, when: datetime) -> None:
        share.is_active = False
        share.revoked_at = when

    def revoke_between(self, user_a: int, user_b: int, when: datetime) -> int:
        """Revoke every share between two users (used when blocking)."""
        from app.models.user import Friendship

        shares = (
            self._db.query(LocationShare)
            .join(Friendship, LocationShare.friendship_id == Friendship.id)
            .filter(
                LocationShare.is_active.is_(True),
                (
                    ((Friendship.requester_id == user_a) & (Friendship.receiver_id == user_b))
                    | ((Friendship.requester_id == user_b) & (Friendship.receiver_id == user_a))
                ),
            )
            .all()
        )
        for share in shares:
            self.revoke(share, when)
        return len(shares)


class LocationUpdateRepository:
    """Data access for latest-known locations (upsert-only)."""

    def __init__(self, db: OrmSession):
        self._db = db

    def get_latest(self, owner_id: int) -> LocationUpdate | None:
        return self._db.query(LocationUpdate).filter(LocationUpdate.owner_id == owner_id).first()

    def upsert_latest(
        self,
        owner_id: int,
        latitude: float,
        longitude: float,
        accuracy: float | None,
        battery_level: int | None,
        recorded_at: datetime,
    ) -> LocationUpdate:
        update = self.get_latest(owner_id)
        if update is None:
            update = LocationUpdate(owner_id=owner_id)
            self._db.add(update)
        update.latitude = latitude
        update.longitude = longitude
        update.accuracy = accuracy
        update.battery_level = battery_level
        update.recorded_at = recorded_at
        self._db.flush()
        return update
