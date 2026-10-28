"""
Operator endpoints, restricted to the ``admin`` role (RBAC).

Least privilege: administrators can manage accounts and read the audit
trail, but no endpoint lets them read anyone's location or location shares.
"""

from fastapi import APIRouter, Query, Request
from sqlalchemy import select

from app.api.deps import AdminUser, DbSession
from app.core.errors import ProblemError, not_found
from app.models import AuditEvent, User
from app.schemas.admin import AdminUserOut, AuditEventOut
from app.services import audit
from app.services.sessions import revoke_all_for_user

router = APIRouter()


def _target(db, user_id: str) -> User:
    user = db.get(User, user_id)
    if user is None:
        raise not_found("User")
    return user


@router.get("/users", response_model=list[AdminUserOut])
def list_users(
    admin: AdminUser,
    db: DbSession,
    limit: int = Query(default=50, ge=1, le=100),
    offset: int = Query(default=0, ge=0, le=100_000),
) -> list[User]:
    return list(db.scalars(select(User).order_by(User.created_at).limit(limit).offset(offset)))


@router.post("/users/{user_id}/unlock", response_model=AdminUserOut)
def unlock_user(user_id: str, request: Request, admin: AdminUser, db: DbSession) -> User:
    user = _target(db, user_id)
    user.locked_until = None
    user.failed_login_attempts = 0
    audit.record(
        db,
        "admin.user_unlocked",
        actor_id=admin.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    return user


@router.post("/users/{user_id}/deactivate", response_model=AdminUserOut)
def deactivate_user(user_id: str, request: Request, admin: AdminUser, db: DbSession) -> User:
    user = _target(db, user_id)
    if user.id == admin.id:
        raise ProblemError(400, "INVALID_TARGET", "Administrators cannot deactivate themselves.")
    user.is_active = False
    revoke_all_for_user(db, user.id, "deactivated")
    audit.record(
        db,
        "admin.user_deactivated",
        actor_id=admin.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    return user


@router.post("/users/{user_id}/activate", response_model=AdminUserOut)
def activate_user(user_id: str, request: Request, admin: AdminUser, db: DbSession) -> User:
    user = _target(db, user_id)
    user.is_active = True
    audit.record(
        db,
        "admin.user_activated",
        actor_id=admin.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    return user


@router.get("/audit-events", response_model=list[AuditEventOut])
def list_audit_events(
    admin: AdminUser,
    db: DbSession,
    event_type: str | None = Query(default=None, max_length=64, pattern=r"^[a-z_.]+$"),
    limit: int = Query(default=100, ge=1, le=500),
    offset: int = Query(default=0, ge=0, le=1_000_000),
) -> list[AuditEvent]:
    query = select(AuditEvent).order_by(AuditEvent.id.desc())
    if event_type:
        query = query.where(AuditEvent.event_type == event_type)
    return list(db.scalars(query.limit(limit).offset(offset)))
