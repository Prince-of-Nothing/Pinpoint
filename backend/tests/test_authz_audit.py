"""Attempts to use other people's records are denied identically to missing records and audited."""

from sqlalchemy import select

from app.models import AuditEvent
from tests.helpers import befriend, create_account, share


def denied_events(db):
    return db.scalars(select(AuditEvent).where(AuditEvent.event_type == "authz.denied")).all()


def test_idor_probe_is_audited_but_indistinguishable(client, db):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    befriend(client, alice, bob)
    sid = share(client, alice, bob)["id"]

    probe = client.get(f"/api/v1/location-shares/{sid}", headers=eve.headers)
    missing = client.get(
        "/api/v1/location-shares/00000000-0000-0000-0000-000000000000", headers=eve.headers
    )
    assert probe.status_code == missing.status_code == 404
    assert probe.json()["detail"] == missing.json()["detail"]

    (event,) = denied_events(db)
    assert event.actor_user_id == eve.id
    assert event.target_type == "location_share" and event.target_id == sid


def test_recipient_trying_to_upgrade_precision_is_audited(client, db):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    befriend(client, alice, bob)
    sid = share(client, alice, bob)["id"]
    response = client.patch(
        f"/api/v1/location-shares/{sid}", headers=bob.headers, json={"precision": "exact"}
    )
    assert response.status_code == 403
    assert denied_events(db)[0].actor_user_id == bob.id


def test_foreign_session_and_check_in_probes_are_audited(client, db):
    alice, eve = create_account(client, "alice"), create_account(client, "eve")
    session_id = client.get("/api/v1/auth/sessions", headers=alice.headers).json()[0]["id"]
    check_in = client.post("/api/v1/check-ins", headers=alice.headers, json={"due_in_minutes": 30})
    client.delete(f"/api/v1/auth/sessions/{session_id}", headers=eve.headers)
    client.post(f"/api/v1/check-ins/{check_in.json()['id']}/cancel", headers=eve.headers)
    assert {e.target_type for e in denied_events(db)} == {"session", "check_in"}
