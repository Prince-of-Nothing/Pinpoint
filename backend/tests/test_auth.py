"""Registration, login, token handling, sessions, logout and password change."""

from datetime import timedelta

import jwt
from sqlalchemy import select

from app.core.config import get_settings
from app.core.timeutils import utcnow
from app.models import AuditEvent, AuthSession, User
from tests.helpers import DEFAULT_PASSWORD, create_account, login, register

SESSIONS = "/api/v1/auth/sessions"


# ------------------------------------------------------------ registration


def test_register_returns_user_without_secrets(client):
    response = register(client, "Alice_1", display_name="Alice")
    assert response.status_code == 201
    body = response.json()
    assert body["username"] == "alice_1"
    assert body["role"] == "user"
    assert "password" not in response.text and "password_hash" not in body


def test_password_is_stored_as_argon2id_hash(client, db):
    register(client, "alice")
    user = db.scalar(select(User).where(User.username == "alice"))
    assert user.password_hash.startswith("$argon2id$")
    assert DEFAULT_PASSWORD not in user.password_hash


def test_duplicate_username_or_email_is_rejected_case_insensitively(client):
    assert register(client, "alice").status_code == 201
    assert register(client, "ALICE").status_code == 409
    other = client.post(
        "/api/v1/auth/register",
        json={"username": "alice2", "email": "ALICE@example.com", "password": DEFAULT_PASSWORD},
    )
    assert other.status_code == 409


def test_weak_passwords_are_rejected(client):
    for password in ["short", "password123", "aaaaaaaaaaaa", "xx-alice-xx-secret"]:
        response = register(client, "alice", password=password)
        assert response.status_code == 422, password
        assert response.json()["code"] == "WEAK_PASSWORD"
        assert password not in response.text  # never echo the password back


def test_invalid_username_rejected(client):
    for username in ["ab", "bad name", "<script>", "admin", "a" * 31]:
        assert register(client, username).status_code == 422, username


def test_register_rejects_mass_assignment_of_role(client):
    response = register(client, "mallory", role="admin")
    assert response.status_code == 422
    assert "role" in response.json()["errors"]


# ------------------------------------------------------------------ login


def test_login_body_mode_returns_tokens(client):
    register(client, "alice")
    response = login(client, "alice")
    assert response.status_code == 200
    body = response.json()
    assert body["token_type"] == "bearer"
    assert body["expires_in"] == 15 * 60
    assert body["refresh_token"]
    assert body["user"]["username"] == "alice"


def test_login_accepts_email(client):
    register(client, "alice")
    assert login(client, "Alice@Example.com").status_code == 200


def test_login_cookie_mode_sets_hardened_cookies(client):
    register(client, "alice")
    response = login(client, "alice", delivery="cookie")
    assert response.status_code == 200
    body = response.json()
    assert body["refresh_token"] is None  # never exposed to JavaScript
    assert body["csrf_token"]
    cookies = response.headers.get_list("set-cookie")
    refresh_cookie = next(c for c in cookies if c.startswith("at_refresh="))
    assert "HttpOnly" in refresh_cookie
    assert "SameSite=strict" in refresh_cookie
    assert "Path=/api/v1/auth" in refresh_cookie
    csrf_cookie = next(c for c in cookies if c.startswith("at_csrf="))
    assert "HttpOnly" not in csrf_cookie


def test_wrong_password_and_unknown_user_look_identical(client):
    register(client, "alice")
    wrong = login(client, "alice", "not the right password")
    unknown = login(client, "nobody", "not the right password")
    assert wrong.status_code == unknown.status_code == 401
    assert wrong.json()["detail"] == unknown.json()["detail"]


def test_login_is_audited(client, db):
    register(client, "alice")
    login(client, "alice", "wrong password here")
    login(client, "alice")
    events = db.scalars(select(AuditEvent).where(AuditEvent.event_type == "auth.login")).all()
    assert [e.outcome for e in events] == ["failure", "success"]
    for event in events:  # secrets never reach the audit trail
        assert DEFAULT_PASSWORD not in str(event.details)
        assert "wrong password here" not in str(event.details)


# ------------------------------------------------------------ access tokens


def test_missing_token_returns_401_not_500(client):
    response = client.get(SESSIONS)
    assert response.status_code == 401
    assert response.headers["www-authenticate"] == "Bearer"


def test_tampered_and_forged_tokens_rejected(client):
    alice = create_account(client, "alice")
    header, payload, signature = alice.access_token.split(".")
    tampered = f"{header}.{payload}.{signature[:-4]}AAAA"
    assert client.get(SESSIONS, headers={"Authorization": f"Bearer {tampered}"}).status_code == 401

    claims = jwt.decode(alice.access_token, options={"verify_signature": False})
    forged_none = jwt.encode(claims, key=None, algorithm="none")
    assert (
        client.get(SESSIONS, headers={"Authorization": f"Bearer {forged_none}"}).status_code == 401
    )

    forged_key = jwt.encode(claims, "attacker-guessed-key-attacker-guessed-key", algorithm="HS256")
    assert (
        client.get(SESSIONS, headers={"Authorization": f"Bearer {forged_key}"}).status_code == 401
    )


def test_expired_access_token_rejected(client):
    alice = create_account(client, "alice")
    settings = get_settings()
    claims = jwt.decode(alice.access_token, options={"verify_signature": False})
    claims["exp"] = int((utcnow() - timedelta(seconds=1)).timestamp())
    expired = jwt.encode(claims, settings.secret_key, algorithm=settings.jwt_algorithm)
    assert client.get(SESSIONS, headers={"Authorization": f"Bearer {expired}"}).status_code == 401


def test_refresh_token_cannot_be_used_as_access_token(client):
    alice = create_account(client, "alice")
    response = client.get(SESSIONS, headers={"Authorization": f"Bearer {alice.refresh_token}"})
    assert response.status_code == 401


def test_idle_session_expires(client, db):
    alice = create_account(client, "alice")
    session = db.scalar(select(AuthSession))
    session.last_seen_at = utcnow() - timedelta(
        minutes=get_settings().session_idle_timeout_minutes + 1
    )
    db.commit()
    assert client.get(SESSIONS, headers=alice.headers).status_code == 401


def test_absolute_session_lifetime_enforced(client, db):
    alice = create_account(client, "alice")
    session = db.scalar(select(AuthSession))
    session.expires_at = utcnow() - timedelta(seconds=1)
    db.commit()
    assert client.get(SESSIONS, headers=alice.headers).status_code == 401


# ------------------------------------------------------- refresh rotation


def test_refresh_rotates_token(client):
    alice = create_account(client, "alice")
    response = client.post("/api/v1/auth/refresh", json={"refresh_token": alice.refresh_token})
    assert response.status_code == 200
    new_refresh = response.json()["refresh_token"]
    assert new_refresh and new_refresh != alice.refresh_token


def test_refresh_token_reuse_revokes_session(client):
    alice = create_account(client, "alice")
    first = client.post("/api/v1/auth/refresh", json={"refresh_token": alice.refresh_token})
    rotated = first.json()["refresh_token"]
    new_access = first.json()["access_token"]

    # An attacker replays the old (already used) token
    replay = client.post("/api/v1/auth/refresh", json={"refresh_token": alice.refresh_token})
    assert replay.status_code == 401

    # The whole session is now dead: the legitimate rotated token and access token fail too
    assert client.post("/api/v1/auth/refresh", json={"refresh_token": rotated}).status_code == 401
    assert (
        client.get(SESSIONS, headers={"Authorization": f"Bearer {new_access}"}).status_code == 401
    )


def test_cookie_refresh_requires_csrf_token(client):
    register(client, "alice")
    body = login(client, "alice", delivery="cookie").json()

    no_header = client.post("/api/v1/auth/refresh")
    assert no_header.status_code == 403
    assert no_header.json()["code"] == "CSRF_FAILED"

    wrong = client.post("/api/v1/auth/refresh", headers={"X-CSRF-Token": "forged-value"})
    assert wrong.status_code == 403

    ok = client.post("/api/v1/auth/refresh", headers={"X-CSRF-Token": body["csrf_token"]})
    assert ok.status_code == 200
    assert ok.json()["refresh_token"] is None


def test_refresh_without_any_token_is_401(client):
    assert client.post("/api/v1/auth/refresh", json={}).status_code == 401


# ------------------------------------------------------ logout / sessions


def test_logout_invalidates_access_and_refresh_tokens(client):
    alice = create_account(client, "alice")
    assert client.post("/api/v1/auth/logout", headers=alice.headers).status_code == 204
    assert client.get(SESSIONS, headers=alice.headers).status_code == 401
    refresh = client.post("/api/v1/auth/refresh", json={"refresh_token": alice.refresh_token})
    assert refresh.status_code == 401


def test_logout_all_ends_every_session(client):
    alice = create_account(client, "alice")
    second = login(client, "alice").json()
    assert client.post("/api/v1/auth/logout-all", headers=alice.headers).status_code == 204
    other_headers = {"Authorization": f"Bearer {second['access_token']}"}
    assert client.get(SESSIONS, headers=other_headers).status_code == 401


def test_list_and_revoke_own_sessions(client):
    alice = create_account(client, "alice")
    second = login(client, "alice").json()
    sessions = client.get(SESSIONS, headers=alice.headers).json()
    assert len(sessions) == 2
    assert sum(s["current"] for s in sessions) == 1
    other = next(s for s in sessions if not s["current"])
    assert client.delete(f"{SESSIONS}/{other['id']}", headers=alice.headers).status_code == 204
    other_headers = {"Authorization": f"Bearer {second['access_token']}"}
    assert client.get(SESSIONS, headers=other_headers).status_code == 401


def test_cannot_revoke_another_users_session(client):
    alice = create_account(client, "alice")
    bob = create_account(client, "bob")
    bob_session = client.get(SESSIONS, headers=bob.headers).json()[0]["id"]
    response = client.delete(f"{SESSIONS}/{bob_session}", headers=alice.headers)
    assert response.status_code == 404
    assert client.get(SESSIONS, headers=bob.headers).status_code == 200


def test_session_limit_revokes_oldest(client, monkeypatch):
    monkeypatch.setattr(get_settings(), "max_sessions_per_user", 2)
    first = create_account(client, "alice")
    login(client, "alice")
    login(client, "alice")
    assert client.get(SESSIONS, headers=first.headers).status_code == 401


# ---------------------------------------------------------------- password


def test_password_change_revokes_all_sessions(client):
    alice = create_account(client, "alice")
    new_password = "a completely new passphrase"
    response = client.post(
        "/api/v1/auth/password",
        headers=alice.headers,
        json={"current_password": DEFAULT_PASSWORD, "new_password": new_password},
    )
    assert response.status_code == 204
    assert client.get(SESSIONS, headers=alice.headers).status_code == 401
    assert login(client, "alice").status_code == 401
    assert login(client, "alice", new_password).status_code == 200


def test_password_change_requires_current_password(client):
    alice = create_account(client, "alice")
    response = client.post(
        "/api/v1/auth/password",
        headers=alice.headers,
        json={"current_password": "wrong password!!", "new_password": "another good passphrase"},
    )
    assert response.status_code == 400
    assert login(client, "alice").status_code == 200


def test_password_change_enforces_policy(client):
    alice = create_account(client, "alice")
    response = client.post(
        "/api/v1/auth/password",
        headers=alice.headers,
        json={"current_password": DEFAULT_PASSWORD, "new_password": "password123"},
    )
    assert response.status_code == 422


# ----------------------------------------------------------------- lockout


def test_account_locks_after_repeated_failures(client, db):
    register(client, "alice")
    for _ in range(get_settings().lockout_threshold):
        assert login(client, "alice", "wrong password guess").status_code == 401
    # Even the correct password is refused while locked, with the same generic message
    locked = login(client, "alice")
    assert locked.status_code == 401
    assert locked.json()["detail"] == "Invalid username/email or password."
    assert db.scalar(select(AuditEvent).where(AuditEvent.event_type == "auth.lockout"))


def test_lockout_expires(client, db):
    register(client, "alice")
    for _ in range(get_settings().lockout_threshold):
        login(client, "alice", "wrong password guess")
    user = db.scalar(select(User).where(User.username == "alice"))
    user.locked_until = utcnow() - timedelta(seconds=1)
    db.commit()
    assert login(client, "alice").status_code == 200


def test_successful_login_resets_failure_counter(client, db):
    register(client, "alice")
    for _ in range(get_settings().lockout_threshold - 1):
        login(client, "alice", "wrong password guess")
    assert login(client, "alice").status_code == 200
    login(client, "alice", "wrong password guess")
    assert login(client, "alice").status_code == 200
