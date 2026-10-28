"""Location share and location update schemas."""

from datetime import datetime
from typing import Literal

from pydantic import Field, field_validator

from app.schemas.auth import normalise_username
from app.schemas.common import ApiResponse, StrictRequest
from app.schemas.social import PublicUser, _no_control_characters

Precision = Literal["exact", "approximate"]
MAX_SHARE_MINUTES = 7 * 24 * 60


class ShareCreate(StrictRequest):
    recipient_username: str = Field(min_length=3, max_length=30)
    precision: Precision = "approximate"  # privacy-preserving default
    purpose: str | None = Field(default=None, max_length=100)
    expires_in_minutes: int | None = Field(
        default=None,
        ge=5,
        le=MAX_SHARE_MINUTES,
        description="Omit to share until revoked.",
    )

    @field_validator("recipient_username")
    @classmethod
    def _username(cls, value: str) -> str:
        return normalise_username(value)

    @field_validator("purpose")
    @classmethod
    def _purpose(cls, value: str | None) -> str | None:
        return _no_control_characters(value)


class ShareUpdate(StrictRequest):
    precision: Precision | None = None
    purpose: str | None = Field(default=None, max_length=100)
    expires_in_minutes: int | None = Field(default=None, ge=5, le=MAX_SHARE_MINUTES)
    remove_expiry: bool = False

    @field_validator("purpose")
    @classmethod
    def _purpose(cls, value: str | None) -> str | None:
        return _no_control_characters(value)


class ShareOut(ApiResponse):
    id: str
    direction: Literal["outgoing", "incoming"]
    owner: PublicUser
    recipient: PublicUser
    precision: Precision
    purpose: str | None
    expires_at: datetime | None
    created_at: datetime
    revoked_at: datetime | None
    active: bool


class ShareList(ApiResponse):
    outgoing: list[ShareOut]
    incoming: list[ShareOut]


class LocationUpdate(StrictRequest):
    latitude: float = Field(ge=-90, le=90, allow_inf_nan=False)
    longitude: float = Field(ge=-180, le=180, allow_inf_nan=False)
    accuracy_m: float | None = Field(default=None, ge=0, le=100_000, allow_inf_nan=False)
    battery_pct: int | None = Field(default=None, ge=0, le=100)
    is_charging: bool | None = None
    recorded_at: datetime | None = Field(
        default=None, description="Device timestamp (UTC). Defaults to the time of receipt."
    )


Freshness = Literal["live", "stale", "dead", "unknown"]


class NavigationLinks(ApiResponse):
    """External navigation links; they carry only the displayed coordinate."""

    openstreetmap: str
    google_maps: str
    geo_uri: str


class OwnLocation(ApiResponse):
    latitude: float
    longitude: float
    accuracy_m: float | None
    battery_pct: int | None
    is_charging: bool | None
    recorded_at: datetime
    received_at: datetime
    freshness: Freshness
    age_seconds: int


class LocationUpdateResult(ApiResponse):
    applied: bool
    location: OwnLocation


class VisibleLocation(ApiResponse):
    share_id: str
    user: PublicUser
    precision: Precision
    share_expires_at: datetime | None
    freshness: Freshness
    latitude: float | None = None
    longitude: float | None = None
    accuracy_m: float | None = None
    recorded_at: datetime | None = None
    age_seconds: int | None = None
    last_battery_pct: int | None = None
    is_charging: bool | None = None
    low_battery: bool = False
    navigation: NavigationLinks | None = None
