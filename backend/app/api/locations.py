"""
Location updates and the recipient view.

There is intentionally no endpoint that takes another user's id and returns
their location: a recipient can only ask "what am I allowed to see?", and the
answer is derived from active shares addressed to the caller. This removes
the IDOR attack surface by design.
"""

from datetime import timedelta

from fastapi import APIRouter, Request, Response, status
from sqlalchemy import or_, select

from app.api.deps import CurrentUser, DbSession
from app.core.errors import ProblemError, not_found
from app.core.timeutils import as_utc, utcnow
from app.models import LatestLocation, LocationShare, SharePrecision
from app.schemas.locations import (
    LocationUpdate,
    LocationUpdateResult,
    NavigationLinks,
    OwnLocation,
    VisibleLocation,
)
from app.schemas.social import PublicUser
from app.services import audit, ratelimit
from app.services.locations import LOW_BATTERY_PCT, approximate, freshness, navigation_links
from app.services.relationships import are_friends, is_blocked_either_way

router = APIRouter()

MAX_CLOCK_SKEW = timedelta(minutes=2)
MAX_BACKFILL = timedelta(hours=24)


def _own(location: LatestLocation) -> OwnLocation:
    label, age = freshness(location.recorded_at)
    return OwnLocation(
        latitude=location.latitude,
        longitude=location.longitude,
        accuracy_m=location.accuracy_m,
        battery_pct=location.battery_pct,
        is_charging=location.is_charging,
        recorded_at=location.recorded_at,
        received_at=location.received_at,
        freshness=label,
        age_seconds=age,
    )


@router.put(
    "/me",
    response_model=LocationUpdateResult,
    dependencies=[ratelimit.rate_limit(ratelimit.LOCATION_UPDATE)],
)
def update_my_location(
    body: LocationUpdate, user: CurrentUser, db: DbSession
) -> LocationUpdateResult:
    now = utcnow()
    recorded_at = as_utc(body.recorded_at) or now
    if recorded_at > now + MAX_CLOCK_SKEW:
        raise ProblemError(422, "INVALID_TIMESTAMP", "recorded_at is in the future.")
    if recorded_at < now - MAX_BACKFILL:
        raise ProblemError(422, "INVALID_TIMESTAMP", "recorded_at is too old.")
    recorded_at = min(recorded_at, now)  # never let a skewed clock make data look fresher

    location = db.get(LatestLocation, user.id)
    if location is not None and recorded_at <= location.recorded_at:
        # Out-of-order delivery: keep the newer point already stored
        return LocationUpdateResult(applied=False, location=_own(location))

    if location is None:
        location = LatestLocation(user_id=user.id)
        db.add(location)
    location.latitude = body.latitude
    location.longitude = body.longitude
    location.accuracy_m = body.accuracy_m
    location.battery_pct = body.battery_pct
    location.is_charging = body.is_charging
    location.recorded_at = recorded_at
    location.received_at = now
    db.commit()
    return LocationUpdateResult(applied=True, location=_own(location))


@router.get("/me", response_model=OwnLocation)
def get_my_location(user: CurrentUser, db: DbSession) -> OwnLocation:
    location = db.get(LatestLocation, user.id)
    if location is None:
        raise not_found("Location")
    return _own(location)


@router.delete("/me", status_code=status.HTTP_204_NO_CONTENT)
def delete_my_location(request: Request, user: CurrentUser, db: DbSession) -> Response:
    """Erase the stored location (e.g. when pausing sharing). Recipients then see 'unknown'."""
    location = db.get(LatestLocation, user.id)
    if location is not None:
        db.delete(location)
        audit.record(
            db,
            "location.deleted",
            actor_id=user.id,
            target_type="user",
            target_id=user.id,
            request=request,
        )
        db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.get("/visible", response_model=list[VisibleLocation])
def visible_locations(user: CurrentUser, db: DbSession) -> list[VisibleLocation]:
    """
    Locations the caller may see right now. Every row is re-authorised on each
    request: the share must be active and unexpired, the two users must still
    be friends and neither may have blocked the other.
    """
    now = utcnow()
    shares = db.scalars(
        select(LocationShare)
        .where(
            LocationShare.recipient_id == user.id,
            LocationShare.revoked_at.is_(None),
            or_(LocationShare.expires_at.is_(None), LocationShare.expires_at > now),
        )
        .order_by(LocationShare.created_at)
    ).all()

    result: list[VisibleLocation] = []
    for share in shares:
        if not are_friends(db, share.owner_id, user.id) or is_blocked_either_way(
            db, share.owner_id, user.id
        ):
            continue
        item = VisibleLocation(
            share_id=share.id,
            user=PublicUser.model_validate(share.owner),
            precision=share.precision,
            share_expires_at=share.expires_at,
            freshness="unknown",
        )
        location = db.get(LatestLocation, share.owner_id)
        if location is not None:
            if share.precision == SharePrecision.EXACT.value:
                lat, lon = round(location.latitude, 6), round(location.longitude, 6)
                accuracy = location.accuracy_m
            else:
                lat, lon, min_accuracy = approximate(location.latitude, location.longitude)
                accuracy = max(location.accuracy_m or 0, min_accuracy)
            label, age = freshness(location.recorded_at, now)
            item.freshness = label
            item.latitude, item.longitude, item.accuracy_m = lat, lon, accuracy
            item.recorded_at = location.recorded_at
            item.age_seconds = age
            # A battery value is always "last known", reported with its timestamp
            item.last_battery_pct = location.battery_pct
            item.is_charging = location.is_charging if label == "live" else None
            item.low_battery = (
                location.battery_pct is not None and location.battery_pct <= LOW_BATTERY_PCT
            )
            item.navigation = NavigationLinks(**navigation_links(lat, lon))
        result.append(item)
    return result
