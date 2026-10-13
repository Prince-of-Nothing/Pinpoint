"""Directional location shares (REQ-03, SC-02)."""

from datetime import timedelta

from sqlalchemy import select

from app.core.timeutils import utcnow
from app.models import AuditEvent, LocationShare
from tests.helpers import befriend, create_account, share

SHARES = "/api/v1/location-shares"


def test_share_with_friend_defaults_to_approximate(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    created = share(client, alice, bob, purpose="Walking home", expires_in_minutes=60)
    assert created["precision"] == "approximate"
    assert created["direction"] == "outgoing" and created["active"] is True
    assert created["expires_at"] is not None

    listed = client.get(SHARES, headers=bob.headers).json()
    assert [s["id"] for s in listed["incoming"]] == [created["id"]]
    assert listed["outgoing"] == []  # sharing is one-way


def test_cannot_share_with_non_friend_or_self(client):
    alice = create_account(client, "alice")
    create_account(client, "eve")
    response = client.post(SHARES, headers=alice.headers, json={"recipient_username": "eve"})
    assert response.status_code == 403
    assert response.json()["code"] == "NOT_FRIENDS"
    assert (
        client.post(SHARES, headers=alice.headers, json={"recipient_username": "alice"}).status_code
        == 400
    )


def test_pending_friendship_is_not_enough(client):
    alice = create_account(client, "alice")
    create_account(client, "bob")
    client.post("/api/v1/friendships", headers=alice.headers, json={"username": "bob"})
    assert (
        client.post(SHARES, headers=alice.headers, json={"recipient_username": "bob"}).status_code
        == 403
    )


def test_duplicate_active_share_conflicts(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    share(client, alice, bob)
    assert (
        client.post(SHARES, headers=alice.headers, json={"recipient_username": "bob"}).status_code
        == 409
    )


def test_input_validation(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    bad_bodies = [
        {"recipient_username": "bob", "precision": "street"},
        {"recipient_username": "bob", "expires_in_minutes": 1},
        {"recipient_username": "bob", "expires_in_minutes": 60 * 24 * 365},
        {"recipient_username": "bob", "purpose": "x" * 101},
        {"recipient_username": "bob", "owner_id": alice.id},
        {"recipient_username": "bob' OR '1'='1"},
    ]
    for body in bad_bodies:
        assert client.post(SHARES, headers=alice.headers, json=body).status_code == 422, body


def test_owner_can_update_recipient_cannot(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    sid = share(client, alice, bob)["id"]
    updated = client.patch(
        f"{SHARES}/{sid}",
        headers=alice.headers,
        json={"precision": "exact", "expires_in_minutes": 30},
    )
    assert updated.status_code == 200
    assert updated.json()["precision"] == "exact" and updated.json()["expires_at"]
    # The recipient cannot upgrade their own access
    assert (
        client.patch(
            f"{SHARES}/{sid}", headers=bob.headers, json={"precision": "exact"}
        ).status_code
        == 403
    )


def test_either_party_can_end_share(client, db):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    first = share(client, alice, bob)["id"]
    assert client.delete(f"{SHARES}/{first}", headers=alice.headers).status_code == 204
    assert client.get(f"{SHARES}/{first}", headers=bob.headers).json()["active"] is False

    second = share(client, alice, bob)["id"]
    assert client.delete(f"{SHARES}/{second}", headers=bob.headers).status_code == 204
    reasons = db.scalars(
        select(LocationShare.revoked_reason).order_by(LocationShare.created_at)
    ).all()
    assert reasons == ["owner_revoked", "recipient_removed"]
    assert db.scalar(select(AuditEvent).where(AuditEvent.event_type == "share.revoked"))


def test_third_party_cannot_see_or_change_share(client):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    befriend(client, alice, bob)
    sid = share(client, alice, bob)["id"]
    assert client.get(f"{SHARES}/{sid}", headers=eve.headers).status_code == 404
    assert (
        client.patch(
            f"{SHARES}/{sid}", headers=eve.headers, json={"precision": "exact"}
        ).status_code
        == 404
    )
    assert client.delete(f"{SHARES}/{sid}", headers=eve.headers).status_code == 404


def test_expired_share_is_not_listed_and_cannot_be_updated(client, db):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    sid = share(client, alice, bob, expires_in_minutes=5)["id"]
    row = db.get(LocationShare, sid)
    row.expires_at = utcnow() - timedelta(seconds=1)
    db.commit()
    assert client.get(SHARES, headers=bob.headers).json()["incoming"] == []
    assert (
        client.patch(
            f"{SHARES}/{sid}", headers=alice.headers, json={"precision": "exact"}
        ).status_code
        == 409
    )
    share(client, alice, bob)  # a new share can be created after expiry


def test_unfriending_and_blocking_revoke_shares(client):
    alice, bob, carol = (create_account(client, n) for n in ("alice", "bob", "carol"))
    fid = befriend(client, alice, bob)
    share(client, alice, bob)
    share(client, bob, alice)
    client.delete(f"/api/v1/friendships/{fid}", headers=alice.headers)
    assert client.get(SHARES, headers=alice.headers).json() == {"outgoing": [], "incoming": []}

    befriend(client, alice, carol)
    share(client, carol, alice)
    client.post("/api/v1/blocks", headers=alice.headers, json={"username": "carol"})
    assert client.get(SHARES, headers=carol.headers).json() == {"outgoing": [], "incoming": []}
