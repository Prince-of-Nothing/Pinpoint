"""
Application configuration loaded from environment variables.

Security-relevant defaults are safe for local development only; production
start-up is refused when insecure values (default secret, debug mode,
non-secure cookies) are detected.
"""

from functools import lru_cache
from typing import Annotated, Literal

from pydantic import Field, field_validator, model_validator
from pydantic_settings import BaseSettings, NoDecode, SettingsConfigDict

# Development placeholder only; production start-up rejects it (see validator below).
DEV_SECRET_KEY = "dev-only-insecure-secret-key-change-me-0123456789"  # noqa: S105


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", case_sensitive=False, extra="ignore")

    # Application
    app_name: str = "Always Together API"
    environment: Literal["development", "test", "production"] = "development"
    debug: bool = False

    # Database / Redis
    database_url: str = "postgresql+psycopg://postgres:postgres@localhost:5432/always_together"
    redis_url: str = "redis://localhost:6379/0"

    # Tokens
    secret_key: str = DEV_SECRET_KEY
    jwt_algorithm: Literal["HS256", "HS384", "HS512"] = "HS256"
    jwt_issuer: str = "always-together-api"
    jwt_audience: str = "always-together-clients"
    access_token_expire_minutes: int = Field(default=15, ge=1, le=60)
    refresh_token_expire_days: int = Field(default=7, ge=1, le=30)

    # Sessions
    session_absolute_timeout_hours: int = Field(default=12, ge=1, le=24 * 30)
    session_idle_timeout_minutes: int = Field(default=60 * 24, ge=5)
    max_sessions_per_user: int = Field(default=10, ge=1)

    # Cookies (web client)
    cookie_secure: bool = True
    refresh_cookie_name: str = "at_refresh"
    csrf_cookie_name: str = "at_csrf"
    csrf_header_name: str = "X-CSRF-Token"

    # Account lockout
    lockout_threshold: int = Field(default=5, ge=1)
    lockout_duration_minutes: int = Field(default=15, ge=1)

    # Rate limiting
    rate_limit_enabled: bool = True
    rate_limit_backend: Literal["redis", "memory"] = "redis"

    # Location freshness thresholds
    location_live_seconds: int = Field(default=5 * 60, ge=30)
    location_dead_seconds: int = Field(default=60 * 60, ge=60)
    approximate_grid_degrees: float = Field(default=0.01, gt=0, le=1)

    # Request limits
    max_request_body_bytes: int = Field(default=64 * 1024, ge=1024)

    # CORS (only needed if the web client is served from another origin)
    allowed_origins: Annotated[list[str], NoDecode] = []

    @field_validator("allowed_origins", mode="before")
    @classmethod
    def _split_origins(cls, value):
        if isinstance(value, str):
            return [origin.strip() for origin in value.split(",") if origin.strip()]
        return value

    @model_validator(mode="after")
    def _refuse_insecure_production(self) -> "Settings":
        if self.environment == "production":
            problems = []
            if self.secret_key == DEV_SECRET_KEY or len(self.secret_key) < 32:
                problems.append(
                    "SECRET_KEY must be set to a random value of at least 32 characters"
                )
            if self.debug:
                problems.append("DEBUG must be false")
            if not self.cookie_secure:
                problems.append("COOKIE_SECURE must be true")
            if problems:
                raise ValueError("Insecure production configuration: " + "; ".join(problems))
        return self


@lru_cache
def get_settings() -> Settings:
    return Settings()
