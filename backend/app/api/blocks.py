"""Blocking: ends the friendship, revokes shares both ways and hides each user from the other."""

from fastapi import APIRouter, Request, Response, status
from sqlalchemy import select

from app.api.deps import CurrentUser, DbSession
from app.core.errors import ProblemError, not_found
from app.models import Block
from app.schemas.social import BlockOut, PublicUser, UsernameRequest
from app.services import audit
from app.services.relationships import (
    active_user_by_username,
    friendship_between,
    revoke_shares_between,
)

router = APIRouter()


@router.get("", response_model=list[BlockOut])
def list_blocks(user: CurrentUser, db: DbSession) -> list[BlockOut]:
    rows = db.scalars(
        select(Block).where(Block.blocker_id == user.id).order_by(Block.created_at.desc())
    ).all()
    return [
        BlockOut(user=PublicUser.model_validate(row.blocked), created_at=row.created_at)
        for row in rows
    ]


@router.post("", response_model=BlockOut, status_code=status.HTTP_201_CREATED)
def block_user(body: UsernameRequest, request: Request, user: CurrentUser, db: DbSession):
    target = active_user_by_username(db, body.username)
    if target is None:
        raise not_found("User")
    if target.id == user.id:
        raise ProblemError(400, "INVALID_TARGET", "You cannot block yourself.")
    existing = db.scalar(
        select(Block).where(Block.blocker_id == user.id, Block.blocked_id == target.id)
    )
    if existing:
        raise ProblemError(409, "ALREADY_BLOCKED", "This user is already blocked.")

    friendship = friendship_between(db, user.id, target.id)
    if friendship is not None:
        db.delete(friendship)
    revoked = revoke_shares_between(db, user.id, target.id, "blocked")
    block = Block(blocker_id=user.id, blocked_id=target.id)
    db.add(block)
    db.flush()
    audit.record(
        db,
        "block.created",
        actor_id=user.id,
        target_type="user",
        target_id=target.id,
        request=request,
        shares_revoked=revoked,
        friendship_removed=friendship is not None,
    )
    db.commit()
    return BlockOut(user=PublicUser.model_validate(target), created_at=block.created_at)


@router.delete("/{username}", status_code=status.HTTP_204_NO_CONTENT)
def unblock_user(username: str, request: Request, user: CurrentUser, db: DbSession):
    target = active_user_by_username(db, username)
    block = (
        db.scalar(select(Block).where(Block.blocker_id == user.id, Block.blocked_id == target.id))
        if target
        else None
    )
    if block is None:
        raise not_found("Block")
    db.delete(block)
    audit.record(
        db,
        "block.removed",
        actor_id=user.id,
        target_type="user",
        target_id=target.id,
        request=request,
    )
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)
