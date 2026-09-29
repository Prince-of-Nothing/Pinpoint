"""Friendships, blocks and private-by-default profiles (REQ-01, REQ-02, SC-03)."""

from sqlalchemy import select

from app.models import AuditEvent
from tests.helpers import create_account

FRIENDS = "/api/v1/friendships"


def befriend(client, a, b) -> str:
    request = client.post(FRIENDS, headers=a.headers, json={"username": b.username})
    assert request.status_code == 201, request.text
    fid = request.json()["id"]
    assert client.post(f"{FRIENDS}/{fid}/accept", headers=b.headers).status_code == 200
    return fid


# ------------------------------------------------------------- friendships


def test_friend_request_lifecycle(client, db):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    sent = client.post(FRIENDS, headers=alice.headers, json={"username": "BOB"})
    assert sent.status_code == 201
    assert sent.json()["status"] == "pending" and sent.json()["direction"] == "outgoing"

    incoming = client.get(FRIENDS, headers=bob.headers).json()["incoming"]
    assert [f["user"]["username"] for f in incoming] == ["alice"]
    assert "email" not in str(incoming)

    accepted = client.post(f"{FRIENDS}/{sent.json()['id']}/accept", headers=bob.headers)
    assert accepted.json()["status"] == "accepted"
    assert len(client.get(FRIENDS, headers=alice.headers).json()["friends"]) == 1
    types = db.scalars(select(AuditEvent.event_type)).all()
    assert "friendship.requested" in types and "friendship.accepted" in types


def test_requester_cannot_accept_own_request(client):
    alice = create_account(client, "alice")
    create_account(client, "bob")
    fid = client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).json()["id"]
    assert client.post(f"{FRIENDS}/{fid}/accept", headers=alice.headers).status_code == 403


def test_third_party_cannot_touch_friendship(client):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    fid = client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).json()["id"]
    assert client.post(f"{FRIENDS}/{fid}/accept", headers=eve.headers).status_code == 404
    assert client.post(f"{FRIENDS}/{fid}/decline", headers=eve.headers).status_code == 404
    assert client.delete(f"{FRIENDS}/{fid}", headers=eve.headers).status_code == 404


def test_decline_allows_new_request_later(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    fid = client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).json()["id"]
    assert client.post(f"{FRIENDS}/{fid}/decline", headers=bob.headers).status_code == 204
    assert client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).status_code == 201


def test_duplicate_and_reverse_requests_conflict(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    assert client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).status_code == 201
    assert client.post(FRIENDS, headers=alice.headers, json={"username": "bob"}).status_code == 409
    assert client.post(FRIENDS, headers=bob.headers, json={"username": "alice"}).status_code == 409


def test_cannot_befriend_self_or_unknown(client):
    alice = create_account(client, "alice")
    assert (
        client.post(FRIENDS, headers=alice.headers, json={"username": "alice"}).status_code == 400
    )
    assert (
        client.post(FRIENDS, headers=alice.headers, json={"username": "ghost"}).status_code == 404
    )


def test_remove_friendship_and_cancel_request(client):
    alice, bob, carol = (create_account(client, n) for n in ("alice", "bob", "carol"))
    fid = befriend(client, alice, bob)
    assert client.delete(f"{FRIENDS}/{fid}", headers=bob.headers).status_code == 204
    assert client.get(FRIENDS, headers=alice.headers).json()["friends"] == []

    pending = client.post(FRIENDS, headers=alice.headers, json={"username": "carol"}).json()["id"]
    assert client.delete(f"{FRIENDS}/{pending}", headers=carol.headers).status_code == 409
    assert client.delete(f"{FRIENDS}/{pending}", headers=alice.headers).status_code == 204


# ------------------------------------------------------------------ blocks


def test_block_removes_friendship_and_hides_user(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    assert (
        client.post("/api/v1/blocks", headers=alice.headers, json={"username": "bob"}).status_code
        == 201
    )
    assert client.get(FRIENDS, headers=bob.headers).json()["friends"] == []

    # Bob cannot send a new request; the answer is identical to an unknown user
    blocked = client.post(FRIENDS, headers=bob.headers, json={"username": "alice"})
    unknown = client.post(FRIENDS, headers=bob.headers, json={"username": "ghost"})
    assert blocked.status_code == unknown.status_code == 404
    assert blocked.json()["detail"] == unknown.json()["detail"]


def test_block_list_and_unblock(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    client.post("/api/v1/blocks", headers=alice.headers, json={"username": "bob"})
    assert (
        client.post("/api/v1/blocks", headers=alice.headers, json={"username": "bob"}).status_code
        == 409
    )
    listed = client.get("/api/v1/blocks", headers=alice.headers).json()
    assert [b["user"]["username"] for b in listed] == ["bob"]
    assert client.delete("/api/v1/blocks/bob", headers=alice.headers).status_code == 204
    assert client.delete("/api/v1/blocks/bob", headers=alice.headers).status_code == 404
    assert client.post(FRIENDS, headers=bob.headers, json={"username": "alice"}).status_code == 201


# ---------------------------------------------------------------- profiles


def test_profile_visibility_by_relationship(client):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    client.patch(
        "/api/v1/users/me", headers=alice.headers, json={"display_name": "Alice", "bio": "Hi"}
    )
    befriend(client, alice, bob)

    own = client.get("/api/v1/users/alice", headers=alice.headers).json()
    assert own["relationship"] == "self" and own["bio"] == "Hi"

    as_friend = client.get("/api/v1/users/alice", headers=bob.headers).json()
    assert as_friend["relationship"] == "friend" and as_friend["display_name"] == "Alice"

    as_stranger = client.get("/api/v1/users/alice", headers=eve.headers).json()
    assert as_stranger == {
        "username": "alice",
        "relationship": "none",
        "display_name": None,
        "bio": None,
        "friends_since": None,
    }


def test_blocked_user_sees_empty_profile(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    client.patch("/api/v1/users/me", headers=alice.headers, json={"display_name": "Alice"})
    befriend(client, alice, bob)
    client.post("/api/v1/blocks", headers=alice.headers, json={"username": "bob"})
    assert client.get("/api/v1/users/alice", headers=bob.headers).json()["relationship"] == "none"


def test_profile_never_exposes_email_or_internal_fields(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    text = client.get("/api/v1/users/alice", headers=bob.headers).text
    for secret_field in ("email", "password", "role", "failed_login", "locked"):
        assert secret_field not in text


def test_profile_update_validation(client):
    alice = create_account(client, "alice")
    assert (
        client.patch("/api/v1/users/me", headers=alice.headers, json={"bio": "x" * 161}).status_code
        == 422
    )
    assert (
        client.patch(
            "/api/v1/users/me", headers=alice.headers, json={"bio": "bad\x00byte"}
        ).status_code
        == 422
    )
    assert (
        client.patch("/api/v1/users/me", headers=alice.headers, json={"role": "admin"}).status_code
        == 422
    )
    me = client.get("/api/v1/users/me", headers=alice.headers).json()
    assert me["role"] == "user"


def test_profile_requires_authentication(client):
    create_account(client, "alice")
    assert client.get("/api/v1/users/alice").status_code == 401
