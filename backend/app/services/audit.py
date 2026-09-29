"""Security audit events. Callers commit the surrounding transaction."""

import logging

from fastapi import Request
from sqlalchemy.orm import Session

from app.models import AuditEvent

logger = logging.getLogger("app.audit")


def client_ip(request: Request | None) -> str | None:
    if request is None or request.client is None:
        return None
    return request.client.host[:45]


def record(
    db: Session,
    event_type: str,
    *,
    outcome: str = "success",
    actor_id: str | None = None,
    target_type: str | None = None,
    target_id: str | None = None,
    request: Request | None = None,
    **details,
) -> AuditEvent:
    """Add an audit event. ``details`` must never contain secrets or coordinates."""
    event = AuditEvent(
        event_type=event_type,
        outcome=outcome,
        actor_user_id=actor_id,
        target_type=target_type,
        target_id=target_id,
        ip_address=client_ip(request),
        details=details or None,
    )
    db.add(event)
    logger.info(
        "audit event=%s outcome=%s actor=%s target=%s:%s",
        event_type,
        outcome,
        actor_id,
        target_type,
        target_id,
    )
    return event
