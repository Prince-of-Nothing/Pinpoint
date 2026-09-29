"""Authentication and authorization dependencies shared by all routers."""

from datetime import timedelta
from typing import Annotated

from fastapi import Depends, Request
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer
from sqlalchemy.orm import Session

from app.core.errors import ProblemError
from app.core.timeutils import utcnow
from app.db.session import get_db
from app.models import AuthSession, User
from app.services import audit
from app.services.sessions import revoke, session_is_usable
from app.services.tokens import decode_access_token

_bearer = HTTPBearer(auto_error=False)

DbSession = Annotated[Session, Depends(get_db)]

# Avoid a database write on every request: last_seen_at granularity is one minute.
_LAST_SEEN_GRANULARITY = timedelta(minutes=1)


def unauthorized(detail: str = "Authentication is required.") -> ProblemError:
    return ProblemError(401, "UNAUTHORIZED", detail, headers={"WWW-Authenticate": "Bearer"})


def get_current_session(
    request: Request,
    db: DbSession,
    credentials: HTTPAuthorizationCredentials | None = Depends(_bearer),
) -> AuthSession:
    """
    Validate the bearer access token *and* the server-side session it belongs
    to, so logout, password change and admin deactivation take effect at once.
    """
    if credentials is None or credentials.scheme.lower() != "bearer":
        raise unauthorized()
    claims = decode_access_token(credentials.credentials)
    if claims is None:
        raise unauthorized("The access token is invalid or expired.")

    session = db.get(AuthSession, claims["sid"])
    if session is None or session.user_id != claims["sub"]:
        raise unauthorized("The access token is invalid or expired.")

    problem = session_is_usable(session)
    if problem:
        if session.revoked_at is None:
            revoke(db, session, problem)
            db.commit()
        raise unauthorized("The session has ended. Please log in again.")

    now = utcnow()
    if now - session.last_seen_at >= _LAST_SEEN_GRANULARITY:
        session.last_seen_at = now
        db.commit()
    request.state.user_id = session.user_id
    return session


CurrentSession = Annotated[AuthSession, Depends(get_current_session)]


def get_current_user(session: CurrentSession) -> User:
    return session.user


CurrentUser = Annotated[User, Depends(get_current_user)]


def require_admin(request: Request, db: DbSession, user: CurrentUser) -> User:
    """Role check for operator endpoints (RBAC). Denials are audited."""
    if not user.is_admin:
        audit.record(
            db,
            "authz.admin_denied",
            outcome="denied",
            actor_id=user.id,
            request=request,
            path=request.url.path,
        )
        db.commit()
        raise ProblemError(403, "FORBIDDEN", "Administrator role required.")
    return user


AdminUser = Annotated[User, Depends(require_admin)]
