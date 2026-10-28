"""
API dependencies (Dependency Injection).

Design patterns applied:
- Dependency Injection: every router receives its repositories/services via
  FastAPI Depends, so policies can be swapped in tests without touching the
  endpoints.
- Decorator: rate_limit() and csrf_protect() wrap endpoints with cross-cutting
  security behaviour by composing them as dependencies.
- Complete mediation: get_auth_context() is the single authentication path for
  both credential types (Bearer JWT and session cookie) and applies the same
  revocation, expiry, idle-timeout, and account-state checks to each.
"""

from dataclasses import dataclass
from datetime import timedelta
from typing import Callable, Optional

from fastapi import Depends, HTTPException, Request, status
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy.orm import Session as OrmSession

from app.core.config import get_settings
from app.core.timeutils import ensure_utc, utcnow
from app.db.session import get_db
from app.models.user import Session as SessionModel
from app.models.user import User, UserRole
from app.repositories import (
    FriendshipRepository,
    LocationShareRepository,
    LocationUpdateRepository,
    LoginAttemptRepository,
    SessionRepository,
    UserRepository,
)
from app.services.audit import AuditLogger, DatabaseAuditHandler, StructuredLogAuditHandler
from app.services.auth_service import AuthService
from app.services.authorization import RelationshipService
from app.services.rate_limit import get_rate_limiter
from app.services.security import hash_token, verify_csrf_token
from app.services.token_service import get_token_service

settings = get_settings()
bearer_scheme = HTTPBearer(auto_error=False)


# ---------------------------------------------------------------------------
# Composable services (one instance per request)
# ---------------------------------------------------------------------------

def get_user_repository(db: OrmSession = Depends(get_db)) -> UserRepository:
    return UserRepository(db)


def get_session_repository(db: OrmSession = Depends(get_db)) -> SessionRepository:
    return SessionRepository(db)


def get_login_attempt_repository(db: OrmSession = Depends(get_db)) -> LoginAttemptRepository:
    return LoginAttemptRepository(db)


def get_friendship_repository(db: OrmSession = Depends(get_db)) -> FriendshipRepository:
    return FriendshipRepository(db)


def get_location_share_repository(db: OrmSession = Depends(get_db)) -> LocationShareRepository:
    return LocationShareRepository(db)


def get_location_update_repository(db: OrmSession = Depends(get_db)) -> LocationUpdateRepository:
    return LocationUpdateRepository(db)


def get_audit_logger(db: OrmSession = Depends(get_db)) -> AuditLogger:
    """Observer subject wired with the log + database handlers."""
    return AuditLogger([StructuredLogAuditHandler(), DatabaseAuditHandler(db)])


def get_auth_service(
    db: OrmSession = Depends(get_db),
    users: UserRepository = Depends(get_user_repository),
    sessions: SessionRepository = Depends(get_session_repository),
    attempts: LoginAttemptRepository = Depends(get_login_attempt_repository),
    audit: AuditLogger = Depends(get_audit_logger),
) -> AuthService:
    return AuthService(users=users, sessions=sessions, attempts=attempts, audit=audit)


def get_relationship_service(
    friendships: FriendshipRepository = Depends(get_friendship_repository),
) -> RelationshipService:
    return RelationshipService(friendships)


# ---------------------------------------------------------------------------
# Request helpers
# ---------------------------------------------------------------------------

def client_ip(request: Request) -> str:
    """Best-effort client IP (behind a trusted proxy, use X-Forwarded-For)."""
    forwarded = request.headers.get("x-forwarded-for")
    if forwarded:
        return forwarded.split(",")[0].strip()
    return request.client.host if request.client else "unknown"


def rate_limit(scope: str, limit: int, window_seconds: int) -> Callable:
    """
    Decorator-style dependency factory: 429s requests over `scope`'s budget.

    Limitations: keyed by client IP (shared NATs share a budget); per-process
    for the memory backend — use the Redis backend in production.
    """
    async def _rate_limit_dependency(request: Request) -> None:
        limiter = get_rate_limiter()
        result = limiter.hit(f"{scope}:{client_ip(request)}", limit, window_seconds)
        if not result.allowed:
            raise HTTPException(
                status_code=status.HTTP_429_TOO_MANY_REQUESTS,
                detail={
                    "code": "RATE_LIMIT_EXCEEDED",
                    "message": f"Too many requests. Please retry in {result.retry_after} seconds.",
                },
                headers={
                    "Retry-After": str(result.retry_after),
                    "X-RateLimit-Limit": str(result.limit),
                    "X-RateLimit-Remaining": "0",
                    "X-RateLimit-Reset": str(result.reset_at),
                },
            )

    return _rate_limit_dependency


# ---------------------------------------------------------------------------
# Authentication
# ---------------------------------------------------------------------------

@dataclass(frozen=True)
class AuthContext:
    """The authenticated caller plus the session that authorised the request."""

    user: User
    session: SessionModel
    via: str  # "bearer" or "cookie"


def _session_is_current(session: SessionModel) -> bool:
    """Revocation, absolute expiry, and idle timeout for a session row."""
    now = utcnow()
    if session.revoked_at is not None:
        return False
    expires_at = ensure_utc(session.expires_at)
    if expires_at is None or expires_at <= now:
        return False
    last_activity = ensure_utc(session.last_activity_at)
    if last_activity is not None and \
            now - last_activity > timedelta(seconds=settings.session_idle_timeout_seconds):
        return False
    return True


def get_auth_context(
    request: Request,
    db: OrmSession = Depends(get_db),
    users: UserRepository = Depends(get_user_repository),
    sessions: SessionRepository = Depends(get_session_repository),
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(bearer_scheme),
) -> AuthContext:
    """
    Single authentication path (complete mediation).

    1. Bearer JWT (mobile): signature, claims, token type, token version, and
       the referenced session must all still be valid.
    2. Session cookie (web companion): the hashed token must map to a live
       session — logout and idle timeout apply immediately.
    """
    if credentials is not None:
        claims = get_token_service().decode(credentials.credentials, expected_type="access")
        if claims is None:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."},
            )

        user = users.get_by_id(claims.user_id)
        if user is None or not user.is_active:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."},
            )

        # Token version check: password change / recovery invalidates all
        # previously issued JWTs without a denylist.
        if user.token_version != claims.token_version:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "INVALID_TOKEN", "message": "Token has been revoked."},
            )

        session = sessions.get_by_id(claims.session_id)
        if session is None or session.user_id != user.id or not _session_is_current(session):
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "SESSION_EXPIRED", "message": "Session is no longer valid."},
            )

        session.last_activity_at = utcnow()  # sliding idle window
        db.commit()

        if user.is_locked:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail={"code": "ACCOUNT_UNAVAILABLE", "message": "Account is unavailable."},
            )

        return AuthContext(user=user, session=session, via="bearer")

    # Cookie-based session (web companion)
    raw_token = request.cookies.get(settings.session_cookie_name)
    if raw_token:
        session = sessions.get_by_token_hash(hash_token(raw_token))
        if session is None or not _session_is_current(session):
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "SESSION_EXPIRED", "message": "Session is no longer valid."},
            )
        user = users.get_by_id(session.user_id)
        if user is None or not user.is_active:
            raise HTTPException(
                status_code=status.HTTP_401_UNAUTHORIZED,
                detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."},
            )
        if user.is_locked:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail={"code": "ACCOUNT_UNAVAILABLE", "message": "Account is unavailable."},
            )
        session.last_activity_at = utcnow()
        db.commit()
        return AuthContext(user=user, session=session, via="cookie")

    raise HTTPException(
        status_code=status.HTTP_401_UNAUTHORIZED,
        detail={"code": "UNAUTHENTICATED", "message": "Missing authentication credentials."},
        headers={"WWW-Authenticate": "Bearer"},
    )


def get_current_user(context: AuthContext = Depends(get_auth_context)) -> User:
    """Most-used dependency: the authenticated user."""
    return context.user


def require_fresh_password(context: AuthContext = Depends(get_auth_context)) -> User:
    """Block everything except password change until the temporary password is replaced."""
    if context.user.must_change_password:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={
                "code": "PASSWORD_CHANGE_REQUIRED",
                "message": "Password change is required before using the API.",
            },
        )
    return context.user


def require_role(role: UserRole) -> Callable:
    """RBAC gate for operator/moderator endpoints."""
    async def _role_dependency(user: User = Depends(require_fresh_password)) -> User:
        if user.role != role:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail={"code": "FORBIDDEN", "message": "Insufficient permissions."},
            )
        return user

    return _role_dependency


# ---------------------------------------------------------------------------
# CSRF (double-submit cookie) for cookie-authenticated mutations
# ---------------------------------------------------------------------------

def csrf_protect(
    request: Request,
    credentials: Optional[HTTPAuthorizationCredentials] = Depends(bearer_scheme),
) -> None:
    """
    Require a matching X-CSRF-Token for state-changing requests that
    authenticate via cookie (Bearer requests are not CSRF-prone).

    Login sets a readable (non-HttpOnly) CSRF cookie; the client must echo it
    in the X-CSRF-Token header on every mutation (double-submit pattern).
    """
    if credentials is not None:
        return  # token-header auth: no ambient credential, no CSRF risk
    if request.method in ("GET", "HEAD", "OPTIONS"):
        return

    cookie_token = request.cookies.get(settings.csrf_cookie_name)
    header_token = request.headers.get("X-CSRF-Token")
    if not cookie_token or not header_token or not verify_csrf_token(header_token, cookie_token):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail={"code": "CSRF_FAILED", "message": "CSRF validation failed."},
        )
