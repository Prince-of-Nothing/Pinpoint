"""Services module initialization.

Back-compatible exports for the low-level primitives; higher-level services
(AuthService, policies, rate limiting) are imported from their own modules.
"""

from app.services.security import (
    verify_password,
    get_password_hash,
    generate_session_token,
    generate_csrf_token,
    verify_csrf_token,
    normalize_email,
    is_weak_password,
    hash_token,
    pseudonymize_email,
)
from app.services.password_policy import DEFAULT_PASSWORD_POLICY, PasswordPolicy
from app.services.token_service import TokenService, get_token_service
from app.services.rate_limit import RateLimiter, RateLimitResult, get_rate_limiter
from app.services.auth_service import (
    AuthService,
    AuthenticationError,
    AuthErrorCode,
    LoginResult,
)

__all__ = [
    # Primitives
    "verify_password",
    "get_password_hash",
    "generate_session_token",
    "generate_csrf_token",
    "verify_csrf_token",
    "normalize_email",
    "is_weak_password",
    "hash_token",
    "pseudonymize_email",
    # Policy / tokens
    "DEFAULT_PASSWORD_POLICY",
    "PasswordPolicy",
    "TokenService",
    "get_token_service",
    # Rate limiting
    "RateLimiter",
    "RateLimitResult",
    "get_rate_limiter",
    # Authentication facade
    "AuthService",
    "AuthenticationError",
    "AuthErrorCode",
    "LoginResult",
]
