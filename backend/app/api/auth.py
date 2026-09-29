"""Registration, login, token refresh, logout, session and password management."""

from datetime import timedelta

from fastapi import APIRouter, Request, Response, status
from sqlalchemy import or_, select

from app.api.deps import CurrentSession, CurrentUser, DbSession, deny_foreign, unauthorized
from app.core.config import get_settings
from app.core.errors import ProblemError, not_found
from app.core.timeutils import utcnow
from app.models import AuthSession, RefreshToken, User
from app.schemas.auth import (
    LoginRequest,
    PasswordChangeRequest,
    RefreshRequest,
    RegisterRequest,
    SessionInfo,
    TokenResponse,
    UserSelf,
)
from app.services import audit, ratelimit
from app.services.passwords import (
    burn_verification_time,
    hash_password,
    needs_rehash,
    password_policy_violations,
    verify_password,
)
from app.services.sessions import (
    RefreshError,
    create_session,
    revoke,
    revoke_all_for_user,
    rotate_refresh_token,
)
from app.services.tokens import hash_secret, secret_matches

router = APIRouter()

INVALID_CREDENTIALS = "Invalid username/email or password."


# ---------------------------------------------------------------- cookies


def _set_auth_cookies(response: Response, refresh_token: str, csrf_token: str | None) -> None:
    settings = get_settings()
    max_age = settings.session_absolute_timeout_hours * 3600
    response.set_cookie(
        settings.refresh_cookie_name,
        refresh_token,
        max_age=max_age,
        httponly=True,  # not readable by JavaScript (XSS cannot steal it)
        secure=settings.cookie_secure,
        samesite="strict",  # never sent on cross-site requests
        path="/api/v1/auth",  # only sent to the auth endpoints
    )
    if csrf_token:
        response.set_cookie(
            settings.csrf_cookie_name,
            csrf_token,
            max_age=max_age,
            httponly=False,  # the web client must read it and echo it in a header
            secure=settings.cookie_secure,
            samesite="strict",
            path="/",
        )


def _clear_auth_cookies(response: Response) -> None:
    settings = get_settings()
    response.delete_cookie(
        settings.refresh_cookie_name,
        path="/api/v1/auth",
        secure=settings.cookie_secure,
        httponly=True,
        samesite="strict",
    )
    response.delete_cookie(
        settings.csrf_cookie_name, path="/", secure=settings.cookie_secure, samesite="strict"
    )


# ------------------------------------------------------------ registration


@router.post(
    "/register",
    response_model=UserSelf,
    status_code=status.HTTP_201_CREATED,
    dependencies=[ratelimit.rate_limit(ratelimit.REGISTER)],
)
def register(body: RegisterRequest, request: Request, db: DbSession) -> User:
    problems = password_policy_violations(body.password, username=body.username, email=body.email)
    if problems:
        raise ProblemError(
            422,
            "WEAK_PASSWORD",
            "The password does not meet the policy.",
            errors={"password": problems},
        )

    exists = db.scalar(
        select(User.id).where(or_(User.username == body.username, User.email == body.email))
    )
    if exists:
        audit.record(db, "auth.register", outcome="failure", request=request, reason="duplicate")
        db.commit()
        raise ProblemError(409, "ACCOUNT_EXISTS", "Username or email is already registered.")

    user = User(
        username=body.username,
        email=body.email,
        password_hash=hash_password(body.password),
        display_name=body.display_name,
    )
    db.add(user)
    db.flush()
    audit.record(
        db,
        "auth.register",
        actor_id=user.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    return user


# ------------------------------------------------------------------ login


@router.post(
    "/login", response_model=TokenResponse, dependencies=[ratelimit.rate_limit(ratelimit.LOGIN)]
)
def login(body: LoginRequest, request: Request, response: Response, db: DbSession) -> TokenResponse:
    settings = get_settings()
    identifier = body.login.strip().lower()
    user = db.scalar(select(User).where(or_(User.username == identifier, User.email == identifier)))
    now = utcnow()

    if user is None or not user.is_active:
        burn_verification_time(body.password)  # same timing as a real account
        audit.record(
            db, "auth.login", outcome="failure", request=request, reason="unknown_or_inactive"
        )
        db.commit()
        raise unauthorized(INVALID_CREDENTIALS)

    locked = user.locked_until is not None and user.locked_until > now
    password_ok = verify_password(user.password_hash, body.password)

    if locked:
        # Same generic answer as a wrong password: lockout state is not disclosed
        audit.record(
            db, "auth.login", outcome="failure", actor_id=user.id, request=request, reason="locked"
        )
        db.commit()
        raise unauthorized(INVALID_CREDENTIALS)

    if not password_ok:
        user.failed_login_attempts += 1
        reason = "bad_password"
        if user.failed_login_attempts >= settings.lockout_threshold:
            user.locked_until = now + timedelta(minutes=settings.lockout_duration_minutes)
            user.failed_login_attempts = 0
            reason = "bad_password_lockout"
            audit.record(
                db,
                "auth.lockout",
                outcome="failure",
                actor_id=user.id,
                target_type="user",
                target_id=user.id,
                request=request,
                minutes=settings.lockout_duration_minutes,
            )
        audit.record(
            db, "auth.login", outcome="failure", actor_id=user.id, request=request, reason=reason
        )
        db.commit()
        raise unauthorized(INVALID_CREDENTIALS)

    user.failed_login_attempts = 0
    user.locked_until = None
    user.last_login_at = now
    if needs_rehash(user.password_hash):
        user.password_hash = hash_password(body.password)

    cookie_mode = body.token_delivery == "cookie"  # noqa: S105  # nosec B105
    issued = create_session(db, user, request, with_csrf=cookie_mode)
    audit.record(
        db,
        "auth.login",
        actor_id=user.id,
        target_type="session",
        target_id=issued.session.id,
        request=request,
        delivery=body.token_delivery,
    )
    db.commit()

    if cookie_mode:
        _set_auth_cookies(response, issued.refresh_token, issued.csrf_token)
    return TokenResponse(
        access_token=issued.access_token,
        expires_in=issued.expires_in,
        refresh_token=None if cookie_mode else issued.refresh_token,
        csrf_token=issued.csrf_token,
        user=UserSelf.model_validate(user),
    )


# ---------------------------------------------------------------- refresh


@router.post(
    "/refresh",
    response_model=TokenResponse,
    dependencies=[ratelimit.rate_limit(ratelimit.REFRESH)],
)
def refresh(
    request: Request, response: Response, db: DbSession, body: RefreshRequest | None = None
) -> TokenResponse:
    """
    Rotate the refresh token. Native clients send it in the body; the web
    client relies on the HttpOnly cookie and must prove same-origin intent
    with the CSRF token bound to its session (synchronizer token).
    """
    settings = get_settings()
    cookie_mode = body is None or body.refresh_token is None
    raw = request.cookies.get(settings.refresh_cookie_name) if cookie_mode else body.refresh_token
    if not raw:
        raise unauthorized("No refresh token was provided.")

    if cookie_mode:
        stored = db.scalar(select(RefreshToken).where(RefreshToken.token_hash == hash_secret(raw)))
        csrf_header = request.headers.get(settings.csrf_header_name)
        if stored is None or not secret_matches(csrf_header, stored.session.csrf_token_hash):
            audit.record(
                db, "auth.csrf_rejected", outcome="denied", request=request, endpoint="refresh"
            )
            db.commit()
            raise ProblemError(403, "CSRF_FAILED", "Missing or invalid CSRF token.")

    try:
        session, new_raw, access, expires_in = rotate_refresh_token(db, raw)
    except RefreshError as exc:
        session = exc.session
        audit.record(
            db,
            "auth.refresh_reuse" if exc.reason == "reuse_detected" else "auth.refresh",
            outcome="failure",
            actor_id=session.user_id if session else None,
            target_type="session" if session else None,
            target_id=session.id if session else None,
            request=request,
            reason=exc.reason,
        )
        db.commit()
        if cookie_mode:
            _clear_auth_cookies(response)
        raise unauthorized("The refresh token is invalid or expired.") from None

    audit.record(
        db,
        "auth.refresh",
        actor_id=session.user_id,
        target_type="session",
        target_id=session.id,
        request=request,
    )
    db.commit()
    if cookie_mode:
        _set_auth_cookies(response, new_raw, None)
    return TokenResponse(
        access_token=access,
        expires_in=expires_in,
        refresh_token=None if cookie_mode else new_raw,
        user=UserSelf.model_validate(session.user),
    )


# ---------------------------------------------------------- logout/sessions


@router.post("/logout", status_code=status.HTTP_204_NO_CONTENT)
def logout(
    request: Request, response: Response, session: CurrentSession, db: DbSession
) -> Response:
    revoke(db, session, "logout")
    audit.record(
        db,
        "auth.logout",
        actor_id=session.user_id,
        target_type="session",
        target_id=session.id,
        request=request,
    )
    db.commit()
    response = Response(status_code=status.HTTP_204_NO_CONTENT)
    _clear_auth_cookies(response)
    return response


@router.post("/logout-all", status_code=status.HTTP_204_NO_CONTENT)
def logout_all(request: Request, user: CurrentUser, db: DbSession) -> Response:
    revoke_all_for_user(db, user.id, "logout_all")
    audit.record(
        db,
        "auth.logout_all",
        actor_id=user.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    response = Response(status_code=status.HTTP_204_NO_CONTENT)
    _clear_auth_cookies(response)
    return response


@router.get("/sessions", response_model=list[SessionInfo])
def list_sessions(session: CurrentSession, db: DbSession) -> list[SessionInfo]:
    now = utcnow()
    rows = db.scalars(
        select(AuthSession)
        .where(
            AuthSession.user_id == session.user_id,
            AuthSession.revoked_at.is_(None),
            AuthSession.expires_at > now,
        )
        .order_by(AuthSession.last_seen_at.desc())
    ).all()
    result = []
    for row in rows:
        info = SessionInfo.model_validate(row)
        info.current = row.id == session.id
        result.append(info)
    return result


@router.delete("/sessions/{session_id}", status_code=status.HTTP_204_NO_CONTENT)
def revoke_session(
    session_id: str, request: Request, current: CurrentSession, db: DbSession
) -> Response:
    target = db.get(AuthSession, session_id)
    # Another user's session is indistinguishable from a missing one (no IDOR oracle)
    if target is not None and target.user_id != current.user_id:
        raise deny_foreign(db, request, current.user_id, "session", session_id, "Session")
    if target is None or target.revoked_at is not None:
        raise not_found("Session")
    revoke(db, target, "user_revoked")
    audit.record(
        db,
        "auth.session_revoked",
        actor_id=current.user_id,
        target_type="session",
        target_id=target.id,
        request=request,
    )
    db.commit()
    return Response(status_code=status.HTTP_204_NO_CONTENT)


# --------------------------------------------------------------- password


@router.post(
    "/password",
    status_code=status.HTTP_204_NO_CONTENT,
    dependencies=[ratelimit.rate_limit(ratelimit.PASSWORD_CHANGE)],
)
def change_password(
    body: PasswordChangeRequest, request: Request, user: CurrentUser, db: DbSession
) -> Response:
    settings = get_settings()
    if not verify_password(user.password_hash, body.current_password):
        # Counts toward lockout so a stolen access token cannot brute-force the password
        user.failed_login_attempts += 1
        if user.failed_login_attempts >= settings.lockout_threshold:
            user.locked_until = utcnow() + timedelta(minutes=settings.lockout_duration_minutes)
            user.failed_login_attempts = 0
            revoke_all_for_user(db, user.id, "lockout")
            audit.record(
                db,
                "auth.lockout",
                outcome="failure",
                actor_id=user.id,
                target_type="user",
                target_id=user.id,
                request=request,
            )
        audit.record(
            db,
            "auth.password_change",
            outcome="failure",
            actor_id=user.id,
            request=request,
            reason="bad_current_password",
        )
        db.commit()
        raise ProblemError(400, "INVALID_CREDENTIALS", "The current password is incorrect.")

    problems = password_policy_violations(
        body.new_password, username=user.username, email=user.email
    )
    if body.new_password == body.current_password:
        problems.append("The new password must differ from the current one.")
    if problems:
        raise ProblemError(
            422,
            "WEAK_PASSWORD",
            "The password does not meet the policy.",
            errors={"new_password": problems},
        )

    user.password_hash = hash_password(body.new_password)
    user.password_changed_at = utcnow()
    user.failed_login_attempts = 0
    # Every session (including this one) must log in again with the new password
    revoke_all_for_user(db, user.id, "password_changed")
    audit.record(
        db,
        "auth.password_change",
        actor_id=user.id,
        target_type="user",
        target_id=user.id,
        request=request,
    )
    db.commit()
    response = Response(status_code=status.HTTP_204_NO_CONTENT)
    _clear_auth_cookies(response)
    return response
