"""Operator (administrator) schemas."""

from datetime import datetime

from app.schemas.common import ApiResponse


class AdminUserOut(ApiResponse):
    id: str
    username: str
    email: str
    role: str
    is_active: bool
    locked_until: datetime | None
    failed_login_attempts: int
    created_at: datetime
    last_login_at: datetime | None


class AuditEventOut(ApiResponse):
    id: int
    created_at: datetime
    event_type: str
    outcome: str
    actor_user_id: str | None
    target_type: str | None
    target_id: str | None
    ip_address: str | None
    details: dict | None
