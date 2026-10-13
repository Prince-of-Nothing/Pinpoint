"""Shared column helpers."""

import uuid
from datetime import datetime

from sqlalchemy import String
from sqlalchemy.orm import Mapped, mapped_column

from app.core.timeutils import utcnow
from app.db.base import UTCDateTime


def new_id() -> str:
    """Random, non-enumerable identifier used for every externally visible record."""
    return str(uuid.uuid4())


def id_column() -> Mapped[str]:
    return mapped_column(String(36), primary_key=True, default=new_id)


def created_at_column() -> Mapped[datetime]:
    return mapped_column(UTCDateTime, default=utcnow, nullable=False)
