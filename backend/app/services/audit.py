"""
Audit logging.

Design patterns applied:
- Observer: AuditLogger is the subject; handlers (database, structured log)
  subscribe and are notified of every recorded event. New sinks (metrics,
  SIEM shipping) can be added without touching calling code.
- Privacy by design: record() accepts IDs and whitelisted scalar context only
  — there is intentionally no parameter for emails, passwords, or raw
  coordinates (SC-08, GDPR data minimisation).
"""

import json
import logging
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Dict, Optional, Protocol

logger = logging.getLogger("audit")


@dataclass(frozen=True)
class AuditEventData:
    """A PII-free audit event."""

    event_type: str
    actor_id: Optional[int] = None
    target_type: Optional[str] = None
    target_id: Optional[str] = None
    context: Dict[str, Any] = field(default_factory=dict)
    created_at: datetime = field(default_factory=lambda: datetime.now(timezone.utc))


class AuditHandler(Protocol):
    """A sink for audit events."""

    def handle(self, event: AuditEventData) -> None: ...


class StructuredLogAuditHandler:
    """Writes each event as a single-line JSON record to the audit logger."""

    def handle(self, event: AuditEventData) -> None:
        logger.info(
            json.dumps(
                {
                    "event_type": event.event_type,
                    "actor_id": event.actor_id,
                    "target_type": event.target_type,
                    "target_id": event.target_id,
                    "context": event.context,
                    "timestamp": event.created_at.isoformat(),
                },
                separators=(",", ":"),
                sort_keys=True,
            )
        )


class DatabaseAuditHandler:
    """
    Persists events to the audit_events table.

    The handler only queues rows on the caller's session; the service layer
    owns the transaction boundary (commit/rollback).
    """

    def __init__(self, db):
        self._db = db

    def handle(self, event: AuditEventData) -> None:
        from app.models.audit import AuditEvent

        self._db.add(
            AuditEvent(
                event_type=event.event_type,
                actor_id=event.actor_id,
                target_type=event.target_type,
                target_id=event.target_id,
                context=event.context or None,
            )
        )


class AuditLogger:
    """The subject of the Observer pattern."""

    def __init__(self, handlers=None):
        self._handlers = list(handlers or [])

    def subscribe(self, handler: AuditHandler) -> None:
        self._handlers.append(handler)

    def unsubscribe(self, handler: AuditHandler) -> None:
        if handler in self._handlers:
            self._handlers.remove(handler)

    def record(
        self,
        event_type: str,
        actor_id: Optional[int] = None,
        target_type: Optional[str] = None,
        target_id: Optional[str] = None,
        **context: Any,
    ) -> AuditEventData:
        """
        Notify all handlers about an event.

        A failing handler must never break the business flow: audit delivery
        is best-effort per handler, with the failure itself logged.
        """
        event = AuditEventData(
            event_type=event_type,
            actor_id=actor_id,
            target_type=target_type,
            target_id=str(target_id) if target_id is not None else None,
            context=context,
        )
        for handler in self._handlers:
            try:
                handler.handle(event)
            except Exception as exc:  # pragma: no cover - defensive
                logger.error(
                    "Audit handler %s failed for %s: %s",
                    type(handler).__name__, event_type, exc,
                )
        return event


_null_audit_logger = AuditLogger()


def get_null_audit_logger() -> AuditLogger:
    """Audit logger with no handlers (used where no DB session exists)."""
    return _null_audit_logger
