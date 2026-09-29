"""Friendship, block and profile schemas."""

from datetime import datetime
from typing import Literal

from pydantic import Field, field_validator

from app.schemas.auth import normalise_username
from app.schemas.common import ApiResponse, StrictRequest


class UsernameRequest(StrictRequest):
    username: str = Field(min_length=3, max_length=30)

    @field_validator("username")
    @classmethod
    def _username(cls, value: str) -> str:
        return normalise_username(value)


class PublicUser(ApiResponse):
    """The only user fields ever shown to other people (never email or ids of blocks)."""

    id: str
    username: str
    display_name: str | None


class FriendshipOut(ApiResponse):
    id: str
    status: Literal["pending", "accepted"]
    direction: Literal["incoming", "outgoing"]
    user: PublicUser
    created_at: datetime
    responded_at: datetime | None


class FriendshipList(ApiResponse):
    friends: list[FriendshipOut]
    incoming: list[FriendshipOut]
    outgoing: list[FriendshipOut]


class BlockOut(ApiResponse):
    user: PublicUser
    created_at: datetime


def _no_control_characters(value: str | None) -> str | None:
    if value is not None and any(ord(ch) < 32 or ord(ch) == 127 for ch in value):
        raise ValueError("Control characters are not allowed.")
    return value


class ProfileUpdate(StrictRequest):
    display_name: str | None = Field(default=None, min_length=1, max_length=50)
    bio: str | None = Field(default=None, max_length=160)

    _clean = field_validator("display_name", "bio")(
        classmethod(lambda cls, v: _no_control_characters(v))
    )


class ProfileView(ApiResponse):
    """
    Viewer-dependent profile. Non-friends (including blocked users in either
    direction) receive the empty form: username only.
    """

    username: str
    relationship: Literal["self", "friend", "none"]
    display_name: str | None = None
    bio: str | None = None
    friends_since: datetime | None = None
