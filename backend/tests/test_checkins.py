"""Check-ins (REQ-05, reduced MVP scope)."""

from datetime import timedelta

from app.core.timeutils import utcnow
from app.models import CheckIn
from tests.helpers import befriend, create_account, share

CHECKINS = "/api/v1/check-ins"


def create(client, account, **body):
    response = client.post(CHECKINS, headers=account.headers, json={"due_in_minutes": 30, **body})
    assert response.status_code == 201, response.text
    return response.json()


def shift_due(db, check_in_id, delta):
    row = db.get(CheckIn, check_in_id)
    row.due_at = utcnow() + delta
    db.commit()


def test_check_in_lifecycle_complete(client):
    alice = create_account(client, "alice")
    created = create(client, alice, note="Hiking, back by 18:00")
    assert created["state"] == "pending"
    done = client.post(f"{CHECKINS}/{created['id']}/complete", headers=alice.headers)
    assert done.json()["state"] == "completed"
    again = client.post(f"{CHECKINS}/{created['id']}/complete", headers=alice.headers)
    assert again.status_code == 409


def test_cancel(client):
    alice = create_account(client, "alice")
    created = create(client, alice)
    assert (
        client.post(f"{CHECKINS}/{created['id']}/cancel", headers=alice.headers).json()["state"]
        == "cancelled"
    )


def test_state_transitions_follow_the_clock(client, db):
    alice = create_account(client, "alice")
    cid = create(client, alice, grace_minutes=15)["id"]

    def state():
        (item,) = client.get(CHECKINS, headers=alice.headers).json()
        return item["state"]

    shift_due(db, cid, timedelta(minutes=5))
    assert state() == "due_soon"
    shift_due(db, cid, timedelta(minutes=-5))
    assert state() == "overdue"
    shift_due(db, cid, timedelta(minutes=-16))
    assert state() == "missed"


def test_only_owner_can_manage(client):
    alice, eve = create_account(client, "alice"), create_account(client, "eve")
    cid = create(client, alice)["id"]
    assert client.post(f"{CHECKINS}/{cid}/complete", headers=eve.headers).status_code == 404
    assert client.post(f"{CHECKINS}/{cid}/cancel", headers=eve.headers).status_code == 404


def test_recipients_of_location_share_see_check_ins(client, db):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    befriend(client, alice, bob)
    befriend(client, alice, eve)
    share(client, alice, bob)
    cid = create(client, alice)["id"]
    shift_due(db, cid, timedelta(minutes=-60))

    (seen,) = client.get(f"{CHECKINS}/shared", headers=bob.headers).json()
    assert seen["user"]["username"] == "alice" and seen["state"] == "missed"
    # A friend without a location share sees nothing
    assert client.get(f"{CHECKINS}/shared", headers=eve.headers).json() == []


def test_validation_and_limits(client):
    alice = create_account(client, "alice")
    for body in [
        {"due_in_minutes": 1},
        {"due_in_minutes": 5000},
        {"due_in_minutes": 30, "grace_minutes": -1},
        {"due_in_minutes": 30, "note": "x" * 141},
    ]:
        assert client.post(CHECKINS, headers=alice.headers, json=body).status_code == 422, body
    for _ in range(5):
        create(client, alice)
    assert (
        client.post(CHECKINS, headers=alice.headers, json={"due_in_minutes": 30}).status_code == 409
    )
