"""
Security regression tests for the authentication and authorization core.

These tests pin the behaviour that the security documentation promises:
anti-enumeration, lockout with automatic unlock, JWT revocation on password
change, token-type confusion defence, CSRF for cookie auth, complete
mediation of location reads (IDOR), precision reduction, blocking, rate
limiting, and security headers.

Run:  cd backend && python -m pytest tests/ -v
"""

from datetime import timedelta

import pytest
from pydantic import ValidationError


# ---------------------------------------------------------------------------
# Configuration hardening
# ---------------------------------------------------------------------------

def _settings(**overrides):
    from app.core.config import Settings

    base = {"environment": "production", "secret_key": "x" * 40, "debug": False}
    base.update(overrides)
    return Settings(**base)


def test_production_rejects_short_secret():
    with pytest.raises(ValidationError):
        _settings(secret_key="short")


def test_production_rejects_placeholder_secret():
    with pytest.raises(ValidationError):
        _settings(secret_key="change-this-secret-key-in-production-min-32-chars-long")


def test_production_rejects_debug():
    with pytest.raises(ValidationError):
        _settings(debug=True)


def test_production_accepts_real_config():
    assert _settings().is_production


def test_algorithm_allowlist_rejects_none():
    from app.core.config import Settings

    with pytest.raises(ValidationError):
        Settings(algorithm="none")


# ---------------------------------------------------------------------------
# Authentication: generic errors, cookies, enumeration resistance
# ---------------------------------------------------------------------------

def test_unknown_account_returns_generic_401(client):
    response = client.post(
        "/api/v1/auth/login",
        json={"email": "nobody@example.com", "password": "Whatever123!"},
    )
    assert response.status_code == 401
    body = response.json()
    assert body["error_code"] == "AUTHENTICATION_FAILED"
    assert "detail" in body
    # No hint about account existence anywhere in the payload.
    assert "exist" not in body["detail"].lower()


def test_login_failure_is_indistinguishable_for_wrong_password(client, create_user):
    create_user("real@example.com")
    wrong_user = client.post(
        "/api/v1/auth/login",
        json={"email": "nobody@example.com", "password": "Whatever123!"},
    )
    wrong_pass = client.post(
        "/api/v1/auth/login",
        json={"email": "real@example.com", "password": "WrongPass123!"},
    )
    assert wrong_user.status_code == wrong_pass.status_code == 401
    assert wrong_user.json()["error_code"] == wrong_pass.json()["error_code"]


def test_login_success_sets_hardened_cookies_and_token(client, create_user):
    create_user("alice@example.com")
    response = client.post(
        "/api/v1/auth/login",
        json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert response.status_code == 200
    body = response.json()
    assert body["token_type"] == "bearer"
    assert body["expires_in"] == 15 * 60

    set_cookies = response.headers.get_list("set-cookie")
    session_cookie = next(c for c in set_cookies if c.startswith("session_id="))
    assert "HttpOnly" in session_cookie
    assert "SameSite=lax" in session_cookie
    csrf_cookie = next(c for c in set_cookies if c.startswith("csrf_token="))
    # CSRF cookie must be readable by the client (double-submit pattern).
    assert "HttpOnly" not in csrf_cookie


def test_me_via_bearer_and_cookie(make_client, client, login):
    # A client with no credentials at all is rejected.
    fresh = make_client()
    assert fresh.get("/api/v1/auth/me").status_code == 401

    user_id, token, _ = login()

    by_token = client.get(
        "/api/v1/auth/me", headers={"Authorization": f"Bearer {token}"}
    )
    assert by_token.status_code == 200
    assert by_token.json()["id"] == user_id

    by_cookie = client.get("/api/v1/auth/me")  # cookie stored on the client
    assert by_cookie.status_code == 200
    assert by_cookie.json()["id"] == user_id


# ---------------------------------------------------------------------------
# Lockout lifecycle
# ---------------------------------------------------------------------------

def test_lockout_after_failures_and_automatic_unlock(client, create_user):
    from app.core.config import get_settings
    from app.core.timeutils import utcnow
    from app.db.session import SessionLocal
    from app.models.user import User

    settings = get_settings()
    user_id = create_user("bob@example.com")

    for _ in range(settings.lockout_threshold):
        response = client.post(
            "/api/v1/auth/login",
            json={"email": "bob@example.com", "password": "BadPass12345!"},
        )
        assert response.status_code == 401

    # Locked: even the CORRECT password gets the generic answer.
    response = client.post(
        "/api/v1/auth/login",
        json={"email": "bob@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert response.status_code == 401
    assert response.json()["error_code"] == "AUTHENTICATION_FAILED"

    with SessionLocal() as db:
        user = db.get(User, user_id)
        assert user.is_locked is True

        # Simulate the lockout window elapsing.
        user.locked_at = utcnow() - timedelta(minutes=settings.lockout_duration_minutes + 1)
        db.commit()

    response = client.post(
        "/api/v1/auth/login",
        json={"email": "bob@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert response.status_code == 200
    with SessionLocal() as db:
        user = db.get(User, user_id)
        assert user.is_locked is False
        assert user.failed_login_attempts == 0


# ---------------------------------------------------------------------------
# Token handling: revocation and type confusion
# ---------------------------------------------------------------------------

def test_password_change_revokes_outstanding_jwts(client, create_user):
    login_response = client.post(
        "/api/v1/auth/login",
        json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert login_response.status_code == 401  # user not created yet
    create_user("alice@example.com")
    login_response = client.post(
        "/api/v1/auth/login",
        json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"},
    )
    old_token = login_response.json()["access_token"]

    change = client.post(
        "/api/v1/auth/password/change",
        headers={"Authorization": f"Bearer {old_token}"},
        json={
            "current_password": "Str0ng!Passw0rd",
            "new_password": "Ev3n!Stronger9Pass",
            "confirm_password": "Ev3n!Stronger9Pass",
        },
    )
    assert change.status_code == 200

    # Old JWT is dead even though its signature is still valid.
    replay = client.get("/api/v1/auth/me", headers={"Authorization": f"Bearer {old_token}"})
    assert replay.status_code == 401

    # And the old password no longer works.
    old_login = client.post(
        "/api/v1/auth/login",
        json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert old_login.status_code == 401
    new_login = client.post(
        "/api/v1/auth/login",
        json={"email": "alice@example.com", "password": "Ev3n!Stronger9Pass"},
    )
    assert new_login.status_code == 200


def test_refresh_token_rejected_as_access_token(client, login, app):
    _, token, _ = login()
    from app.services.token_service import get_token_service

    tokens = get_token_service()
    claims = tokens.decode(token)
    assert claims is not None
    refresh = tokens.create_refresh_token(
        user_id=claims.user_id, session_id=claims.session_id,
        token_version=claims.token_version,
    )
    response = client.get(
        "/api/v1/auth/me", headers={"Authorization": f"Bearer {refresh}"}
    )
    assert response.status_code == 401


def test_logout_invalidates_cookie_session(make_client, login):
    login()  # alice exists (via the shared client fixture)
    client = make_client()
    second = client.post("/api/v1/auth/login",
                         json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"})
    assert second.status_code == 200, second.text
    assert client.get("/api/v1/auth/me").status_code == 200

    csrf = client.cookies.get("csrf_token")
    logout = client.post("/api/v1/auth/logout", headers={"X-CSRF-Token": csrf})
    assert logout.status_code == 200

    assert client.get("/api/v1/auth/me").status_code == 401


# ---------------------------------------------------------------------------
# CSRF (cookie-authenticated mutations)
# ---------------------------------------------------------------------------

def test_cookie_authenticated_mutation_requires_csrf(make_client, create_user):
    create_user("bob@example.com")
    create_user("alice@example.com", full_name="Alice")
    client = make_client()
    assert client.post("/api/v1/auth/login",
                       json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"}).status_code == 200

    # Without the CSRF header the mutation is rejected.
    denied = client.post("/api/v1/friendships/",
                         json={"receiver_email": "bob@example.com"})
    assert denied.status_code == 403
    assert denied.json()["error_code"] == "CSRF_FAILED"

    # With the double-submit token it succeeds.
    csrf = client.cookies.get("csrf_token")
    allowed = client.post(
        "/api/v1/friendships/",
        headers={"X-CSRF-Token": csrf},
        json={"receiver_email": "bob@example.com"},
    )
    assert allowed.status_code == 201


def test_bearer_authenticated_mutation_needs_no_csrf(client, login):
    _, token, _ = login()
    from app.db.session import SessionLocal
    from app.models.user import User, UserRole
    from app.services.security import get_password_hash, normalize_email
    with SessionLocal() as db:
        db.add(User(email="bob@example.com",
                    email_normalized=normalize_email("bob@example.com"),
                    password_hash=get_password_hash("Str0ng!Passw0rd"),
                    full_name="Bob", role=UserRole.RESIDENT))
        db.commit()

    response = client.post(
        "/api/v1/friendships/",
        headers={"Authorization": f"Bearer {token}"},
        json={"receiver_email": "bob@example.com"},
    )
    assert response.status_code == 201


# ---------------------------------------------------------------------------
# Forced password change gate
# ---------------------------------------------------------------------------

def test_must_change_password_blocks_api_until_forced_change(client, create_user):
    create_user("temp@example.com", must_change_password=True)
    login = client.post(
        "/api/v1/auth/login",
        json={"email": "temp@example.com", "password": "Str0ng!Passw0rd"},
    )
    assert login.status_code == 200
    token = login.json()["access_token"]
    headers = {"Authorization": f"Bearer {token}"}

    blocked = client.get("/api/v1/friendships/", headers=headers)
    assert blocked.status_code == 403
    assert blocked.json()["error_code"] == "PASSWORD_CHANGE_REQUIRED"

    weak = client.post(
        "/api/v1/auth/password/force-change", headers=headers,
        json={"new_password": "short", "confirm_password": "short"},
    )
    assert weak.status_code == 422  # schema bounds before the policy runs

    done = client.post(
        "/api/v1/auth/password/force-change", headers=headers,
        json={"new_password": "Ev3n!Stronger9Pass", "confirm_password": "Ev3n!Stronger9Pass"},
    )
    assert done.status_code == 200

    # The pre-change JWT is revoked (token version bump) — by design.
    assert client.get("/api/v1/friendships/", headers=headers).status_code == 401

    # Re-authentication with the new password restores access.
    relogin = client.post(
        "/api/v1/auth/login",
        json={"email": "temp@example.com", "password": "Ev3n!Stronger9Pass"},
    )
    assert relogin.status_code == 200
    new_headers = {"Authorization": f"Bearer {relogin.json()['access_token']}"}
    assert client.get("/api/v1/friendships/", headers=new_headers).status_code == 200


# ---------------------------------------------------------------------------
# Friendships + directional location authorization (SC-02) + IDOR
# ---------------------------------------------------------------------------

def _make_friends(client_a, token_a, client_b, email_b):
    from app.db.session import SessionLocal
    from app.models.user import User, UserRole
    from app.services.security import get_password_hash, normalize_email
    with SessionLocal() as db:
        if db.query(User).filter(User.email_normalized == normalize_email(email_b)).first() is None:
            db.add(User(email=email_b, email_normalized=normalize_email(email_b),
                        password_hash=get_password_hash("Str0ng!Passw0rd"),
                        full_name=email_b.split("@")[0].title(), role=UserRole.RESIDENT))
            db.commit()

    request = client_a.post(
        "/api/v1/friendships/",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"receiver_email": email_b},
    )
    assert request.status_code == 201, request.text
    friendship_id = request.json()["id"]

    login_b = client_b.post(
        "/api/v1/auth/login",
        json={"email": email_b, "password": "Str0ng!Passw0rd"},
    )
    assert login_b.status_code == 200, login_b.text
    token_b = login_b.json()["access_token"]

    accept = client_b.post(
        f"/api/v1/friendships/{friendship_id}/accept",
        headers={"Authorization": f"Bearer {token_b}"},
    )
    assert accept.status_code == 200, accept.text
    return friendship_id, token_b


def _login_client(client, email, full_name=None, create=None):
    """Ensure a user exists, log in, return the access token."""
    if create is not None:
        create(email, full_name=full_name or email.split("@")[0].title())
    response = client.post("/api/v1/auth/login",
                           json={"email": email, "password": "Str0ng!Passw0rd"})
    assert response.status_code == 200, response.text
    return response.json()["access_token"]


def test_directional_location_sharing_and_idor(make_client, create_user):
    alice_id = create_user("alice@example.com", full_name="Alice")
    client_a, client_b, client_c = make_client(), make_client(), make_client()
    token_a = _login_client(client_a, "alice@example.com")

    friendship_ab, token_b = _make_friends(client_a, token_a, client_b, "bob@example.com")
    create_user("carol@example.com", full_name="Carol")
    token_c = _login_client(client_c, "carol@example.com")

    # Alice grants explicit, purpose-bound consent to Bob only.
    share = client_a.post(
        "/api/v1/locations/shares",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"friendship_id": friendship_ab, "direction": "to_friend",
              "precision_level": "exact", "purpose": "safety"},
    )
    assert share.status_code == 201, share.text
    share_id = share.json()["id"]
    assert share.json()["expires_at"] is not None  # bounded by default

    # Consent guards: mutual cannot be self-granted; expiry is bounded.
    mutual = client_a.post(
        "/api/v1/locations/shares",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"friendship_id": friendship_ab, "direction": "mutual"},
    )
    assert mutual.status_code == 400
    far_future = client_a.post(
        "/api/v1/locations/shares",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"friendship_id": friendship_ab,
              "expires_at": "2030-01-01T00:00:00Z"},
    )
    assert far_future.status_code == 400

    # Alice publishes her latest point.
    update = client_a.post(
        "/api/v1/locations/current",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"latitude": 51.5007, "longitude": -0.1246, "accuracy": 10, "battery_level": 80},
    )
    assert update.status_code == 200, update.text

    # Bob (consented recipient) sees it, exactly.
    bob_view = client_b.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_b}"}
    )
    assert bob_view.status_code == 200
    bob_locations = bob_view.json()["friends_locations"]
    assert len(bob_locations) == 1
    assert bob_locations[0]["friend_id"] == alice_id
    assert bob_locations[0]["latitude"] == 51.5007
    assert bob_locations[0]["is_stale"] is False
    assert bob_locations[0]["battery_level"] == 80

    # Carol (no consent) sees nothing.
    carol_view = client_c.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_c}"}
    )
    assert carol_view.status_code == 200
    assert carol_view.json()["friends_locations"] == []

    # IDOR: Carol cannot read the share record either.
    idor = client_c.get(
        f"/api/v1/locations/shares/{share_id}", headers={"Authorization": f"Bearer {token_c}"}
    )
    assert idor.status_code == 403

    # Only the owner can revoke; revocation cuts access immediately.
    not_owner = client_b.delete(
        f"/api/v1/locations/shares/{share_id}", headers={"Authorization": f"Bearer {token_b}"}
    )
    assert not_owner.status_code == 403
    revoke = client_a.delete(
        f"/api/v1/locations/shares/{share_id}", headers={"Authorization": f"Bearer {token_a}"}
    )
    assert revoke.status_code == 200
    after_revoke = client_b.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_b}"}
    )
    assert after_revoke.json()["friends_locations"] == []


def test_approximate_precision_is_reduced_server_side(make_client, create_user):
    create_user("alice@example.com", full_name="Alice")
    client_a, client_b = make_client(), make_client()
    login_a = client_a.post("/api/v1/auth/login",
                            json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"})
    token_a = login_a.json()["access_token"]

    friendship_ab, token_b = _make_friends(client_a, token_a, client_b, "bob@example.com")

    share = client_a.post(
        "/api/v1/locations/shares",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"friendship_id": friendship_ab, "precision_level": "approximate"},
    )
    assert share.status_code == 201
    client_a.post(
        "/api/v1/locations/current",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"latitude": 51.5007, "longitude": -0.1246},
    )

    view = client_b.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_b}"}
    ).json()["friends_locations"][0]
    # Rounded to the ~1 km grid before leaving the server.
    assert view["latitude"] == 51.5
    assert view["longitude"] == -0.12
    assert view["precision_level"] == "approximate"


def test_block_revokes_shares_and_hides_locations(make_client, create_user):
    create_user("alice@example.com", full_name="Alice")
    client_a, client_b = make_client(), make_client()
    login_a = client_a.post("/api/v1/auth/login",
                            json={"email": "alice@example.com", "password": "Str0ng!Passw0rd"})
    token_a = login_a.json()["access_token"]
    friendship_ab, token_b = _make_friends(client_a, token_a, client_b, "bob@example.com")

    client_a.post(
        "/api/v1/locations/shares",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"friendship_id": friendship_ab, "precision_level": "exact"},
    )
    client_a.post(
        "/api/v1/locations/current",
        headers={"Authorization": f"Bearer {token_a}"},
        json={"latitude": 51.5, "longitude": -0.12},
    )
    assert len(client_b.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_b}"}
    ).json()["friends_locations"]) == 1

    block = client_b.post(
        f"/api/v1/friendships/{friendship_ab}/block",
        headers={"Authorization": f"Bearer {token_b}"},
    )
    assert block.status_code == 200
    assert block.json()["status"] == "blocked"

    # Alice loses every view of Bob, and Bob of Alice.
    assert client_b.get(
        "/api/v1/locations/friends/latest", headers={"Authorization": f"Bearer {token_b}"}
    ).json()["friends_locations"] == []
    shares = client_a.get(
        "/api/v1/locations/shares", headers={"Authorization": f"Bearer {token_a}"}
    ).json()
    assert all(share["is_active"] is False for share in shares)


# ---------------------------------------------------------------------------
# Profile visibility (SC-03)
# ---------------------------------------------------------------------------

def test_profile_visibility_filtering(client, login, create_user):
    alice_id, token, _ = login()
    bob_id = create_user("bob@example.com", full_name="Bob Builder")
    headers = {"Authorization": f"Bearer {token}"}

    own = client.get(f"/api/v1/users/{alice_id}", headers=headers).json()
    assert own["is_full_profile"] is True and own["id"] == alice_id

    stranger = client.get(f"/api/v1/users/{bob_id}", headers=headers).json()
    assert stranger["is_full_profile"] is False
    assert stranger["display_name"] is None
    assert stranger["id"] is None

    # Non-existent accounts are indistinguishable from redacted ones.
    ghost = client.get("/api/v1/users/99999", headers=headers).json()
    assert ghost == stranger


# ---------------------------------------------------------------------------
# Forced password change gate + rate limiting + headers
# ---------------------------------------------------------------------------

def test_login_rate_limit_returns_429_with_retry_after(client, create_user):
    for index in range(11):
        response = client.post(
            "/api/v1/auth/login",
            json={"email": f"victim{index}@example.com", "password": "Str0ng!Passw0rd"},
        )
    assert response.status_code == 429
    assert response.headers.get("Retry-After")
    assert response.json()["error_code"] == "RATE_LIMIT_EXCEEDED"


def test_security_headers_present(client):
    response = client.get("/health")
    assert response.headers["X-Content-Type-Options"] == "nosniff"
    assert response.headers["X-Frame-Options"] == "DENY"
    assert "default-src 'none'" in response.headers["Content-Security-Policy"]
    assert response.headers["Referrer-Policy"] == "no-referrer"


def test_health_and_validation_error_shape(client):
    health = client.get("/health")
    assert health.status_code == 200
    assert health.json()["database"] == "connected"

    bad = client.post("/api/v1/auth/login", json={"email": "not-an-email"})
    assert bad.status_code == 422
    body = bad.json()
    assert body["status"] == 422
    assert body["error_code"] == "VALIDATION_FAILED"
    assert "fields" in body
