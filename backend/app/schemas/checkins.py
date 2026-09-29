"""Check-in schemas."""

from datetime import datetime
from typing import Literal

from pydantic import Field, field_validator

from app.schemas.common import ApiResponse, StrictRequest
from app.schemas.social import PublicUser, _no_control_characters

CheckInState = Literal["pending", "due_soon", "overdue", "missed", "completed", "cancelled"]


class CheckInCreate(StrictRequest):
    due_in_minutes: int = Field(ge=5, le=24 * 60)
    grace_minutes: int = Field(default=15, ge=0, le=120)
    note: str | None = Field(default=None, max_length=140)

    @field_validator("note")
    @classmethod
    def _note(cls, value: str | None) -> str | None:
        return _no_control_characters(value)


class CheckInOut(ApiResponse):
    id: str
    note: str | None
    due_at: datetime
    grace_minutes: int
    state: CheckInState
    created_at: datetime
    completed_at: datetime | None
    cancelled_at: datetime | None


class SharedCheckIn(CheckInOut):
    user: PublicUser
