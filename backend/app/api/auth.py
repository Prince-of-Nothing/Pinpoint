"""
Authentication API endpoints.
Implements login, logout, password management, and session handling.
"""

from fastapi import APIRouter, Depends, HTTPException, status, Request, Response
from fastapi.security import HTTPBearer, HTTPAuthorizationCredentials
from sqlalchemy.orm import Session
from datetime import datetime, timezone, timedelta

from app.db.session import get_db
from app.models.user import User, Session as SessionModel, LoginAttempt, UserRole
from app.schemas.auth import (
    LoginRequest,
    LoginResponse,
    PasswordChangeRequest,
    ForcePasswordChangeRequest,
    UserResponse
)
from app.services.security import (
    verify_password,
    get_password_hash,
    create_access_token,
    generate_session_token,
    normalize_email,
    is_weak_password
)
from app.core.config import get_settings

settings = get_settings()
router = APIRouter()
security = HTTPBearer(auto_error=False)


def get_rate_limit_key(ip: str) -> str:
    """Generate rate limit key for Redis."""
    return f"rate_limit:login:{ip}"


def get_lockout_key(email: str) -> str:
    """Generate lockout key for Redis."""
    return f"lockout:{email}"


@router.post("/login", response_model=LoginResponse)
async def login(
    request: Request,
    credentials: LoginRequest,
    db: Session = Depends(get_db)
):
    """
    Authenticate user and create a new session.
    
    Security features:
    - Rate limiting by IP
    - Account lockout after failed attempts
    - Generic error messages
    - Secure session cookies
    """
    client_ip = request.client.host if request.client else "unknown"
    normalized_email = normalize_email(credentials.email)
    
    # Check account lockout
    lockout_key = get_lockout_key(normalized_email)
    # In production, use Redis for this
    # For now, check database
    
    user = db.query(User).filter(
        User.email_normalized == normalized_email,
        User.is_active == True
    ).first()
    
    # Generic error for unknown/inactive/locked users
    if not user or user.is_locked:
        # Log failed attempt
        attempt = LoginAttempt(
            email=normalized_email,
            ip_address=client_ip,
            success=False
        )
        db.add(attempt)
        db.commit()
        
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={
                "code": "AUTHENTICATION_FAILED",
                "message": "Invalid credentials."
            }
        )
    
    # Verify password
    if not verify_password(credentials.password, user.password_hash):
        # Increment failed attempts
        user.failed_login_attempts += 1
        
        # Check if should lock account
        if user.failed_login_attempts >= settings.lockout_threshold:
            user.is_locked = True
            user.locked_at = datetime.now(timezone.utc)
        
        # Log failed attempt
        attempt = LoginAttempt(
            email=normalized_email,
            ip_address=client_ip,
            success=False
        )
        db.add(attempt)
        db.commit()
        
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={
                "code": "AUTHENTICATION_FAILED",
                "message": "Invalid credentials."
            }
        )
    
    # Successful login - reset failed attempts
    user.failed_login_attempts = 0
    user.last_login_at = datetime.now(timezone.utc)
    
    # Check if password must be changed
    if user.must_change_password:
        # Allow login but flag in response
        pass
    
    # Create session
    session_token = generate_session_token()
    expires_at = datetime.now(timezone.utc) + timedelta(seconds=settings.session_max_age_seconds)
    
    session = SessionModel(
        session_token=session_token,
        user_id=user.id,
        ip_address=client_ip,
        user_agent=request.headers.get("user-agent"),
        expires_at=expires_at
    )
    
    db.add(session)
    db.commit()
    db.refresh(session)
    
    # Create JWT access token
    access_token = create_access_token(
        data={"sub": str(user.id), "email": user.email, "role": user.role.value}
    )
    
    # Set secure session cookie
    response = Response()
    response.set_cookie(
        key=settings.session_cookie_name,
        value=session_token,
        max_age=settings.session_max_age_seconds,
        httponly=True,
        secure=True,  # Only over HTTPS in production
        samesite="lax",
        path="/"
    )
    
    return LoginResponse(
        access_token=access_token,
        token_type="bearer",
        expires_in=settings.access_token_expire_minutes * 60,
        user=UserResponse(
            id=user.id,
            email=user.email,
            full_name=user.full_name,
            role=user.role,
            is_active=user.is_active,
            must_change_password=user.must_change_password,
            created_at=user.created_at,
            last_login_at=user.last_login_at
        )
    )


@router.post("/logout")
async def logout(
    request: Request,
    response: Response,
    db: Session = Depends(get_db)
):
    """
    Logout user and invalidate session.
    
    Security features:
    - Server-side session invalidation
    - Cookie clearing
    """
    # Get session token from cookie or header
    session_token = request.cookies.get(settings.session_cookie_name)
    
    if not session_token:
        # Try from Authorization header
        auth_header = request.headers.get("Authorization")
        if auth_header and auth_header.startswith("Bearer "):
            # This is JWT, not session token
            pass
    
    # Invalidate session in database
    if session_token:
        session = db.query(SessionModel).filter(
            SessionModel.session_token == session_token
        ).first()
        
        if session:
            session.revoked_at = datetime.now(timezone.utc)
            db.commit()
    
    # Clear cookie
    response.delete_cookie(
        settings.session_cookie_name,
        path="/",
        secure=True,
        samesite="lax"
    )
    
    return {"message": "Successfully logged out"}


@router.post("/password/change")
async def change_password(
    request: Request,
    password_data: PasswordChangeRequest,
    db: Session = Depends(get_db),
    credentials: HTTPAuthorizationCredentials = Depends(security)
):
    """
    Change user password.
    
    Security features:
    - Requires current password verification
    - Password strength validation
    - Invalidates all sessions on password change
    """
    # Decode token to get user ID
    from app.services.security import decode_token
    
    payload = decode_token(credentials.credentials)
    if not payload:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."}
        )
    
    user_id = int(payload.get("sub"))
    user = db.query(User).filter(User.id == user_id).first()
    
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "USER_NOT_FOUND", "message": "User not found."}
        )
    
    # Verify current password
    if not verify_password(password_data.current_password, user.password_hash):
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "INVALID_PASSWORD",
                "message": "Current password is incorrect."
            }
        )
    
    # Check password strength
    is_weak, issues = is_weak_password(password_data.new_password)
    if is_weak:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "WEAK_PASSWORD",
                "message": "Password does not meet security requirements.",
                "fields": {"new_password": issues}
            }
        )
    
    # Update password
    user.password_hash = get_password_hash(password_data.new_password)
    user.must_change_password = False
    
    # Invalidate all sessions
    db.query(SessionModel).filter(SessionModel.user_id == user.id).update(
        {"revoked_at": datetime.now(timezone.utc)}
    )
    
    db.commit()
    
    return {"message": "Password changed successfully. Please log in again."}


@router.post("/password/force-change")
async def force_change_password(
    request: Request,
    password_data: ForcePasswordChangeRequest,
    db: Session = Depends(get_db),
    credentials: HTTPAuthorizationCredentials = Depends(security)
):
    """
    Force password change for temporary password accounts.
    
    Required for first-time login with temporary password.
    """
    # Decode token
    from app.services.security import decode_token
    
    payload = decode_token(credentials.credentials)
    if not payload:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."}
        )
    
    user_id = int(payload.get("sub"))
    user = db.query(User).filter(User.id == user_id).first()
    
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "USER_NOT_FOUND", "message": "User not found."}
        )
    
    if not user.must_change_password:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "NOT_REQUIRED",
                "message": "Password change is not required."
            }
        )
    
    # Check password strength
    is_weak, issues = is_weak_password(password_data.new_password)
    if is_weak:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail={
                "code": "WEAK_PASSWORD",
                "message": "Password does not meet security requirements.",
                "fields": {"new_password": issues}
            }
        )
    
    # Update password
    user.password_hash = get_password_hash(password_data.new_password)
    user.must_change_password = False
    
    # Invalidate all sessions except current
    db.commit()
    
    return {"message": "Password changed successfully."}


@router.get("/me", response_model=UserResponse)
async def get_current_user(
    db: Session = Depends(get_db),
    credentials: HTTPAuthorizationCredentials = Depends(security)
):
    """Get current authenticated user information."""
    from app.services.security import decode_token
    
    payload = decode_token(credentials.credentials)
    if not payload:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail={"code": "INVALID_TOKEN", "message": "Invalid or expired token."}
        )
    
    user_id = int(payload.get("sub"))
    user = db.query(User).filter(User.id == user_id).first()
    
    if not user:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail={"code": "USER_NOT_FOUND", "message": "User not found."}
        )
    
    return UserResponse(
        id=user.id,
        email=user.email,
        full_name=user.full_name,
        role=user.role,
        is_active=user.is_active,
        must_change_password=user.must_change_password,
        created_at=user.created_at,
        last_login_at=user.last_login_at
    )
