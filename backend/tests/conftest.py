"""Shared fixtures. Tests run against in-memory SQLite and an in-memory rate limiter."""

import os

os.environ.update(
    {
        "ENVIRONMENT": "test",
        "DATABASE_URL": "sqlite://",
        "RATE_LIMIT_BACKEND": "memory",
        "COOKIE_SECURE": "false",  # the test client talks plain http://testserver
        # Cheap Argon2 parameters keep the suite fast; production uses the defaults
        "PASSWORD_HASH_TIME_COST": "1",
        "PASSWORD_HASH_MEMORY_KIB": "1024",
        "PASSWORD_HASH_PARALLELISM": "1",
    }
)

import pytest  # noqa: E402
from fastapi.testclient import TestClient  # noqa: E402

from app.db.base import Base  # noqa: E402
from app.db.session import SessionLocal, engine  # noqa: E402
from app.main import app  # noqa: E402
from app.services.ratelimit import get_backend  # noqa: E402


@pytest.fixture(autouse=True)
def _fresh_database():
    Base.metadata.drop_all(engine)
    Base.metadata.create_all(engine)
    get_backend().reset()
    yield


@pytest.fixture
def db():
    session = SessionLocal()
    try:
        yield session
    finally:
        session.close()


@pytest.fixture
def client():
    with TestClient(app) as test_client:
        yield test_client
