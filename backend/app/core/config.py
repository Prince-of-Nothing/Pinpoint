"""
Application configuration and settings.
Loads environment variables and provides validated configuration.
"""

from pydantic_settings import BaseSettings
from typing import List
from functools import lru_cache


class Settings(BaseSettings):
    """Application settings loaded from environment variables."""
    
    # Database
    database_url: str = "postgresql://postgres:postgres@localhost:5432/always_together"
    
    # Redis
    redis_url: str = "redis://localhost:6379/0"
    
    # Security
    secret_key: str = "change-this-secret-key-in-production-min-32-characters-long"
    algorithm: str = "HS256"
    access_token_expire_minutes: int = 30
    refresh_token_expire_days: int = 7
    
    # Session Settings
    session_cookie_name: str = "session_id"
    session_max_age_seconds: int = 10800  # 3 hours
    session_idle_timeout_seconds: int = 1800  # 30 minutes
    
    # Rate Limiting
    rate_limit_requests: int = 10
    rate_limit_window_seconds: int = 60
    
    # Account Lockout
    lockout_threshold: int = 5
    lockout_duration_minutes: int = 15
    
    # CORS
    allowed_origins: List[str] = ["http://localhost:3000", "http://localhost:8080"]
    
    # Application
    app_name: str = "Always Together API"
    debug: bool = True
    
    class Config:
        env_file = ".env"
        case_sensitive = False


@lru_cache()
def get_settings() -> Settings:
    """Get cached settings instance."""
    return Settings()
