"""Location presentation rules: precision reduction, freshness and navigation links."""

import math
from datetime import datetime

from app.core.config import get_settings
from app.core.timeutils import utcnow

METRES_PER_DEGREE_LAT = 111_320
LOW_BATTERY_PCT = 15


def approximate(latitude: float, longitude: float) -> tuple[float, float, float]:
    """
    Snap a position to the centre of a fixed grid cell (about 1.1 km with the
    default 0.01 degree grid). Snapping is deterministic, so repeated reads of
    the same position cannot be averaged to recover the exact point.
    Returns (lat, lon, minimum accuracy in metres).
    """
    grid = get_settings().approximate_grid_degrees
    lat = (math.floor(latitude / grid) + 0.5) * grid
    lon = (math.floor(longitude / grid) + 0.5) * grid
    lat = max(-90.0, min(90.0, lat))
    lon = ((lon + 180.0) % 360.0) - 180.0
    return round(lat, 5), round(lon, 5), round(grid * METRES_PER_DEGREE_LAT)


def freshness(recorded_at: datetime | None, now: datetime | None = None) -> tuple[str, int | None]:
    """
    Honest freshness label. A location is only "live" when it is recent; after
    ``location_dead_seconds`` the device is presented as "dead" (no current
    data) rather than silently showing an old point as current.
    """
    if recorded_at is None:
        return "unknown", None
    settings = get_settings()
    age = max(0, int(((now or utcnow()) - recorded_at).total_seconds()))
    if age <= settings.location_live_seconds:
        return "live", age
    if age <= settings.location_dead_seconds:
        return "stale", age
    return "dead", age


def navigation_links(latitude: float, longitude: float) -> dict[str, str]:
    """Links to external map apps. Only the coordinate is included: no names or ids."""
    point = f"{latitude:.5f},{longitude:.5f}"
    return {
        "openstreetmap": f"https://www.openstreetmap.org/directions?to={point.replace(',', '%2C')}",
        "google_maps": f"https://www.google.com/maps/dir/?api=1&destination={point}",
        "geo_uri": f"geo:{point}",
    }
