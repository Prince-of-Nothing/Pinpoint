"""Location updates, recipient view, precision, freshness and navigation links (REQ-03/04/06/07)."""

from datetime import timedelta
from urllib.parse import urlparse

from app.core.config import get_settings
from app.core.timeutils import utcnow
from app.models import LatestLocation, LocationShare
from tests.helpers import befriend, create_account, share

ME = "/api/v1/locations/me"
VISIBLE = "/api/v1/locations/visible"
BUCHAREST = {"latitude": 44.426767, "longitude": 26.102538, "accuracy_m": 12.0, "battery_pct": 80}


def setup_pair(client, **share_options):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    created = share(client, alice, bob, **share_options) if share_options is not None else None
    return alice, bob, created


def test_update_and_read_own_location(client):
    alice = create_account(client, "alice")
    result = client.put(ME, headers=alice.headers, json=BUCHAREST)
    assert result.status_code == 200 and result.json()["applied"] is True
    own = client.get(ME, headers=alice.headers).json()
    assert own["latitude"] == BUCHAREST["latitude"] and own["freshness"] == "live"


def test_exact_share_returns_exact_position(client):
    alice, bob, _ = setup_pair(client, precision="exact")
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    (item,) = client.get(VISIBLE, headers=bob.headers).json()
    assert item["user"]["username"] == "alice"
    assert item["latitude"] == BUCHAREST["latitude"] and item["longitude"] == BUCHAREST["longitude"]
    assert item["accuracy_m"] == 12.0 and item["freshness"] == "live"


def test_approximate_share_hides_exact_position(client):
    alice, bob, _ = setup_pair(client, precision="approximate")
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    (item,) = client.get(VISIBLE, headers=bob.headers).json()
    assert item["latitude"] != BUCHAREST["latitude"]
    assert abs(item["latitude"] - BUCHAREST["latitude"]) <= 0.01
    assert item["accuracy_m"] >= 1000
    assert str(BUCHAREST["latitude"]) not in client.get(VISIBLE, headers=bob.headers).text


def test_approximation_is_stable_for_nearby_points(client):
    """Deterministic snapping: small movements inside a cell cannot be averaged out."""
    alice, bob, _ = setup_pair(client)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    first = client.get(VISIBLE, headers=bob.headers).json()[0]
    nudged = {**BUCHAREST, "latitude": BUCHAREST["latitude"] + 0.0001}
    client.put(ME, headers=alice.headers, json=nudged)
    second = client.get(VISIBLE, headers=bob.headers).json()[0]
    assert (first["latitude"], first["longitude"]) == (second["latitude"], second["longitude"])


def test_sharing_is_one_way(client):
    alice, bob, _ = setup_pair(client)
    client.put(ME, headers=bob.headers, json=BUCHAREST)
    assert client.get(VISIBLE, headers=alice.headers).json() == []


def test_friend_without_share_sees_nothing(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    assert client.get(VISIBLE, headers=bob.headers).json() == []


def test_revoked_share_stops_access_immediately(client):
    alice, bob, created = setup_pair(client)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    assert len(client.get(VISIBLE, headers=bob.headers).json()) == 1
    client.delete(f"/api/v1/location-shares/{created['id']}", headers=alice.headers)
    assert client.get(VISIBLE, headers=bob.headers).json() == []


def test_expired_share_stops_access(client, db):
    alice, bob, created = setup_pair(client, expires_in_minutes=5)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    db.get(LocationShare, created["id"]).expires_at = utcnow() - timedelta(seconds=1)
    db.commit()
    assert client.get(VISIBLE, headers=bob.headers).json() == []


def test_freshness_transitions(client, db):
    alice, bob, _ = setup_pair(client)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    settings = get_settings()
    location = db.get(LatestLocation, alice.id)

    location.recorded_at = utcnow() - timedelta(seconds=settings.location_live_seconds + 5)
    db.commit()
    stale = client.get(VISIBLE, headers=bob.headers).json()[0]
    assert stale["freshness"] == "stale"

    location.recorded_at = utcnow() - timedelta(seconds=settings.location_dead_seconds + 5)
    db.commit()
    dead = client.get(VISIBLE, headers=bob.headers).json()[0]
    assert dead["freshness"] == "dead"
    assert dead["is_charging"] is None  # a dead device does not claim a current charging state
    assert dead["last_battery_pct"] == 80 and dead["recorded_at"]


def test_share_without_location_is_unknown(client):
    _, bob, _ = setup_pair(client)
    (item,) = client.get(VISIBLE, headers=bob.headers).json()
    assert (
        item["freshness"] == "unknown" and item["latitude"] is None and item["navigation"] is None
    )


def test_low_battery_flag(client):
    alice, bob, _ = setup_pair(client)
    client.put(ME, headers=alice.headers, json={**BUCHAREST, "battery_pct": 9})
    assert client.get(VISIBLE, headers=bob.headers).json()[0]["low_battery"] is True


def test_navigation_links_contain_only_coordinates(client):
    alice, bob, _ = setup_pair(client, precision="exact")
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    links = client.get(VISIBLE, headers=bob.headers).json()[0]["navigation"]
    for url in links.values():
        assert "alice" not in url and alice.id not in url and "bob" not in url
    parsed = urlparse(links["openstreetmap"])
    assert parsed.netloc == "www.openstreetmap.org" and parsed.query == "to=44.42677%2C26.10254"
    assert links["geo_uri"] == "geo:44.42677,26.10254"


def test_out_of_order_updates_do_not_overwrite_newer_data(client):
    alice = create_account(client, "alice")
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    older = (utcnow() - timedelta(minutes=10)).isoformat()
    result = client.put(
        ME, headers=alice.headers, json={**BUCHAREST, "latitude": 1.0, "recorded_at": older}
    )
    assert result.json()["applied"] is False
    assert client.get(ME, headers=alice.headers).json()["latitude"] == BUCHAREST["latitude"]


def test_invalid_coordinates_and_timestamps_rejected(client):
    alice = create_account(client, "alice")
    future = (utcnow() + timedelta(hours=1)).isoformat()
    ancient = (utcnow() - timedelta(days=3)).isoformat()
    bad_bodies = [
        {"latitude": 91, "longitude": 0},
        {"latitude": 0, "longitude": -181},
        {"latitude": "NaN", "longitude": 0},
        {"latitude": "Infinity", "longitude": 0},
        {"latitude": "44.4; DROP TABLE users", "longitude": 0},
        {"latitude": 0, "longitude": 0, "battery_pct": 101},
        {"latitude": 0, "longitude": 0, "accuracy_m": -1},
        {"latitude": 0, "longitude": 0, "user_id": "someone-else"},
        {"latitude": 0, "longitude": 0, "recorded_at": future},
        {"latitude": 0, "longitude": 0, "recorded_at": ancient},
        {"longitude": 0},
    ]
    for body in bad_bodies:
        assert client.put(ME, headers=alice.headers, json=body).status_code == 422, body


def test_delete_own_location(client):
    alice, bob, _ = setup_pair(client)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    assert client.delete(ME, headers=alice.headers).status_code == 204
    assert client.get(ME, headers=alice.headers).status_code == 404
    assert client.get(VISIBLE, headers=bob.headers).json()[0]["freshness"] == "unknown"


def test_block_hides_location_even_if_share_row_survives(client, db):
    alice, bob, created = setup_pair(client)
    client.put(ME, headers=alice.headers, json=BUCHAREST)
    client.post("/api/v1/blocks", headers=bob.headers, json={"username": "alice"})
    # Defence in depth: even if a share row were re-activated, the block check hides it
    row = db.get(LocationShare, created["id"])
    row.revoked_at = None
    db.commit()
    assert client.get(VISIBLE, headers=bob.headers).json() == []
