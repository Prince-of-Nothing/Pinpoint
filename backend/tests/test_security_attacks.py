"""
Attack-oriented security tests (Task 7). Each test names the control it
verifies (see docs/reference/security-control-register.md).
"""

import logging

import pytest
from fastapi.testclient import TestClient
from sqlalchemy import func, select

from app.main import app
from app.models import User
from tests.helpers import DEFAULT_PASSWORD, befriend, create_account, login, register, share

SQLI_PAYLOADS = [
    "' OR '1'='1",
    "admin'--",
    "'; DROP TABLE users; --",
    "1 UNION SELECT password_hash FROM users",
]
XSS_PAYLOAD = '<script>alert("x")</script><img src=x onerror=alert(1)>'


# ------------------------------------------------------ SC-04 injection


@pytest.mark.parametrize("payload", SQLI_PAYLOADS)
def test_sc04_sql_injection_in_login_is_inert(client, payload):
    create_account(client, "alice")
    response = login(client, payload, payload)
    assert response.status_code == 401  # not 500, not a successful bypass


@pytest.mark.parametrize("payload", SQLI_PAYLOADS)
def test_sc04_sql_injection_in_identifiers_is_rejected(client, payload):
    alice = create_account(client, "alice")
    assert (
        client.post(
            "/api/v1/friendships", headers=alice.headers, json={"username": payload}
        ).status_code
        == 422
    )
    assert (
        client.get(f"/api/v1/location-shares/{payload}", headers=alice.headers).status_code == 404
    )


def test_sc04_sql_injection_in_free_text_is_stored_literally(client, db):
    alice = create_account(client, "alice")
    payload = "'; DROP TABLE users; --"
    response = client.patch("/api/v1/users/me", headers=alice.headers, json={"bio": payload})
    assert response.json()["bio"] == payload
    assert db.scalar(select(func.count()).select_from(User)) == 1  # table intact


# ------------------------------------------------------------ SC-05 XSS


def test_sc05_stored_xss_is_returned_as_json_data_only(client):
    alice, bob = create_account(client, "alice"), create_account(client, "bob")
    client.patch("/api/v1/users/me", headers=alice.headers, json={"display_name": XSS_PAYLOAD[:50]})
    befriend(client, alice, bob)
    response = client.get("/api/v1/users/alice", headers=bob.headers)
    assert response.headers["content-type"].startswith("application/json")
    assert response.headers["x-content-type-options"] == "nosniff"
    # The client renders this with textContent (see test_web_client.py)
    assert response.json()["display_name"] == XSS_PAYLOAD[:50]


# ------------------------------------------------------ SC-04 mass assignment


@pytest.mark.parametrize(
    ("method", "path", "body"),
    [
        (
            "post",
            "/api/v1/auth/register",
            {
                "username": "x_y_z",
                "email": "x@example.com",
                "password": DEFAULT_PASSWORD,
                "is_active": True,
            },
        ),
        ("patch", "/api/v1/users/me", {"role": "admin"}),
        ("patch", "/api/v1/users/me", {"email": "takeover@example.com"}),
        ("put", "/api/v1/locations/me", {"latitude": 1, "longitude": 1, "user_id": "victim"}),
        ("post", "/api/v1/check-ins", {"due_in_minutes": 30, "user_id": "victim"}),
    ],
)
def test_sc04_unknown_fields_are_rejected(client, method, path, body):
    alice = create_account(client, "alice")
    response = getattr(client, method)(path, headers=alice.headers, json=body)
    assert response.status_code == 422


# ------------------------------------------------------------ SC-02 IDOR matrix


def test_sc02_idor_matrix_every_foreign_id_is_denied(client):
    alice, bob, eve = (create_account(client, n) for n in ("alice", "bob", "eve"))
    fid = befriend(client, alice, bob)
    sid = share(client, alice, bob, precision="exact")["id"]
    cid = client.post(
        "/api/v1/check-ins", headers=alice.headers, json={"due_in_minutes": 30}
    ).json()["id"]
    session_id = client.get("/api/v1/auth/sessions", headers=alice.headers).json()[0]["id"]
    client.put(
        "/api/v1/locations/me", headers=alice.headers, json={"latitude": 45.0, "longitude": 25.0}
    )

    attempts = [
        ("post", f"/api/v1/friendships/{fid}/accept", None),
        ("post", f"/api/v1/friendships/{fid}/decline", None),
        ("delete", f"/api/v1/friendships/{fid}", None),
        ("get", f"/api/v1/location-shares/{sid}", None),
        ("patch", f"/api/v1/location-shares/{sid}", {"precision": "exact"}),
        ("delete", f"/api/v1/location-shares/{sid}", None),
        ("post", f"/api/v1/check-ins/{cid}/complete", None),
        ("post", f"/api/v1/check-ins/{cid}/cancel", None),
        ("delete", f"/api/v1/auth/sessions/{session_id}", None),
    ]
    for method, path, body in attempts:
        kwargs = {"headers": eve.headers}
        if body is not None:
            kwargs["json"] = body
        response = getattr(client, method)(path, **kwargs)
        assert response.status_code == 404, (method, path, response.status_code)

    # Eve's own views contain nothing of Alice's
    assert client.get("/api/v1/locations/visible", headers=eve.headers).json() == []
    assert client.get("/api/v1/check-ins/shared", headers=eve.headers).json() == []
    assert client.get("/api/v1/users/alice", headers=eve.headers).json()["relationship"] == "none"
    # And Alice's data is untouched
    assert (
        client.get(f"/api/v1/location-shares/{sid}", headers=alice.headers).json()["active"] is True
    )


def test_sc02_no_endpoint_accepts_a_user_id_for_location_reads(client):
    paths = app.openapi()["paths"]
    location_paths = [p for p in paths if p.startswith("/api/v1/locations")]
    assert sorted(location_paths) == ["/api/v1/locations/me", "/api/v1/locations/visible"]


# ------------------------------------------------------------- SC-05 CSRF / CORS


def test_sc05_cross_origin_requests_are_not_granted_cors(client):
    response = client.options(
        "/api/v1/auth/refresh",
        headers={"Origin": "https://evil.example", "Access-Control-Request-Method": "POST"},
    )
    assert "access-control-allow-origin" not in response.headers


def test_sc05_cookie_refresh_without_session_csrf_token_fails(client):
    register(client, "alice")
    login(client, "alice", delivery="cookie")
    # An attacker page can make the browser send the cookie but cannot read the CSRF token
    assert client.post("/api/v1/auth/refresh").status_code == 403
    # Copying the CSRF *cookie* name with a guessed value does not help either
    assert client.post("/api/v1/auth/refresh", headers={"X-CSRF-Token": "guess"}).status_code == 403


def test_sc05_bearer_token_is_never_accepted_from_cookies_or_query(client):
    alice = create_account(client, "alice")
    assert (
        client.get("/api/v1/users/me", params={"access_token": alice.access_token}).status_code
        == 401
    )
    client.cookies.set("access_token", alice.access_token)
    assert client.get("/api/v1/users/me").status_code == 401


# ----------------------------------------------------------- SC-04 safe errors


def test_sc04_unhandled_errors_do_not_leak_details(monkeypatch):
    def boom(*_args, **_kwargs):
        raise RuntimeError("psycopg: password authentication failed for user secret_db_user")

    with TestClient(app, raise_server_exceptions=False) as client:
        alice = create_account(client, "alice")
        monkeypatch.setattr("app.api.users.active_user_by_username", boom)
        response = client.get("/api/v1/users/bob", headers=alice.headers)
    assert response.status_code == 500
    body = response.json()
    assert body["code"] == "INTERNAL_ERROR" and body["detail"] == "An unexpected error occurred."
    assert "secret_db_user" not in response.text and "Traceback" not in response.text


def test_sc04_validation_errors_do_not_echo_input(client):
    response = register(client, "x", password="SuperSecretValue!!")
    assert response.status_code == 422
    assert "SuperSecretValue" not in response.text


def test_sc04_method_not_allowed_uses_problem_format(client):
    response = client.delete("/api/v1/health")
    assert response.status_code == 405
    assert response.json()["code"] == "METHOD_NOT_ALLOWED"


def test_sc04_malformed_json_is_rejected_cleanly(client):
    alice = create_account(client, "alice")
    response = client.put(
        "/api/v1/locations/me",
        headers={**alice.headers, "content-type": "application/json"},
        content=b'{"latitude": 1, "longitude":',
    )
    assert response.status_code == 422


# -------------------------------------------------------- SC-01 secrets in logs


def test_sc01_passwords_and_tokens_never_reach_logs(client, caplog):
    caplog.set_level(logging.DEBUG)
    register(client, "alice")
    login(client, "alice", "a wrong password value")
    tokens = login(client, "alice").json()
    text = caplog.text
    assert DEFAULT_PASSWORD not in text and "a wrong password value" not in text
    assert tokens["access_token"] not in text and tokens["refresh_token"] not in text


# ---------------------------------------------------- SC-05 headers everywhere


@pytest.mark.parametrize("path", ["/", "/api/v1/health", "/api/v1/users/me", "/api/v1/missing"])
def test_sc05_security_headers_on_every_response(client, path):
    headers = client.get(path).headers
    for name in (
        "content-security-policy",
        "x-content-type-options",
        "x-frame-options",
        "referrer-policy",
    ):
        assert name in headers, (path, name)
