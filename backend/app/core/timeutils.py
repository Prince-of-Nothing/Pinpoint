"""Time helpers shared across services (timezone-safe comparisons)."""

from datetime import datetime, timezone


def utcnow() -> datetime:
    """Current UTC time with tzinfo."""
    return datetime.now(timezone.utc)


def ensure_utc(value: datetime | None) -> datetime | None:
    """
    Normalize a datetime to UTC-aware.

    SQLite returns naive datetimes even for timezone-aware columns; comparing
    them with aware values raises TypeError, so every persisted timestamp is
    normalised at the boundary.
    """
    if value is None:
        return None
    if value.tzinfo is None:
        return value.replace(tzinfo=timezone.utc)
    return value.astimezone(timezone.utc)
