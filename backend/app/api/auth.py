"""
Authentication API endpoints.
Implements login, logout, password management, and session handling.

Security behaviour (see docs/reference/design-patterns.md):
- Login is rate limited per IP and answers every credential failure with the
  same generic 401 (no account enumeration, no lockout-state disclosure).
- The session cookie is HttpOnly + SameSite=Lax (+ Secure over TLS) and is now
  correctly attached to the real response object.
- A readable CSRF cookie is issued for the web companion; mutations that
  authenticate via cookie must echo it (double-submit).
- Password changes bump the token version, revoking all outstanding JWTs.
"""

from typing import Optional

from fastapi import APIRouter, Depends, HTTPException, Request, Response, status

from app.api.deps import (
    client_ip,
    csrf_protect,
    get_auth_context,
    get_auth_service,
    get_current_user,
    rate_limit,
    AuthContext,
)
from app.core.config import get_settings
from app.schemas.auth import (
    ForcePasswordChangeRequest,
    LoginRequest,
    LoginResponse,
    PasswordChangeRequest,
    UserResponse,
)
from app.services.auth_service import AuthErrorCode, AuthenticationError, AuthService

settings = get_settings()
router = APIRouter()


def _set_auth_cookies(response: Response, request: Request, session_token: str) -> None:
    """Attach the session cookie (HttpOnly) and CSRF cookie (readable by JS)."""
    secure = settings.is_production or request.url.scheme == "https"

    response.set_cookie(
        key=settings.session_cookie_name,
        value=session_token,
        max_age=settings.session_max_age_seconds,
        httponly=True,
        secure=secure,
        samesite="lax",
        path="/",
    )
    # CSRF token: deliberately not HttpOnly — the client must echo it in the
    # X-CSRF-Token header (double-submit pattern).
    import secrets as _secrets

    response.set_cookie(
        key=settings.csrf_cookie_name,
        value=_secrets.token_urlsafe(32),
        max_age=settings.session_max_age_seconds,
        httponly=False,
        secure=secure,
        samesite="lax",
        path="/",
    )


def _clear_auth_cookies(response: Response, request: Request) -> None:
    secure = settings.is_production or request.url.scheme == "https"
    response.delete_cookie(settings.session_cookie_name, path="/", secure=secure, samesite="lax")
    response.delete_cookie(settings.csrf_cookie_name, path="/", secure=secure, samesite="lax")


@router.post("/login", response_model=LoginResponse)
async def login(
    request: Request,
    credentials: LoginRequest,
    response: Response,
    auth: AuthService = Depends(get_auth_service),
    _rate: None = Depends(rate_limit("login", settings.rate_limit_requests, settings.rate_limit_window_seconds)),
):
    """
    Authenticate user and create a new session.

    Security features: per-IP rate limiting, account lockout with automatic
    unlock, timing-equalised unknown-account handling, generic error messages,
    secure session + CSRF cookies.
    """
    try:
        result = auth.login(
            email=credentials.email,
            password=credentials.password,
            ip_address=client_ip(request),
            user_agent=request.headers.get("user-agent"),
        )
    except AuthenticationError as exc:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={"code": exc.code.value, "message": exc.message},
            headers={"WWW-Authenticate": "Bearer"},
        )

    _set_auth_cookies(response, request, result.session_token)

    return LoginResponse(
        access_token=result.access_token,
        token_type="bearer",
        expires_in=result.expires_in,
        user=UserResponse(
            id=result.user.id,
            email=result.user.email,
            full_name=result.user.full_name,
            role=result.user.role,
            is_active=result.user.is_active,
            must_change_password=result.user.must_change_password,
            created_at=result.user.created_at,
            last_login_at=result.user.last_login_at,
        ),
    )


@router.post("/logout")
async def logout(
    request: Request,
    response: Response,
    auth: AuthService = Depends(get_auth_service),
    _csrf: None = Depends(csrf_protect),
):
    """
    Logout user and invalidate the presented credential.

    Idempotent: revokes the session behind the cookie or the JWT's `sid`
    claim, then clears both cookies. Unknown/expired credentials still
    receive 200 (no state disclosure).
    """
    session_token: Optional[str] = request.cookies.get(settings.session_cookie_name)
    if session_token:
        auth.logout_by_session_token(session_token)
    else:
        auth_header = request.headers.get("Authorization", "")
        if auth_header.startswith("Bearer "):
            claims = auth.decode_access_token(auth_header.removeprefix("Bearer ").strip())
            if claims is not None:
                auth.logout_by_session_id(claims.session_id, claims.user_id)

    _clear_auth_cookies(response, request)
    return {"message": "Successfully logged out"}


@router.post("/password/change")
async def change_password(
    payload: PasswordChangeRequest,
    context: AuthContext = Depends(get_auth_context),
    auth: AuthService = Depends(get_auth_service),
    _csrf: None = Depends(csrf_protect),
):
    """
    Change user password.

    Requires the current password, enforces the password policy, bumps the
    token version (revoking all outstanding JWTs) and revokes every other
    session.
    """
    try:
        auth.change_password(
            user=context.user,
            current_password=payload.current_password,
            new_password=payload.new_password,
            keep_session_id=context.session.id,
        )
    except AuthenticationError as exc:
        status_code = status.HTTP_400_BAD_REQUEST
        fields = None
        if exc.code == AuthErrorCode.WEAK_PASSWORD:
            from app.services.password_policy import DEFAULT_PASSWORD_POLICY

            fields = {"new_password": DEFAULT_PASSWORD_POLICY.validate(payload.new_password)}
        raise HTTPException(
            status_code=status_code,
            detail={"code": exc.code.value, "message": exc.message, "fields": fields},
        )

    return {"message": "Password changed successfully."}


@router.post("/password/force-change")
async def force_change_password(
    payload: ForcePasswordChangeRequest,
    context: AuthContext = Depends(get_auth_context),
    auth: AuthService = Depends(get_auth_service),
    _csrf: None = Depends(csrf_protect),
):
    """First-login flow: replace the temporary password before anything else."""
    try:
        auth.force_change_password(
            user=context.user,
            new_password=payload.new_password,
            keep_session_id=context.session.id,
        )
    except AuthenticationError as exc:
        status_code = (
            status.HTTP_400_BAD_REQUEST
            if exc.code in (AuthErrorCode.WEAK_PASSWORD, AuthErrorCode.PASSWORD_CHANGE_REQUIRED)
            else status.HTTP_401_UNAUTHORIZED
        )
        raise HTTPException(
            status_code=status_code,
            detail={"code": exc.code.value, "message": exc.message},
        )

    return {"message": "Password changed successfully."}


@router.get("/me", response_model=UserResponse)
async def get_me(user=Depends(get_current_user)):
    """Get current authenticated user information (kept for compatibility)."""
    return UserResponse(
        id=user.id,
        email=user.email,
        full_name=user.full_name,
        role=user.role,
        is_active=user.is_active,
        must_change_password=user.must_change_password,
        created_at=user.created_at,
        last_login_at=user.last_login_at,
    )
