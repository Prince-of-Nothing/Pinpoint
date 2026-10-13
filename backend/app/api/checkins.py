"""
Safety check-ins: "I will confirm I'm OK by <time>". States are derived from
the clock on every read (pending -> due_soon -> overdue during the grace
period -> missed), so a missed check-in is visible without a background job.
Friends who currently receive the owner's location can see the owner's
check-ins; nobody else can.
"""

from datetime import timedelta

from fastapi import APIRouter, Request
from sqlalchemy import func, or_, select

from app.api.deps import CurrentUser, DbSession, deny_foreign
from app.core.errors import ProblemError, not_found
from app.core.timeutils import utcnow
from app.models import CheckIn, CheckInStatus, LocationShare, User
from app.schemas.checkins import CheckInCreate, CheckInOut, SharedCheckIn
from app.schemas.social import PublicUser
from app.services import audit
from app.services.relationships import are_friends, is_blocked_either_way

router = APIRouter()

MAX_ACTIVE = 5
DUE_SOON = timedelta(minutes=10)
HISTORY_WINDOW = timedelta(hours=24)


def derive_state(check_in: CheckIn, now) -> str:
    if check_in.status == CheckInStatus.COMPLETED.value:
        return "completed"
    if check_in.status == CheckInStatus.CANCELLED.value:
        return "cancelled"
    if now < check_in.due_at - DUE_SOON:
        return "pending"
    if now <= check_in.due_at:
        return "due_soon"
    if now <= check_in.due_at + timedelta(minutes=check_in.grace_minutes):
        return "overdue"
    return "missed"


def _out(check_in: CheckIn, now) -> CheckInOut:
    return CheckInOut(
        id=check_in.id,
        note=check_in.note,
        due_at=check_in.due_at,
        grace_minutes=check_in.grace_minutes,
        state=derive_state(check_in, now),
        created_at=check_in.created_at,
        completed_at=check_in.completed_at,
        cancelled_at=check_in.cancelled_at,
    )


def _recent_filter(now):
    return or_(
        CheckIn.status == CheckInStatus.ACTIVE.value,
        CheckIn.completed_at > now - HISTORY_WINDOW,
        CheckIn.cancelled_at > now - HISTORY_WINDOW,
    )


@router.get("", response_model=list[CheckInOut])
def list_my_check_ins(user: CurrentUser, db: DbSession) -> list[CheckInOut]:
    now = utcnow()
    rows = db.scalars(
        select(CheckIn)
        .where(CheckIn.user_id == user.id, _recent_filter(now))
        .order_by(CheckIn.due_at)
    ).all()
    return [_out(row, now) for row in rows]


@router.post("", response_model=CheckInOut, status_code=201)
def create_check_in(body: CheckInCreate, request: Request, user: CurrentUser, db: DbSession):
    active = db.scalar(
        select(func.count())
        .select_from(CheckIn)
        .where(CheckIn.user_id == user.id, CheckIn.status == CheckInStatus.ACTIVE.value)
    )
    if active >= MAX_ACTIVE:
        raise ProblemError(409, "TOO_MANY_CHECK_INS", f"At most {MAX_ACTIVE} active check-ins.")
    now = utcnow()
    check_in = CheckIn(
        user_id=user.id,
        note=body.note,
        due_at=now + timedelta(minutes=body.due_in_minutes),
        grace_minutes=body.grace_minutes,
    )
    db.add(check_in)
    db.flush()
    audit.record(
        db,
        "checkin.created",
        actor_id=user.id,
        target_type="check_in",
        target_id=check_in.id,
        request=request,
    )
    db.commit()
    return _out(check_in, now)


def _own_active(db, request: Request, check_in_id: str, user_id: str) -> CheckIn:
    check_in = db.get(CheckIn, check_in_id)
    if check_in is None:
        raise not_found("Check-in")
    if check_in.user_id != user_id:
        raise deny_foreign(db, request, user_id, "check_in", check_in_id, "Check-in")
    if check_in.status != CheckInStatus.ACTIVE.value:
        raise ProblemError(409, "CHECK_IN_CLOSED", "This check-in is already closed.")
    return check_in


@router.post("/{check_in_id}/complete", response_model=CheckInOut)
def complete_check_in(check_in_id: str, request: Request, user: CurrentUser, db: DbSession):
    check_in = _own_active(db, request, check_in_id, user.id)
    now = utcnow()
    late = derive_state(check_in, now) in ("overdue", "missed")
    check_in.status = CheckInStatus.COMPLETED.value
    check_in.completed_at = now
    audit.record(
        db,
        "checkin.completed",
        actor_id=user.id,
        target_type="check_in",
        target_id=check_in.id,
        request=request,
        late=late,
    )
    db.commit()
    return _out(check_in, now)


@router.post("/{check_in_id}/cancel", response_model=CheckInOut)
def cancel_check_in(check_in_id: str, request: Request, user: CurrentUser, db: DbSession):
    check_in = _own_active(db, request, check_in_id, user.id)
    now = utcnow()
    check_in.status = CheckInStatus.CANCELLED.value
    check_in.cancelled_at = now
    audit.record(
        db,
        "checkin.cancelled",
        actor_id=user.id,
        target_type="check_in",
        target_id=check_in.id,
        request=request,
    )
    db.commit()
    return _out(check_in, now)


@router.get("/shared", response_model=list[SharedCheckIn])
def shared_check_ins(user: CurrentUser, db: DbSession) -> list[SharedCheckIn]:
    """Check-ins of people who currently share their location with the caller."""
    now = utcnow()
    owner_ids = db.scalars(
        select(LocationShare.owner_id).where(
            LocationShare.recipient_id == user.id,
            LocationShare.revoked_at.is_(None),
            or_(LocationShare.expires_at.is_(None), LocationShare.expires_at > now),
        )
    ).all()
    allowed = [
        owner
        for owner in set(owner_ids)
        if are_friends(db, owner, user.id) and not is_blocked_either_way(db, owner, user.id)
    ]
    if not allowed:
        return []
    rows = db.scalars(
        select(CheckIn)
        .where(CheckIn.user_id.in_(allowed), _recent_filter(now))
        .order_by(CheckIn.due_at)
    ).all()
    result = []
    for row in rows:
        owner = db.get(User, row.user_id)
        result.append(
            SharedCheckIn(**_out(row, now).model_dump(), user=PublicUser.model_validate(owner))
        )
    return result
