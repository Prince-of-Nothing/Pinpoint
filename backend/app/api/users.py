"""
User/profile API endpoints.

SC-03 (private by default): every profile read goes through
ProfileVisibilityPolicy — non-friends receive a redacted, indistinguishable
projection, and the response is filtered by the backend, never the client.
"""

from fastapi import APIRouter, Depends, HTTPException, status

from app.api.deps import (
    get_audit_logger,
    get_current_user,
    get_relationship_service,
    get_user_repository,
    rate_limit,
)
from app.db.session import get_db
from app.models.user import User
from app.repositories import UserRepository
from app.schemas.auth import ProfileResponse, UserResponse
from app.services.audit import AuditLogger
from app.services.authorization import ProfileVisibilityPolicy, RelationshipService

router = APIRouter()


def _user_response(user: User) -> UserResponse:
    return UserResponse(
        id=user.id,
        email=user.email,
        full_name=user.full_name,
        role=user.role,
        is_active=user.is_active,
        must_change_password=user.must_change_password,
        created_at=user.created_at,
        last_login_at=user.last_login_at,
    )


@router.get("/me", response_model=UserResponse)
async def get_me(user: User = Depends(get_current_user)):
    """Get the authenticated user's own profile."""
    return _user_response(user)


@router.get("/{user_id}", response_model=ProfileResponse)
async def get_profile(
    user_id: int,
    viewer: User = Depends(get_current_user),
    users: UserRepository = Depends(get_user_repository),
    relationships: RelationshipService = Depends(get_relationship_service),
    audit: AuditLogger = Depends(get_audit_logger),
    db=Depends(get_db),
    _rate: None = Depends(rate_limit("profile_view", 100, 60)),
):
    """
    Get a user profile filtered by the viewer relationship.

    - self: full profile
    - friend: display name only
    - anyone else (pending/blocked/stranger): redacted shell — identical to
      the response for a non-existent account, so user IDs cannot be probed.
    """
    target = users.get_by_id(user_id)
    relationship = relationships.resolve(viewer.id, user_id)
    visible = ProfileVisibilityPolicy().visible_profile(viewer.id, target, relationship)

    if viewer.id != user_id:
        # Privacy-preserving audit: ids and relationship only, no PII.
        audit.record(
            "profile.viewed",
            actor_id=viewer.id,
            target_type="user",
            target_id=user_id,
            relationship=relationship.value,
            revealed=visible.is_full,
        )
        db.commit()

    return ProfileResponse(
        id=visible.user_id,
        display_name=visible.display_name,
        is_full_profile=visible.is_full,
    )
