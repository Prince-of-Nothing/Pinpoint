"""Declarative base and portable column types."""

from datetime import datetime

from sqlalchemy import DateTime
from sqlalchemy.orm import DeclarativeBase
from sqlalchemy.types import TypeDecorator

from app.core.timeutils import as_utc


class UTCDateTime(TypeDecorator):
    """
    Timezone-aware datetime column that always returns aware UTC values.

    PostgreSQL keeps the zone natively; SQLite (used in tests) drops it, so
    values are normalised on the way in and out.
    """

    impl = DateTime(timezone=True)
    cache_ok = True

    def process_bind_param(self, value: datetime | None, dialect):
        return as_utc(value)

    def process_result_value(self, value: datetime | None, dialect):
        return as_utc(value)


class Base(DeclarativeBase):
    pass
