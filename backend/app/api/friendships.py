"""
Friendship API endpoints.

Security behaviour:
- Rate limited (10 requests/hour, per the API security guide).
- Only the receiver can accept/reject; only the requester can revoke —
  ownership violations on existing resources return 403 (IDOR attempt),
  unknown IDs return 404.
- Blocking revokes every location share between the pair immediately.
"""

from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, status

from app.api.deps import (
    csrf_protect,
    get_audit_logger,
    get_friendship_repository,
    get_location_share_repository,
    get_user_repository,
    rate_limit,
    require_fresh_password,
)
from app.db.session import get_db
from app.models.user import Friendship, FriendshipStatus, User
from app.repositories import FriendshipRepository, LocationShareRepository, UserRepository
from app.schemas.auth import FriendshipRequest, FriendshipResponse
from app.services.audit import AuditLogger
from app.services.security import normalize_email

router = APIRouter()

FRIENDSHIP_RATE_LIMIT = dict(limit=10, window_seconds=3600)


def _friendship_response(friendship: Friendship, requester_name: str,
                         receiver_name: str) -> FriendshipResponse:
    return FriendshipResponse(
        id=friendship.id,
        requester_id=friendship.requester_id,
        requester_name=requester_name,
        receiver_id=friendship.receiver_id,
        receiver_name=receiver_name,
        status=friendship.status,
        created_at=friendship.created_at,
        responded_at=friendship.responded_at,
    )


def _load_owned_friendship(friendships: FriendshipRepository, friendship_id: int,
                           user: User, must_be_receiver: bool = False) -> Friendship:
    """Fetch a friendship and enforce caller participation (anti-IDOR)."""
    friendship = friendships.get_by_id(friendship_id)
    if friendship is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "NOT_FOUND", "message": "Friendship not found."},
        )

    participant = (
        friendship.receiver_id == user.id if must_be_receiver
        else user.id in (friendship.requester_id, friendship.receiver_id)
    )
    if not participant:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Authorization denied."},
        )
    return friendship


@router.post("/", response_model=FriendshipResponse, status_code=status.HTTP_201_CREATED)
async def send_friend_request(
    payload: FriendshipRequest,
    user: User = Depends(require_fresh_password),
    users: UserRepository = Depends(get_user_repository),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
    _rate: None = Depends(rate_limit("friendship_create", **FRIENDSHIP_RATE_LIMIT)),
):
    """Send a friendship request to another user (by email)."""
    receiver = users.get_by_email_normalized(normalize_email(payload.receiver_email))
    if receiver is None or not receiver.is_active:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "NOT_FOUND", "message": "User not found."},
        )

    if receiver.id == user.id:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={"code": "INVALID_REQUEST", "message": "Cannot send a request to yourself."},
        )

    existing = friendships.find_between(user.id, receiver.id)
    if existing is not None:
        if existing.status == FriendshipStatus.BLOCKED:
            # Same generic 403 for "blocked" and "not allowed": no state leak.
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail={"code": "FORBIDDEN", "message": "Authorization denied."},
            )
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "CONFLICT", "message": "A friendship already exists or is pending."},
        )

    friendship = friendships.add(
        Friendship(
            requester_id=user.id,
            receiver_id=receiver.id,
            status=FriendshipStatus.PENDING,
        )
    )
    audit.record(
        "friendship.requested",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
    )
    db.commit()

    return _friendship_response(friendship, user.full_name, receiver.full_name)


@router.get("/", response_model=list[FriendshipResponse])
async def list_friendships(
    user: User = Depends(require_fresh_password),
    users: UserRepository = Depends(get_user_repository),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
):
    """List the caller's friendships (pending, accepted, blocked)."""
    responses = []
    for friendship in friendships.list_for_user(user.id):
        requester = users.get_by_id(friendship.requester_id)
        receiver = users.get_by_id(friendship.receiver_id)
        responses.append(
            _friendship_response(
                friendship,
                requester.full_name if requester else "",
                receiver.full_name if receiver else "",
            )
        )
    return responses


@router.post("/{friendship_id}/accept", response_model=FriendshipResponse)
async def accept_friendship(
    friendship_id: int,
    user: User = Depends(require_fresh_password),
    users: UserRepository = Depends(get_user_repository),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
):
    """Accept a pending request (receiver only)."""
    friendship = _load_owned_friendship(friendships, friendship_id, user, must_be_receiver=True)
    if friendship.status != FriendshipStatus.PENDING:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "CONFLICT", "message": "Friendship request is not pending."},
        )

    friendship.status = FriendshipStatus.ACCEPTED
    friendship.responded_at = datetime.now(timezone.utc)
    audit.record(
        "friendship.accepted",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
    )
    db.commit()

    requester = users.get_by_id(friendship.requester_id)
    receiver = users.get_by_id(friendship.receiver_id)
    return _friendship_response(
        friendship,
        requester.full_name if requester else "",
        receiver.full_name if receiver else "",
    )


@router.post("/{friendship_id}/reject", response_model=FriendshipResponse)
async def reject_friendship(
    friendship_id: int,
    user: User = Depends(require_fresh_password),
    users: UserRepository = Depends(get_user_repository),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
):
    """Reject a pending request (receiver only)."""
    friendship = _load_owned_friendship(friendships, friendship_id, user, must_be_receiver=True)
    if friendship.status != FriendshipStatus.PENDING:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "CONFLICT", "message": "Friendship request is not pending."},
        )

    friendship.status = FriendshipStatus.REJECTED
    friendship.responded_at = datetime.now(timezone.utc)
    audit.record(
        "friendship.rejected",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship.id,
    )
    db.commit()

    requester = users.get_by_id(friendship.requester_id)
    receiver = users.get_by_id(friendship.receiver_id)
    return _friendship_response(
        friendship,
        requester.full_name if requester else "",
        receiver.full_name if receiver else "",
    )


@router.delete("/{friendship_id}")
async def revoke_friendship_request(
    friendship_id: int,
    user: User = Depends(require_fresh_password),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
):
    """Revoke a sent request (requester only)."""
    friendship = _load_owned_friendship(friendships, friendship_id, user)
    if friendship.requester_id != user.id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "FORBIDDEN", "message": "Only the requester can revoke a request."},
        )
    if friendship.status != FriendshipStatus.PENDING:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail={"code": "CONFLICT", "message": "Only pending requests can be revoked."},
        )

    db.delete(friendship)
    audit.record(
        "friendship.request_revoked",
        actor_id=user.id,
        target_type="friendship",
        target_id=friendship_id,
    )
    db.commit()
    return {"message": "Friendship request revoked."}


@router.post("/{friendship_id}/block", response_model=FriendshipResponse)
async def block_user(
    friendship_id: int,
    user: User = Depends(require_fresh_password),
    users: UserRepository = Depends(get_user_repository),
    friendships: FriendshipRepository = Depends(get_friendship_repository),
    shares: LocationShareRepository = Depends(get_location_share_repository),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _csrf: None = Depends(csrf_protect),
):
    """
    Block the counterparty of a friendship.

    Blocks win over everything: the row becomes BLOCKED and every location
    share between the two users is revoked immediately.
    """
    friendship = _load_owned_friendship(friendships, friendship_id, user)
    counterparty_id = friendship.other_party(user.id)

    friendship.status = FriendshipStatus.BLOCKED
    friendship.responded_at = datetime.now(timezone.utc)
    revoked_shares = shares.revoke_between(user.id, counterparty_id, datetime.now(timezone.utc))

    audit.record(
        "user.blocked",
        actor_id=user.id,
        target_type="user",
        target_id=counterparty_id,
        shares_revoked=revoked_shares,
    )
    db.commit()

    requester = users.get_by_id(friendship.requester_id)
    receiver = users.get_by_id(friendship.receiver_id)
    return _friendship_response(
        friendship,
        requester.full_name if requester else "",
        receiver.full_name if receiver else "",
    )
