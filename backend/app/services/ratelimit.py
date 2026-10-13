"""
Fixed-window rate limiting backed by Redis (shared across API workers) or by
process memory (tests and single-process development).

If Redis is unreachable the limiter fails open and logs an error: account
lockout still protects credentials, and an outage of the cache must not take
the whole safety service down.
"""

import logging
import threading
import time
from dataclasses import dataclass
from functools import lru_cache
from typing import Literal

from fastapi import Depends, Request
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer

from app.core.config import get_settings
from app.core.errors import ProblemError
from app.db.session import SessionLocal
from app.services import audit
from app.services.tokens import decode_access_token

logger = logging.getLogger("app.ratelimit")


@dataclass(frozen=True)
class Limit:
    name: str
    requests: int
    window_seconds: int
    per: Literal["ip", "user"]


# Central, documented table of limits (see docs/reference/security-control-register.md)
REGISTER = Limit("register", 5, 3600, "ip")
LOGIN = Limit("login", 10, 60, "ip")
REFRESH = Limit("refresh", 30, 60, "ip")
PASSWORD_CHANGE = Limit("password_change", 5, 900, "user")
FRIEND_REQUEST = Limit("friend_request", 20, 3600, "user")
LOCATION_UPDATE = Limit("location_update", 60, 60, "user")
SHARE_CHANGE = Limit("share_change", 30, 3600, "user")
USER_LOOKUP = Limit("user_lookup", 60, 60, "user")


class MemoryBackend:
    def __init__(self) -> None:
        self._counts: dict[str, tuple[int, int]] = {}
        self._lock = threading.Lock()

    def hit(self, key: str, window_seconds: int) -> tuple[int, int]:
        now = int(time.time())
        window = now // window_seconds
        with self._lock:
            start, count = self._counts.get(key, (window, 0))
            if start != window:
                start, count = window, 0
            count += 1
            self._counts[key] = (start, count)
        retry_after = (window + 1) * window_seconds - now
        return count, retry_after

    def reset(self) -> None:
        with self._lock:
            self._counts.clear()


class RedisBackend:
    def __init__(self, url: str) -> None:
        import redis

        self._client = redis.Redis.from_url(url, socket_timeout=0.5, socket_connect_timeout=0.5)

    def hit(self, key: str, window_seconds: int) -> tuple[int, int]:
        now = int(time.time())
        window = now // window_seconds
        redis_key = f"ratelimit:{key}:{window}"
        pipe = self._client.pipeline()
        pipe.incr(redis_key)
        pipe.expire(redis_key, window_seconds + 1)
        count, _ = pipe.execute()
        return int(count), (window + 1) * window_seconds - now

    def reset(self) -> None:  # pragma: no cover - only used by tests with the memory backend
        pass


@lru_cache
def get_backend() -> MemoryBackend | RedisBackend:
    settings = get_settings()
    if settings.rate_limit_backend == "redis":
        return RedisBackend(settings.redis_url)
    return MemoryBackend()


_bearer = HTTPBearer(auto_error=False)


def rate_limit(limit: Limit):
    """Dependency factory: raise 429 with Retry-After once ``limit`` is exceeded."""

    def dependency(
        request: Request,
        credentials: HTTPAuthorizationCredentials | None = Depends(_bearer),
    ) -> None:
        settings = get_settings()
        if not settings.rate_limit_enabled:
            return
        subject = audit.client_ip(request) or "unknown"
        if limit.per == "user":
            claims = decode_access_token(credentials.credentials) if credentials else None
            if claims is None:
                return  # unauthenticated: the auth dependency will reject the request
            subject = claims["sub"]
        try:
            count, retry_after = get_backend().hit(f"{limit.name}:{subject}", limit.window_seconds)
        except Exception:  # noqa: BLE001 - fail open on cache outage (documented)
            logger.exception("Rate limiter unavailable; allowing request")
            return
        if count > limit.requests:
            if count == limit.requests + 1:  # audit once per window, not on every rejected hit
                with SessionLocal() as db:
                    audit.record(
                        db,
                        "ratelimit.exceeded",
                        outcome="denied",
                        actor_id=subject if limit.per == "user" else None,
                        request=request,
                        limit=limit.name,
                    )
                    db.commit()
            raise ProblemError(
                429,
                "RATE_LIMITED",
                "Too many requests. Please try again later.",
                headers={"Retry-After": str(max(retry_after, 1))},
            )

    return Depends(dependency)
