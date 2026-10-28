"""
Authorization policies (the complete-mediation core, SC-02 / SC-03).

Design patterns applied:
- Strategy: RelationshipService, LocationAccessPolicy, ProfileVisibilityPolicy
  and the PrecisionReducer family are interchangeable, individually testable
  algorithms behind small interfaces.
- Registry: precision reducers are looked up by name, so adding a level never
  requires editing the access-policy code (Open/Closed).

Every location read or profile read goes through one of these policies — no
router ever decides visibility with inline conditionals.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from datetime import datetime
from enum import StrEnum

from app.core.timeutils import utcnow
from app.models.user import (
    Friendship,
    FriendshipStatus,
    LocationShare,
    LocationShareDirection,
    User,
)
from app.repositories.people import FriendshipRepository


class Relationship(StrEnum):
    """Viewer's relationship to a target account."""

    SELF = "self"
    FRIEND = "friend"
    PENDING = "pending"
    BLOCKED = "blocked"
    NON_FRIEND = "non_friend"


class RelationshipService:
    """Resolves the viewer↔target relationship (used by all visibility policies)."""

    def __init__(self, friendships: FriendshipRepository):
        self._friendships = friendships

    def resolve(self, viewer_id: int, target_id: int) -> Relationship:
        if viewer_id == target_id:
            return Relationship.SELF

        friendship = self._friendships.find_between(viewer_id, target_id)
        if friendship is None:
            return Relationship.NON_FRIEND
        if friendship.status == FriendshipStatus.BLOCKED:
            return Relationship.BLOCKED
        if friendship.status == FriendshipStatus.ACCEPTED:
            return Relationship.FRIEND
        if friendship.status == FriendshipStatus.PENDING:
            return Relationship.PENDING
        return Relationship.NON_FRIEND


# ---------------------------------------------------------------------------
# Precision reducers (Strategy + registry)
# ---------------------------------------------------------------------------


class PrecisionReducer(ABC):
    """Reduces a coordinate pair to a coarser representation."""

    @property
    @abstractmethod
    def label(self) -> str: ...

    @abstractmethod
    def reduce(self, latitude: float, longitude: float) -> tuple[float, float]: ...


class ExactPrecision(PrecisionReducer):
    @property
    def label(self) -> str:
        return "exact"

    def reduce(self, latitude: float, longitude: float) -> tuple[float, float]:
        return latitude, longitude


class ApproximatePrecision(PrecisionReducer):
    """
    ~1 km grid (2 decimal places ≈ 110 m × 90 m cells).

    Deterministic rounding is used instead of adding random noise: repeated
    reads then return the same point, so an observer cannot average several
    jittered samples back down to the precise location.
    """

    @property
    def label(self) -> str:
        return "approximate"

    def reduce(self, latitude: float, longitude: float) -> tuple[float, float]:
        return round(latitude, 2), round(longitude, 2)


class CityOnlyPrecision(PrecisionReducer):
    """~11 km grid (1 decimal place) — city-level."""

    @property
    def label(self) -> str:
        return "city_only"

    def reduce(self, latitude: float, longitude: float) -> tuple[float, float]:
        return round(latitude, 1), round(longitude, 1)


PRECISION_REDUCERS: dict[str, PrecisionReducer] = {
    reducer.label: reducer
    for reducer in (ExactPrecision(), ApproximatePrecision(), CityOnlyPrecision())
}


def get_precision_reducer(level: str | None) -> PrecisionReducer:
    """Look up a reducer; unknown levels fail closed to the coarsest option."""
    return PRECISION_REDUCERS.get(level or "", PRECISION_REDUCERS["city_only"])


# ---------------------------------------------------------------------------
# Location access policy (complete mediation for location reads)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class LocationAccessDecision:
    """Result of a location-access check."""

    allowed: bool
    precision_level: str | None = None
    reason: str = ""


class LocationAccessPolicy:
    """
    Decides whether viewer may see owner's latest location, and at which
    precision. One decision per (viewer, owner) pair; callers must consult it
    for *every* location read (complete mediation).
    """

    def __init__(self, relationships: RelationshipService):
        self._relationships = relationships

    def check(
        self,
        viewer_id: int,
        owner_id: int,
        share: LocationShare | None,
        now: datetime | None = None,
    ) -> LocationAccessDecision:
        now = now or utcnow()

        if share is None or share.owner_id != owner_id:
            return LocationAccessDecision(False, None, "no_active_share")

        if not share.is_currently_valid(now):
            return LocationAccessDecision(
                False,
                None,
                "share_revoked" if share.revoked_at or not share.is_active else "share_expired",
            )

        friendship: Friendship | None = share.friendship
        if friendship is None or friendship.status != FriendshipStatus.ACCEPTED:
            return LocationAccessDecision(False, None, "friendship_not_accepted")

        relationship = self._relationships.resolve(viewer_id, owner_id)
        if relationship == Relationship.BLOCKED:
            return LocationAccessDecision(False, None, "blocked")
        if relationship != Relationship.FRIEND:
            return LocationAccessDecision(False, None, "not_friends")

        # Consent direction: the record's owner is the sharer; the friendship
        # counterparty is the recipient. FROM_FRIEND records grant nothing to
        # their owner (they are informational; see ADR-0003 note).
        if share.direction not in (LocationShareDirection.TO_FRIEND, LocationShareDirection.MUTUAL):
            return LocationAccessDecision(False, None, "direction_mismatch")

        return LocationAccessDecision(True, share.precision_level, "granted")


# ---------------------------------------------------------------------------
# Profile visibility policy (SC-03: private by default)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class VisibleProfile:
    """Relationship-filtered projection of a user profile."""

    user_id: int | None
    display_name: str | None
    is_full: bool
    reason: str


class ProfileVisibilityPolicy:
    """Backend response filtering — never trust the client to hide fields."""

    def visible_profile(
        self,
        viewer_id: int,
        target: User | None,
        relationship: Relationship,
    ) -> VisibleProfile:
        if target is None:
            # Indistinguishable from a redacted existing user: no existence leak.
            return VisibleProfile(None, None, False, "not_found_or_redacted")

        if relationship == Relationship.SELF:
            return VisibleProfile(target.id, target.full_name, True, "self")

        if relationship == Relationship.FRIEND:
            return VisibleProfile(target.id, target.full_name, False, "friend")

        # Pending, blocked, or stranger: nothing beyond a redacted shell.
        return VisibleProfile(None, None, False, "redacted")
