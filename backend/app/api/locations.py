"""
Location sharing API endpoints (the safety core, SC-02).

Every read and write is mediated by LocationAccessPolicy / explicit consent
records:

- A share can only be created between accepted friends, by the sharer, with a
  bounded expiry (<= 24h) — consent is explicit, purpose-bound, revocable.
- Latest locations are returned only for friends holding a valid share in the
  owner→viewer direction, and coordinates are transformed by the share's
  precision reducer before leaving the server.
- Only the *latest* point is ever stored (data minimisation; no trails).
"""

from datetime import timedelta

from fastapi import APIRouter, Depends, HTTPException, status

from app.api.deps import (
    csrf_protect,
    get_audit_logger,
    get_friendship_repository,
    get_location_share_repository,
    get_location_update_repository,
    get_relationship_service,
    rate_limit,
    require_fresh_password,
)
from app.core.timeutils import ensure_utc, utcnow
from app.db.session import get_db
from app.models.user import (
    FriendshipStatus,
    LocationShare,
    LocationShareDirection,
    User,
)
from app.repositories import (
    FriendshipRepository,
    LocationShareRepository,
    LocationUpdateRepository,
)
from app.schemas.auth import (
    FriendLocationResponse,
    LatestLocationsResponse,
    LocationShareCreate,
    LocationShareResponse,
    LocationUpdateRequest,
)
from app.services.audit import AuditLogger
from app.services.authorization import (
    LocationAccessPolicy,
    get_precision_reducer,
)

router = APIRouter()

MAX_SHARE_DURATION = timedelta(hours=24)
FUTURE_SKEW_TOLERANCE = timedelta(minutes=5)
STALE_AFTER = timedelta(minutes=30)


def _share_response(share: LocationShare) -> LocationShareResponse:
    return LocationShareResponse(
        id=share.id,
        friendship_id=share.friendship_id,
        owner_id=share.owner_id,
        direction=share.direction.value if hasattr(share.direction, "value") else str(share.direction),
        is_active=share.is_active,
        precision_level=share.precision_level or "exact",
        purpose=share.purpose or "safety",
        expires_at=ensure_utc(share.expires_at),
        created_at=share.created_at,
        revoked_at=ensure_utc(share.revoked_at),
        last_location_update_at=ensure_utc(share.last_location_update_at),
    )


def _load_share_parties(share_id: int, shares: LocationShareRepository,
                        user: User) -> LocationShare:
    """Fetch a share and require the caller to be owner or recipient (anti-IDOR)."""
    share = shares.get_by_id(share_id)
    if share is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "NOT_FOUND", "message": "Location share not found."},
        )

    counterparty = share.friendship.other_party(share.owner_id) if share.friendship else None
    if user.id not in (share.owner_id, counterparty):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Authorization denied."},
        )
    return share


# ---------------------------------------------------------------------------
# Consent records
# ---------------------------------------------------------------------------

@router.post("/shares", response_model=LocationShareResponse,
             status_code=status.HTTP_201_CREATED)
async def create_location_share(
    payload: LocationShareCreate,
    user: User = Depends(require_fresh_password),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    shares: LocationShareRepository = Depends(get_location_share_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
    _rate: None = Depends(rate_limit("location_share_create", 20, 3600)),
):
    """
    Create a directional location-share consent record.

    Consent rules: accepted friendship required, caller must be the sharer
    (owner), direction must be to_friend (a mutual share needs *both* sides'
    explicit records — one user can never grant on behalf of another), and
    expiry is mandatory-bounded at 24 hours.
    """
    friendship = friendships.get_by_id(payload.friendship_id)
    if friendship is None or friendship.status != FriendshipStatus.ACCEPTED:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Authorization denied."},
        )
    if user.id not in (friendship.requester_id, friendship.receiver_id):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Authorization denied."},
        )
    if payload.direction != LocationShareDirection.TO_FRIEND.value:
        # Only the sharer may grant consent; "mutual" must be two records so
        # each side consents separately (never granted on behalf of another).
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "INVALID_REQUEST",
                "message": (
                    "Only 'to_friend' can be created directly; a mutual share "
                    "requires each friend to create their own record."
                ),
            },
        )

    now = utcnow()
    if payload.expires_at is not None:
        expires_at = ensure_utc(payload.expires_at)
        if expires_at <= now:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={"code": "INVALID_REQUEST", "message": "Expiry must be in the future."},
            )
        if expires_at > now + MAX_SHARE_DURATION:
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={
                    "code": "INVALID_REQUEST",
                    "message": "Maximum sharing duration is 24 hours.",
                },
            )
    else:
        expires_at = now + MAX_SHARE_DURATION  # fail-safe default: bounded consent

    recipient_id = friendship.other_party(user.id)

    # Replacing an existing share: the old record is revoked, never silently
    # widened (audit keeps both events).
    existing = shares.get_active_between(user.id, recipient_id)
    if existing is not None:
        shares.revoke(existing, now)
        audit.record(
            "location_share.revoked",
            actor_id=user.id,
            target_type="location_share",
            target_id=existing.id,
            reason="replaced",
        )

    share = shares.add(
        LocationShare(
            friendship_id=friendship.id,
            owner_id=user.id,
            direction=LocationShareDirection(payload.direction),
            is_active=True,
            precision_level=payload.precision_level or "exact",
            purpose=payload.purpose,
            expires_at=expires_at,
        )
    )
    audit.record(
        "location_share.created",
        actor_id=user.id,
        target_type="location_share",
        target_id=share.id,
        recipient_id=recipient_id,
        precision=share.precision_level,
        purpose=share.purpose,
    )
    db.commit()
    return _share_response(share)


@router.get("/shares", response_model=list[LocationShareResponse])
async def list_location_shares(
    user: User = Depends(require_fresh_password),
    shares: LocationShareRepository = Depends(get_location_share_repository),
):
    """List the caller's own share records (the 'who can see me' dashboard)."""
    return [_share_response(share) for share in shares.list_owned_by(user.id)]


@router.get("/shares/{share_id}", response_model=LocationShareResponse)
async def get_location_share(
    share_id: int,
    user: User = Depends(require_fresh_password),
    shares: LocationShareRepository = Depends(get_location_share_repository),
):
    """Get a share's status (owner or recipient only)."""
    share = _load_share_parties(share_id, shares, user)
    return _share_response(share)


@router.delete("/shares/{share_id}")
async def revoke_location_share(
    share_id: int,
    user: User = Depends(require_fresh_password),
    shares: LocationShareRepository = Depends(get_location_share_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
):
    """Revoke an active share (owner only) — the recipient loses access at once."""
    share = _load_share_parties(share_id, shares, user)
    if share.owner_id != user.id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Only the owner can revoke a share."},
        )
    if not share.is_active:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "CONFLICT", "message": "Share is already revoked."},
        )

    shares.revoke(share, utcnow())
    audit.record(
        "location_share.revoked",
        actor_id=user.id,
        target_type="location_share",
        target_id=share.id,
        reason="owner_request",
    )
    db.commit()
    return {"message": "Location share revoked."}


# ---------------------------------------------------------------------------
# Location updates and reads
# ---------------------------------------------------------------------------

@router.post("/current")
async def update_current_location(
    payload: LocationUpdateRequest,
    user: User = Depends(require_fresh_password),
    updates: LocationUpdateRepository = Depends(get_location_update_repository),
    shares: LocationShareRepository = Depends(get_location_share_repository),
    db=Depends(get_db),
    _rate: None = Depends(rate_limit("location_update", 100, 60)),
):
    """
    Store the caller's latest location (upsert — no history is kept).

    The caller may always update their own position; whether anyone else may
    see it is decided per reader by LocationAccessPolicy at read time.
    """
    recorded_at = utcnow()
    if payload.timestamp is not None:
        given = ensure_utc(payload.timestamp)
        if given > recorded_at + timedelta(minutes=5):
            raise HTTPException(
                status_code=status.HTTP_400_BAD_REQUEST,
                detail={"code": "INVALID_REQUEST", "message": "Timestamp is in the future."},
            )
        recorded_at = given

    update = updates.upsert_latest(
        owner_id=user.id,
        latitude=payload.latitude,
        longitude=payload.longitude,
        accuracy=payload.accuracy,
        battery_level=payload.battery_level,
        recorded_at=recorded_at,
    )
    for share in shares.list_owned_by(user.id):
        if share.is_currently_valid():
            share.last_location_update_at = update.recorded_at
    db.commit()
    return {"message": "Location updated.", "recorded_at": update.recorded_at.isoformat()}


@router.get("/friends/latest", response_model=LatestLocationsResponse)
async def friends_latest_locations(
    user: User = Depends(require_fresh_password),
    shares: LocationShareRepository = Depends(get_location_share_repository),
    updates: LocationUpdateRepository = Depends(get_location_update_repository),
    relationships=Depends(get_relationship_service),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _rate: None = Depends(rate_limit("latest_locations", 60, 60)),
):
    """
    Latest locations of friends the caller is authorized to see.

    Per friend: complete mediation via LocationAccessPolicy (share validity,
    direction, friendship, blocks), then the share's precision reducer is
    applied *server-side* before the coordinate leaves the API. Stale points
    (> 30 min) are labelled so the UI can never present them as live.
    """
    policy = LocationAccessPolicy(relationships)
    now = utcnow()
    results: list[FriendLocationResponse] = []

    for share in shares.list_active_incoming_for(user.id):
        owner_id = share.owner_id
        if owner_id == user.id:
            continue  # never mediates access to your own location

        decision = policy.check(viewer_id=user.id, owner_id=owner_id, share=share, now=now)
        if not decision.allowed:
            continue  # revoked, expired, blocked, or direction mismatch

        update = updates.get_latest(owner_id)
        if update is None:
            continue

        reducer = get_precision_reducer(decision.precision_level)
        latitude, longitude = reducer.reduce(update.latitude, update.longitude)
        recorded_at = ensure_utc(update.recorded_at) or now

        results.append(
            FriendLocationResponse(
                friend_id=owner_id,
                friend_name=share.owner.full_name if share.owner else "",
                latitude=latitude,
                longitude=longitude,
                accuracy=update.accuracy,
                precision_level=reducer.label,
                recorded_at=recorded_at,
                is_stale=(now - recorded_at) > STALE_AFTER,
                battery_level=update.battery_level,
            )
        )

    # One privacy-preserving audit event per list read (ids + counts only).
    audit.record(
        "location.friends_viewed",
        actor_id=user.id,
        friends_returned=len(results),
    )
    db.commit()

    return LatestLocationsResponse(friends_locations=results)
