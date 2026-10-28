"""Application core: health, error format, security headers, body limits, config guards."""

import pytest
from pydantic import ValidationError

from app.core.config import DEV_SECRET_KEY, Settings


def test_health(client):
    response = client.get("/api/v1/health")
    assert response.status_code == 200
    assert response.json()["status"] == "ok"


def test_unknown_route_returns_problem_details(client):
    response = client.get("/api/v1/does-not-exist")
    assert response.status_code == 404
    assert response.headers["content-type"].startswith("application/problem+json")
    body = response.json()
    assert body["code"] == "NOT_FOUND"
    assert body["status"] == 404


def test_security_headers_present(client):
    response = client.get("/api/v1/health")
    headers = response.headers
    assert headers["x-content-type-options"] == "nosniff"
    assert headers["x-frame-options"] == "DENY"
    assert headers["referrer-policy"] == "no-referrer"
    assert "default-src 'self'" in headers["content-security-policy"]
    assert headers["cache-control"] == "no-store"


def test_oversized_body_rejected(client):
    response = client.post(
        "/api/v1/health", content=b"x" * (65 * 1024), headers={"content-type": "application/json"}
    )
    assert response.status_code == 413
    assert response.json()["code"] == "PAYLOAD_TOO_LARGE"


@pytest.mark.parametrize(
    "overrides",
    [
        {"secret_key": DEV_SECRET_KEY},
        {"secret_key": "short"},
        {"secret_key": "x" * 40, "debug": True},
        {"secret_key": "x" * 40, "cookie_secure": False},
    ],
)
def test_production_refuses_insecure_settings(overrides):
    with pytest.raises(ValidationError):
        Settings(environment="production", **overrides)


def test_production_accepts_secure_settings():
    settings = Settings(environment="production", secret_key="x" * 40, cookie_secure=True)
    assert settings.environment == "production"
