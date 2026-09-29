"""Directional location sharing: the owner grants one recipient access to the owner's location."""

from datetime import timedelta

from fastapi import APIRouter, Request, Response, status
from sqlalchemy import or_, select

from app.api.deps import CurrentUser, DbSession
from app.core.errors import ProblemError, not_found
from app.core.timeutils import utcnow
from app.models import LocationShare
from app.schemas.locations import ShareCreate, ShareList, ShareOut, ShareUpdate
from app.schemas.social import PublicUser
from app.services import audit, ratelimit
from app.services.relationships import active_user_by_username, are_friends, is_blocked_either_way

router = APIRouter()


def _out(share: LocationShare, viewer_id: str) -> ShareOut:
    return ShareOut(
        id=share.id,
        direction="outgoing" if share.owner_id == viewer_id else "incoming",
        owner=PublicUser.model_validate(share.owner),
        recipient=PublicUser.model_validate(share.recipient),
        precision=share.precision,
        purpose=share.purpose,
        expires_at=share.expires_at,
        created_at=share.created_at,
        revoked_at=share.revoked_at,
        active=share.is_active(utcnow()),
    )


def _visible_share(db, share_id: str, user_id: str) -> LocationShare:
    share = db.get(LocationShare, share_id)
    if share is None or user_id not in (share.owner_id, share.recipient_id):
        raise not_found("Location share")
    return share


@router.get("", response_model=ShareList)
def list_shares(user: CurrentUser, db: DbSession) -> ShareList:
    now = utcnow()
    rows = db.scalars(
        select(LocationShare)
        .where(
            or_(LocationShare.owner_id == user.id, LocationShare.recipient_id == user.id),
            LocationShare.revoked_at.is_(None),
            or_(LocationShare.expires_at.is_(None), LocationShare.expires_at > now),
        )
        .order_by(LocationShare.created_at.desc())
    ).all()
    result = ShareList(outgoing=[], incoming=[])
    for row in rows:
        item = _out(row, user.id)
        (result.outgoing if item.direction == "outgoing" else result.incoming).append(item)
    return result


@router.post(
    "",
    response_model=ShareOut,
    status_code=status.HTTP_201_CREATED,
    dependencies=[ratelimit.rate_limit(ratelimit.SHARE_CHANGE)],
)
def create_share(body: ShareCreate, request: Request, user: CurrentUser, db: DbSession):
    recipient = active_user_by_username(db, body.recipient_username)
    if recipient is None or is_blocked_either_way(db, user.id, recipient.id):
        raise not_found("User")
    if recipient.id == user.id:
        raise ProblemError(400, "INVALID_TARGET", "You cannot share your location with yourself.")
    if not are_friends(db, user.id, recipient.id):
        raise ProblemError(403, "NOT_FRIENDS", "You can only share your location with friends.")

    now = utcnow()
    existing = db.scalar(
        select(LocationShare).where(
            LocationShare.owner_id == user.id,
            LocationShare.recipient_id == recipient.id,
            LocationShare.revoked_at.is_(None),
            or_(LocationShare.expires_at.is_(None), LocationShare.expires_at > now),
        )
    )
    if existing is not None:
        raise ProblemError(409, "SHARE_EXISTS", "You already share your location with this friend.")

    share = LocationShare(
        owner_id=user.id,
        recipient_id=recipient.id,
        precision=body.precision,
        purpose=body.purpose,
        expires_at=now + timedelta(minutes=body.expires_in_minutes)
        if body.expires_in_minutes
        else None,
    )
    db.add(share)
    db.flush()
    audit.record(
        db,
        "share.created",
        actor_id=user.id,
        target_type="location_share",
        target_id=share.id,
        request=request,
        recipient_id=recipient.id,
        precision=share.precision,
        expires=share.expires_at.isoformat() if share.expires_at else None,
    )
    db.commit()
    return _out(share, user.id)


@router.get("/{share_id}", response_model=ShareOut)
def get_share(share_id: str, user: CurrentUser, db: DbSession) -> ShareOut:
    return _out(_visible_share(db, share_id, user.id), user.id)


@router.patch(
    "/{share_id}",
    response_model=ShareOut,
    dependencies=[ratelimit.rate_limit(ratelimit.SHARE_CHANGE)],
)
def update_share(
    share_id: str, body: ShareUpdate, request: Request, user: CurrentUser, db: DbSession
):
    share = _visible_share(db, share_id, user.id)
    if share.owner_id != user.id:
        raise ProblemError(403, "FORBIDDEN", "Only the owner can change a location share.")
    now = utcnow()
    if not share.is_active(now):
        raise ProblemError(409, "SHARE_INACTIVE", "This share has ended; create a new one.")
    changes = body.model_dump(exclude_unset=True)
    if "precision" in changes and body.precision:
        share.precision = body.precision
    if "purpose" in changes:
        share.purpose = body.purpose
    if body.remove_expiry:
        share.expires_at = None
    elif body.expires_in_minutes:
        share.expires_at = now + timedelta(minutes=body.expires_in_minutes)
    audit.record(
        db,
        "share.updated",
        actor_id=user.id,
        target_type="location_share",
        target_id=share.id,
        request=request,
        fields=sorted(changes),
    )
    db.commit()
    return _out(share, user.id)


@router.delete(
    "/{share_id}",
    status_code=status.HTTP_204_NO_CONTENT,
    dependencies=[ratelimit.rate_limit(ratelimit.SHARE_CHANGE)],
)
def revoke_share(share_id: str, request: Request, user: CurrentUser, db: DbSession):
    """The owner stops sharing, or the recipient stops receiving. Effective immediately."""
    share = _visible_share(db, share_id, user.id)
    if share.revoked_at is None:
        share.revoked_at = utcnow()
        share.revoked_reason = "owner_revoked" if share.owner_id == user.id else "recipient_removed"
        audit.record(
            db,
            "share.revoked",
            actor_id=user.id,
            target_type="location_share",
            target_id=share.id,
            request=request,
            reason=share.revoked_reason,
        )
        db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
