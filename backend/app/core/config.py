"""
Application configuration and settings.

Design patterns applied:
- Singleton: one cached Settings instance via get_settings().
- Fail-safe defaults (Saltzer & Schroeder): no working secret ships in code.
  Production refuses to boot with a missing, short, or placeholder secret;
  development gets a random per-process secret instead of a fixed default.
"""

import logging
import secrets
from functools import lru_cache
from typing import Annotated, List

from pydantic import field_validator, model_validator
from pydantic_settings import BaseSettings, NoDecode, SettingsConfigDict

logger = logging.getLogger(__name__)

VALID_ENVIRONMENTS = {"development", "testing", "production"}


class Settings(BaseSettings):
    """Application settings loaded from environment variables."""

    # Environment -----------------------------------------------------------
    environment: str = "development"
    debug: bool = False  # fail-safe default: no verbose errors by default

    # Database --------------------------------------------------------------
    database_url: str = "postgresql://postgres:postgres@localhost:5432/always_together"

    # Redis -----------------------------------------------------------------
    redis_url: str = "redis://localhost:6379/0"
    rate_limit_backend: str = "memory"  # memory | redis

    # Security --------------------------------------------------------------
    # Empty by default on purpose. See get_settings() for dev/test fallback.
    secret_key: str = ""
    algorithm: str = "HS256"  # validated against an allowlist, "none" rejected
    access_token_expire_minutes: int = 15  # short-lived, per API security guide
    refresh_token_expire_days: int = 7

    # Sessions --------------------------------------------------------------
    session_cookie_name: str = "session_id"
    session_max_age_seconds: int = 10800  # 3 hours absolute lifetime
    session_idle_timeout_seconds: int = 1800  # 30 minutes inactivity timeout
    csrf_cookie_name: str = "csrf_token"

    # Rate limiting ---------------------------------------------------------
    rate_limit_requests: int = 10
    rate_limit_window_seconds: int = 60

    # Account lockout -------------------------------------------------------
    lockout_threshold: int = 5
    lockout_duration_minutes: int = 15

    # Transport / browser hardening -----------------------------------------
    # NoDecode: env vars are comma-separated (not JSON); split_csv_lists below
    # handles the parsing so ALLOWED_ORIGINS=a,b works as documented.
    allowed_origins: Annotated[List[str], NoDecode] = ["http://localhost:3000", "http://localhost:8080"]
    trusted_hosts: Annotated[List[str], NoDecode] = ["localhost", "127.0.0.1", "testserver"]

    # Application -----------------------------------------------------------
    app_name: str = "Always Together API"
    enable_docs: bool = True  # forced off in production unless set explicitly

    model_config = SettingsConfigDict(
        env_file=".env",
        case_sensitive=False,
        extra="ignore",
    )

    # Validators ------------------------------------------------------------

    @field_validator("allowed_origins", "trusted_hosts", mode="before")
    @classmethod
    def split_csv_lists(cls, value):
        """Accept comma-separated strings from env vars as well as JSON lists."""
        if isinstance(value, str):
            stripped = value.strip()
            if stripped.startswith("["):
                import json

                try:
                    parsed = json.loads(stripped)
                    if isinstance(parsed, list):
                        return parsed
                except ValueError:
                    pass
            return [item.strip() for item in stripped.split(",") if item.strip()]
        return value

    @field_validator("environment")
    @classmethod
    def validate_environment(cls, value: str) -> str:
        if value not in VALID_ENVIRONMENTS:
            raise ValueError(f"ENVIRONMENT must be one of {sorted(VALID_ENVIRONMENTS)}")
        return value

    @field_validator("algorithm")
    @classmethod
    def validate_algorithm(cls, value: str) -> str:
        # Allowlist: rejects "none" and anything unexpected (JWT alg confusion).
        if value not in {"HS256"}:
            raise ValueError("ALGORITHM must be HS256")
        return value

    @field_validator("rate_limit_backend")
    @classmethod
    def validate_rate_limit_backend(cls, value: str) -> str:
        if value not in {"memory", "redis"}:
            raise ValueError("RATE_LIMIT_BACKEND must be 'memory' or 'redis'")
        return value

    @field_validator("access_token_expire_minutes")
    @classmethod
    def validate_token_lifetime(cls, value: int) -> int:
        if not 1 <= value <= 60:
            raise ValueError("ACCESS_TOKEN_EXPIRE_MINUTES must be between 1 and 60")
        return value

    @model_validator(mode="after")
    def enforce_production_security(self) -> "Settings":
        """Refuse insecure configuration in production (fail fast, fail safe)."""
        if self.is_production:
            secret = self.secret_key or ""
            placeholder = secret.lower().startswith(("change", "your-", "dev-", "example"))
            if len(secret) < 32 or placeholder:
                raise ValueError(
                    "SECRET_KEY must be a unique random value of at least 32 characters "
                    "in production (placeholder values are rejected)."
                )
            if self.debug:
                raise ValueError("DEBUG must be false in production.")
        return self

    @property
    def is_production(self) -> bool:
        return self.environment == "production"

    @property
    def docs_enabled(self) -> bool:
        """Interactive API docs are disabled in production by default."""
        return self.enable_docs and not self.is_production


@lru_cache()
def get_settings() -> Settings:
    """
    Get the cached settings instance (Singleton).

    If no SECRET_KEY is configured outside production, generate a random
    per-process key so development/testing never fall back to a shared,
    guessable secret. Production configuration without a real key raises at
    startup (see Settings.enforce_production_security).
    """
    try:
        settings = Settings()
    except Exception as exc:  # pragma: no cover - exercised via env validation
        logger.error("Invalid configuration: %s", exc)
        raise

    if not settings.secret_key:
        if settings.is_production:  # pragma: no cover - guarded by validator
            raise RuntimeError("SECRET_KEY is required in production.")
        settings.secret_key = secrets.token_urlsafe(48)
        logger.warning(
            "SECRET_KEY not configured; generated an ephemeral per-process key. "
            "Sessions/tokens will not survive a restart. Set SECRET_KEY in .env."
        )

    return settings
