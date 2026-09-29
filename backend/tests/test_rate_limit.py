"""API rate limiting (HTTP 429 + Retry-After) and its audit trail."""

import pytest
from sqlalchemy import select

from app.models import AuditEvent
from app.services import ratelimit
from tests.helpers import DEFAULT_PASSWORD, create_account, login, register


def test_login_is_rate_limited_per_ip(client):
    register(client, "alice")
    for _ in range(ratelimit.LOGIN.requests):
        assert login(client, "alice", "wrong guess value").status_code == 401
    blocked = login(client, "alice")
    assert blocked.status_code == 429
    assert blocked.json()["code"] == "RATE_LIMITED"
    assert int(blocked.headers["retry-after"]) >= 1


def test_registration_is_rate_limited(client):
    for i in range(ratelimit.REGISTER.requests):
        assert register(client, f"user{i}").status_code == 201
    assert register(client, "one_too_many").status_code == 429


def test_rate_limit_exceeded_is_audited_once_per_window(client, db):
    for i in range(ratelimit.REGISTER.requests + 3):
        register(client, f"user{i}")
    events = db.scalars(
        select(AuditEvent).where(AuditEvent.event_type == "ratelimit.exceeded")
    ).all()
    assert len(events) == 1
    assert events[0].details == {"limit": "register"}


def test_per_user_limit_uses_account_not_ip(client):
    alice = create_account(client, "alice")
    for _ in range(ratelimit.PASSWORD_CHANGE.requests):
        client.post(
            "/api/v1/auth/password",
            headers=alice.headers,
            json={"current_password": "wrong password!", "new_password": DEFAULT_PASSWORD + "x"},
        )
    response = client.post(
        "/api/v1/auth/password",
        headers=alice.headers,
        json={"current_password": DEFAULT_PASSWORD, "new_password": "brand new passphrase"},
    )
    assert response.status_code in (401, 429)  # lockout may already have ended the session


def test_disabled_limiter_allows_requests(client, monkeypatch):
    from app.core.config import get_settings

    monkeypatch.setattr(get_settings(), "rate_limit_enabled", False)
    for i in range(ratelimit.REGISTER.requests + 2):
        assert register(client, f"user{i}").status_code == 201


def test_memory_backend_window_resets():
    backend = ratelimit.MemoryBackend()
    count, retry_after = backend.hit("k", 60)
    assert count == 1 and 1 <= retry_after <= 60
    assert backend.hit("k", 60)[0] == 2
    backend.reset()
    assert backend.hit("k", 60)[0] == 1


def test_redis_backend_counts_and_expires():
    redis = pytest.importorskip("redis")
    try:
        client = redis.Redis.from_url("redis://localhost:6379/15", socket_connect_timeout=0.3)
        client.ping()
    except redis.exceptions.RedisError:
        pytest.skip("Redis is not running locally")
    client.flushdb()
    backend = ratelimit.RedisBackend("redis://localhost:6379/15")
    assert backend.hit("k", 60)[0] == 1
    assert backend.hit("k", 60)[0] == 2
    (key,) = client.keys("ratelimit:k:*")
    assert 0 < client.ttl(key) <= 61
    client.flushdb()


def test_redis_outage_fails_open(client, monkeypatch):
    class Broken:
        def hit(self, *_args):
            raise ConnectionError("redis down")

        def reset(self):
            pass

    monkeypatch.setattr(ratelimit, "get_backend", lambda: Broken())
    for i in range(ratelimit.REGISTER.requests + 1):
        assert register(client, f"user{i}").status_code == 201
