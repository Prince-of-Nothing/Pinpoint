"""
Rate limiting.

Design patterns applied:
- Strategy + Adapter: one RateLimiter interface with two interchangeable
  implementations — an in-process sliding window (development/tests) and a
  Redis sliding/fixed window (production, shared across workers).
- Factory: get_rate_limiter() picks the implementation from configuration and
  falls back safely (fail closed on limits, fail open on infrastructure only
  where documented).
"""

import logging
import threading
import time
from collections import deque
from dataclasses import dataclass
from functools import lru_cache
from typing import Deque, Dict, Protocol

from app.core.config import get_settings

logger = logging.getLogger(__name__)

settings = get_settings()


@dataclass(frozen=True)
class RateLimitResult:
    """Outcome of one rate-limit hit."""

    allowed: bool
    limit: int
    remaining: int
    retry_after: int  # seconds; 0 when allowed
    reset_at: int  # unix timestamp when the window frees up


class RateLimiter(Protocol):
    """Common interface for rate-limit backends."""

    def hit(self, key: str, limit: int, window_seconds: int) -> RateLimitResult:
        """Record one request for `key` and report whether it is allowed."""
        ...


class InMemoryRateLimiter:
    """
    Per-process sliding-window limiter.

    Limitation: state is per worker process; use the Redis backend when the
    API runs with multiple workers or instances.
    """

    def __init__(self):
        self._hits: Dict[str, Deque[float]] = {}
        self._lock = threading.Lock()

    def hit(self, key: str, limit: int, window_seconds: int) -> RateLimitResult:
        now = time.monotonic()
        cutoff = now - window_seconds

        with self._lock:
            bucket = self._hits.setdefault(key, deque())
            while bucket and bucket[0] <= cutoff:
                bucket.popleft()

            if len(bucket) >= limit:
                retry_after = max(1, int(bucket[0] + window_seconds - now) + 1)
                return RateLimitResult(
                    allowed=False,
                    limit=limit,
                    remaining=0,
                    retry_after=retry_after,
                    reset_at=int(time.time()) + retry_after,
                )

            bucket.append(now)
            remaining = limit - len(bucket)
            reset_at = int(time.time()) + window_seconds

            # Opportunistic cleanup so the dict cannot grow unbounded.
            if len(self._hits) > 10_000:
                self._prune(cutoff)

            return RateLimitResult(
                allowed=True,
                limit=limit,
                remaining=remaining,
                retry_after=0,
                reset_at=reset_at,
            )

    def _prune(self, cutoff: float) -> None:
        empty = [key for key, bucket in self._hits.items() if not bucket or bucket[-1] <= cutoff]
        for key in empty:
            self._hits.pop(key, None)


class RedisRateLimiter:
    """Shared rate limiter backed by Redis (INCR + EXPIRE fixed window)."""

    def __init__(self, redis_url: str):
        import redis  # lazy import: not required for tests/dev

        self._redis = redis.Redis.from_url(redis_url, decode_responses=True)

    def hit(self, key: str, limit: int, window_seconds: int) -> RateLimitResult:
        redis_key = f"rate_limit:{key}:{int(time.time() // window_seconds)}"
        try:
            count = self._redis.incr(redis_key)
            if count == 1:
                self._redis.expire(redis_key, window_seconds, nx=True)
        except Exception as exc:  # Redis down: fail closed, do not bypass limits
            logger.error("Redis rate limiter unavailable: %s", exc)
            raise

        retry_after = window_seconds if count > limit else 0
        return RateLimitResult(
            allowed=count <= limit,
            limit=limit,
            remaining=max(0, limit - count),
            retry_after=retry_after,
            reset_at=int(time.time()) + retry_after,
        )


@lru_cache()
def get_rate_limiter() -> RateLimiter:
    """Select the configured rate-limit backend (factory, cached)."""
    if settings.rate_limit_backend == "redis":
        try:
            limiter = RedisRateLimiter(settings.redis_url)
            limiter.hit("warmup", 5, 60)  # connectivity probe
            logger.info("Rate limiting: redis backend active")
            return limiter
        except Exception as exc:
            logger.warning(
                "REDIS backend configured but unavailable (%s); "
                "falling back to per-process in-memory limiter.", exc,
            )
    return InMemoryRateLimiter()
