"""Mutual friendships. Being friends never grants access to anyone's location."""

from fastapi import APIRouter, Request, Response, status
from sqlalchemy import or_, select

from app.api.deps import CurrentUser, DbSession
from app.core.errors import ProblemError, not_found
from app.core.timeutils import utcnow
from app.models import Friendship, FriendshipStatus, pair_key
from app.schemas.social import FriendshipList, FriendshipOut, PublicUser, UsernameRequest
from app.services import audit, ratelimit
from app.services.relationships import (
    active_user_by_username,
    friendship_between,
    is_blocked_either_way,
    revoke_shares_between,
)

router = APIRouter()


def _out(friendship: Friendship, viewer_id: str) -> FriendshipOut:
    return FriendshipOut(
        id=friendship.id,
        status=friendship.status,
        direction="outgoing" if friendship.requester_id == viewer_id else "incoming",
        user=PublicUser.model_validate(friendship.other_party(viewer_id)),
        created_at=friendship.created_at,
        responded_at=friendship.responded_at,
    )


def _own_friendship(db, friendship_id: str, user_id: str) -> Friendship:
    friendship = db.get(Friendship, friendship_id)
    # Non-parties get the same 404 as a missing record (no IDOR oracle)
    if friendship is None or user_id not in (friendship.requester_id, friendship.addressee_id):
        raise not_found("Friendship")
    return friendship


@router.get("", response_model=FriendshipList)
def list_friendships(user: CurrentUser, db: DbSession) -> FriendshipList:
    rows = db.scalars(
        select(Friendship)
        .where(or_(Friendship.requester_id == user.id, Friendship.addressee_id == user.id))
        .order_by(Friendship.created_at.desc())
    ).all()
    result = FriendshipList(friends=[], incoming=[], outgoing=[])
    for row in rows:
        item = _out(row, user.id)
        if row.status == FriendshipStatus.ACCEPTED.value:
            result.friends.append(item)
        elif item.direction == "incoming":
            result.incoming.append(item)
        else:
            result.outgoing.append(item)
    return result


@router.post(
    "",
    response_model=FriendshipOut,
    status_code=status.HTTP_201_CREATED,
    dependencies=[ratelimit.rate_limit(ratelimit.FRIEND_REQUEST)],
)
def send_request(body: UsernameRequest, request: Request, user: CurrentUser, db: DbSession):
    target = active_user_by_username(db, body.username)
    if target is not None and target.id == user.id:
        raise ProblemError(400, "INVALID_TARGET", "You cannot send a friend request to yourself.")
    # Blocked users look exactly like non-existent ones
    if target is None or is_blocked_either_way(db, user.id, target.id):
        raise not_found("User")
    existing = friendship_between(db, user.id, target.id)
    if existing is not None:
        raise ProblemError(
            409, "FRIENDSHIP_EXISTS", "A friendship or pending request already exists."
        )

    friendship = Friendship(
        requester_id=user.id, addressee_id=target.id, pair_key=pair_key(user.id, target.id)
    )
    db.add(friendship)
    db.flush()
    audit.record(
        db,
        "friendship.requested",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
        request=request,
    )
    db.commit()
    return _out(friendship, user.id)


@router.post("/{friendship_id}/accept", response_model=FriendshipOut)
def accept(friendship_id: str, request: Request, user: CurrentUser, db: DbSession):
    friendship = _own_friendship(db, friendship_id, user.id)
    if friendship.addressee_id != user.id:
        raise ProblemError(403, "FORBIDDEN", "Only the recipient can accept a friend request.")
    if friendship.status != FriendshipStatus.PENDING.value:
        raise ProblemError(409, "NOT_PENDING", "This request is no longer pending.")
    friendship.status = FriendshipStatus.ACCEPTED.value
    friendship.responded_at = utcnow()
    audit.record(
        db,
        "friendship.accepted",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
        request=request,
    )
    db.commit()
    return _out(friendship, user.id)


@router.post("/{friendship_id}/decline", status_code=status.HTTP_204_NO_CONTENT)
def decline(friendship_id: str, request: Request, user: CurrentUser, db: DbSession):
    friendship = _own_friendship(db, friendship_id, user.id)
    if friendship.addressee_id != user.id:
        raise ProblemError(403, "FORBIDDEN", "Only the recipient can decline a friend request.")
    if friendship.status != FriendshipStatus.PENDING.value:
        raise ProblemError(409, "NOT_PENDING", "This request is no longer pending.")
    db.delete(friendship)
    audit.record(
        db,
        "friendship.declined",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
        request=request,
    )
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)


@router.delete("/{friendship_id}", status_code=status.HTTP_204_NO_CONTENT)
def remove(friendship_id: str, request: Request, user: CurrentUser, db: DbSession):
    """Cancel an outgoing request or end a friendship. Location shares end with it."""
    friendship = _own_friendship(db, friendship_id, user.id)
    was_pending = friendship.status == FriendshipStatus.PENDING.value
    if was_pending and friendship.requester_id != user.id:
        raise ProblemError(
            409, "USE_DECLINE", "Decline an incoming request instead of deleting it."
        )
    revoked = revoke_shares_between(
        db, friendship.requester_id, friendship.addressee_id, "friendship_ended"
    )
    db.delete(friendship)
    audit.record(
        db,
        "friendship.cancelled" if was_pending else "friendship.removed",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
        request=request,
        shares_revoked=revoked,
    )
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
