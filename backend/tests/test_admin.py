"""Role-based access control for operator endpoints (RBAC)."""

from sqlalchemy import select

from app.cli import main as cli_main
from app.models import AuditEvent, User
from tests.helpers import DEFAULT_PASSWORD, create_account, login

ADMIN = "/api/v1/admin"


def make_admin(client, db, username="ops"):
    account = create_account(client, username)
    user = db.scalar(select(User).where(User.username == username))
    user.role = "admin"
    db.commit()
    return account


def test_regular_user_is_denied_and_audited(client, db):
    alice = create_account(client, "alice")
    for method, path in [
        ("get", "/users"),
        ("get", "/audit-events"),
        ("post", f"/users/{alice.id}/unlock"),
        ("post", f"/users/{alice.id}/deactivate"),
    ]:
        response = getattr(client, method)(f"{ADMIN}{path}", headers=alice.headers)
        assert response.status_code == 403, path
    denied = db.scalars(
        select(AuditEvent).where(AuditEvent.event_type == "authz.admin_denied")
    ).all()
    assert len(denied) == 4


def test_unauthenticated_is_401(client):
    assert client.get(f"{ADMIN}/users").status_code == 401


def test_role_in_token_is_not_trusted(client, db):
    """Promotion/demotion takes effect immediately because the role is read from the database."""
    ops = make_admin(client, db)
    assert client.get(f"{ADMIN}/users", headers=ops.headers).status_code == 200
    user = db.scalar(select(User).where(User.username == "ops"))
    user.role = "user"
    db.commit()
    assert client.get(f"{ADMIN}/users", headers=ops.headers).status_code == 403


def test_admin_can_list_users_and_audit_events(client, db):
    ops = make_admin(client, db)
    create_account(client, "alice")
    users = client.get(f"{ADMIN}/users", headers=ops.headers).json()
    assert {u["username"] for u in users} == {"ops", "alice"}
    assert "password_hash" not in str(users)
    events = client.get(
        f"{ADMIN}/audit-events", headers=ops.headers, params={"event_type": "auth.login"}
    ).json()
    assert events and all(e["event_type"] == "auth.login" for e in events)
    assert (
        client.get(f"{ADMIN}/audit-events", headers=ops.headers, params={"limit": 1000}).status_code
        == 422
    )
    assert (
        client.get(
            f"{ADMIN}/audit-events", headers=ops.headers, params={"event_type": "x' OR 1=1"}
        ).status_code
        == 422
    )


def test_admin_unlocks_account(client, db):
    ops = make_admin(client, db)
    alice = create_account(client, "alice")
    for _ in range(5):
        login(client, "alice", "wrong password guess")
    assert login(client, "alice").status_code == 401
    assert client.post(f"{ADMIN}/users/{alice.id}/unlock", headers=ops.headers).status_code == 200
    assert login(client, "alice").status_code == 200


def test_deactivation_ends_sessions_and_blocks_login(client, db):
    ops = make_admin(client, db)
    alice = create_account(client, "alice")
    assert (
        client.post(f"{ADMIN}/users/{alice.id}/deactivate", headers=ops.headers).json()["is_active"]
        is False
    )
    assert client.get("/api/v1/users/me", headers=alice.headers).status_code == 401
    assert login(client, "alice").status_code == 401
    assert client.post(f"{ADMIN}/users/{ops.id}/deactivate", headers=ops.headers).status_code == 400
    client.post(f"{ADMIN}/users/{alice.id}/activate", headers=ops.headers)
    assert login(client, "alice").status_code == 200


def test_admin_has_no_location_access(client, db):
    """Least privilege: there is no admin route for locations or shares."""
    from app.main import app

    admin_paths = [path for path in app.openapi()["paths"] if path.startswith(ADMIN)]
    assert admin_paths and not any("location" in path for path in admin_paths)


def test_cli_creates_admin(client, db, monkeypatch):
    monkeypatch.setenv("ADMIN_PASSWORD", DEFAULT_PASSWORD)
    assert cli_main(["create-admin", "--username", "root_ops", "--email", "ops@example.com"]) == 0
    assert db.scalar(select(User.role).where(User.username == "root_ops")) == "admin"
    assert cli_main(["create-admin", "--username", "root_ops", "--email", "ops@example.com"]) == 1
    monkeypatch.setenv("ADMIN_PASSWORD", "password123")
    assert cli_main(["create-admin", "--username", "weak_ops", "--email", "w@example.com"]) == 1
