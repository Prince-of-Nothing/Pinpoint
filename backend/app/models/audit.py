"""
Audit event model.

Privacy-preserving audit trail (SC-08): events reference users by immutable
integer ID and store only whitelisted, non-PII context keys (precision,
purpose, counts, reasons) — never emails, passwords, or raw coordinates of
people who did not consent to be logged.
"""

from sqlalchemy import JSON, Column, DateTime, Integer, String
from sqlalchemy.sql import func

from app.db import Base


class AuditEvent(Base):
    """Append-only security/privacy audit record."""

    __tablename__ = "audit_events"

    id = Column(Integer, primary_key=True, index=True)
    event_type = Column(String(64), nullable=False, index=True)
    actor_id = Column(Integer, nullable=True, index=True)  # who triggered it
    target_type = Column(String(32), nullable=True)  # e.g. "user", "location_share"
    target_id = Column(String(64), nullable=True)  # id of the affected object
    context = Column(JSON, nullable=True)  # whitelisted, PII-free metadata
    created_at = Column(DateTime(timezone=True), server_default=func.now(), nullable=False, index=True)

    def __repr__(self):
        return f"<AuditEvent(id={self.id}, type='{self.event_type}', actor={self.actor_id})>"
