"""Own account and viewer-filtered profiles (private by default)."""

from fastapi import APIRouter, Request

from app.api.deps import CurrentUser, DbSession
from app.core.errors import not_found
from app.models import FriendshipStatus, User
from app.schemas.auth import UserSelf
from app.schemas.social import ProfileUpdate, ProfileView
from app.services import audit, ratelimit
from app.services.relationships import (
    active_user_by_username,
    friendship_between,
    is_blocked_either_way,
)

router = APIRouter()


@router.get("/me", response_model=UserSelf)
def get_me(user: CurrentUser) -> User:
    return user


@router.patch("/me", response_model=UserSelf)
def update_me(body: ProfileUpdate, request: Request, user: CurrentUser, db: DbSession) -> User:
    changes = body.model_dump(exclude_unset=True)
    for field, value in changes.items():
        setattr(user, field, value)
    audit.record(
        db,
        "profile.updated",
        actor_id=user.id,
        target_type="user",
        target_id=user.id,
        request=request,
        fields=sorted(changes),
    )
    db.commit()
    return user


@router.get(
    "/{username}",
    response_model=ProfileView,
    dependencies=[ratelimit.rate_limit(ratelimit.USER_LOOKUP)],
)
def get_profile(username: str, viewer: CurrentUser, db: DbSession) -> ProfileView:
    """
    Response filtering happens here on the server, not in the client:
    self -> full profile, accepted friend -> profile, anyone else -> empty.
    A block (either direction) is indistinguishable from "not a friend".
    """
    target = active_user_by_username(db, username)
    if target is None:
        raise not_found("User")
    if target.id == viewer.id:
        return ProfileView(
            username=target.username,
            relationship="self",
            display_name=target.display_name,
            bio=target.bio,
        )
    friendship = friendship_between(db, viewer.id, target.id)
    if (
        friendship is not None
        and friendship.status == FriendshipStatus.ACCEPTED.value
        and not is_blocked_either_way(db, viewer.id, target.id)
    ):
        return ProfileView(
            username=target.username,
            relationship="friend",
            display_name=target.display_name,
            bio=target.bio,
            friends_since=friendship.responded_at,
        )
    return ProfileView(username=target.username, relationship="none")
