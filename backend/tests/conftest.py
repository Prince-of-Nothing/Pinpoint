"""
Pytest configuration.

Environment is pinned *before* any app import: the Settings singleton and the
database engine are created at import time, so the test environment must be
in place first. SQLite (file-based, per run) replaces PostgreSQL and the
in-memory rate limiter replaces Redis.
"""

import os
import tempfile

_TMP_DIR = tempfile.mkdtemp(prefix="pinpoint-tests-")

os.environ["ENVIRONMENT"] = "testing"
os.environ["DATABASE_URL"] = f"sqlite:///{_TMP_DIR}/test.db"
os.environ["REDIS_URL"] = "redis://localhost:6379/9"
os.environ["RATE_LIMIT_BACKEND"] = "memory"
os.environ["SECRET_KEY"] = "test-only-secret-key-not-for-production-0123456789abcdef"
os.environ["TRUSTED_HOSTS"] = "localhost,127.0.0.1,testserver"
os.environ["ALLOWED_ORIGINS"] = "http://localhost:3000"
os.environ["DEBUG"] = "false"
os.environ["ENABLE_DOCS"] = "true"

import pytest  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402


@pytest.fixture()
def app(tmp_path):
    """Fresh application + clean database per test."""
    from app.core.config import get_settings
    from app.db.session import Base, engine, init_db
    from app.services.rate_limit import get_rate_limiter

    get_settings.cache_clear()
    get_rate_limiter.cache_clear()  # isolated rate-limit budget per test

    Base.metadata.drop_all(bind=engine)
    init_db()

    from app.main import app as fastapi_app

    yield fastapi_app

    Base.metadata.drop_all(bind=engine)


@pytest.fixture()
def client(app):
    """Test client with lifespan startup (DB init) applied."""
    with TestClient(app, base_url="http://testserver") as test_client:
        yield test_client


@pytest.fixture()
def make_client(app):
    """Factory for isolated clients (own cookie jar)."""
    def _make() -> TestClient:
        return TestClient(app, base_url="http://testserver")

    return _make


@pytest.fixture()
def create_user():
    """Create a user directly in the database (bypassing signup, which is MVP-deferred)."""
    from app.db.session import SessionLocal
    from app.models.user import User, UserRole
    from app.services.security import get_password_hash, normalize_email

    def _create(email: str, password: str = "Str0ng!Passw0rd",
                full_name: str = "Test User", must_change_password: bool = False) -> int:
        with SessionLocal() as db:
            user = User(
                email=email,
                email_normalized=normalize_email(email),
                password_hash=get_password_hash(password),
                full_name=full_name,
                role=UserRole.RESIDENT,
                must_change_password=must_change_password,
            )
            db.add(user)
            db.commit()
            return user.id

    return _create


@pytest.fixture()
def login(client, create_user):
    """Create a user and log in, returning (user_id, token, cookies)."""
    def _login(email: str = "alice@example.com", password: str = "Str0ng!Passw0rd"):
        user_id = create_user(email, password)
        response = client.post(
            "/api/v1/auth/login", json={"email": email, "password": password}
        )
        assert response.status_code == 200, response.text
        body = response.json()
        return user_id, body["access_token"], dict(client.cookies)

    return _login
