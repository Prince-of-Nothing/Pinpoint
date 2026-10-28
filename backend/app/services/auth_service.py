"""
Authentication flows.

Design patterns applied:
- Facade: AuthService exposes a few plain operations (login, logout,
  change_password) over repositories, token service, hashing, and auditing.
- Null Object / anti-enumeration: unknown accounts are verified against a
  throwaway hash so response timing does not reveal account existence.
- Template Method: _apply_password_change defines the shared password-change
  skeleton (validate policy → hash → bump token version → revoke sessions →
  audit) used by both self-service and forced changes.
- Fail-safe defaults: every credential failure returns the same generic
  AUTHENTICATION_FAILED error; lockout state is never revealed to callers.
"""

from dataclasses import dataclass
from datetime import timedelta
from enum import Enum
from typing import List, Optional

from app.core.config import get_settings
from app.core.timeutils import ensure_utc, utcnow
from app.models.user import Session, User
from app.repositories.people import (
    LoginAttemptRepository,
    SessionRepository,
    UserRepository,
)
from app.services.audit import AuditLogger, get_null_audit_logger
from app.services.password_policy import DEFAULT_PASSWORD_POLICY, PasswordPolicy
from app.services.security import (
    get_dummy_password_hash,
    get_password_hash,
    generate_session_token,
    hash_token,
    normalize_email,
    pseudonymize_email,
    verify_password,
)
from app.services.token_service import TokenService, get_token_service

settings = get_settings()


class AuthErrorCode(str, Enum):
    """Machine-readable authentication failure codes (never leak state)."""

    INVALID_CREDENTIALS = "AUTHENTICATION_FAILED"
    INVALID_TOKEN = "INVALID_TOKEN"
    PASSWORD_CHANGE_REQUIRED = "PASSWORD_CHANGE_REQUIRED"
    WEAK_PASSWORD = "WEAK_PASSWORD"


class AuthenticationError(Exception):
    """Raised for every credential failure; message is always generic."""

    def __init__(self, code: AuthErrorCode, message: str = "Invalid credentials."):
        super().__init__(message)
        self.code = code
        self.message = message


@dataclass(frozen=True)
class LoginResult:
    """Successful login outcome."""

    user: User
    session_token: str  # raw value for the cookie; never persisted raw
    access_token: str
    expires_in: int


class AuthService:
    """Facade over authentication business logic."""

    def __init__(
        self,
        users: UserRepository,
        sessions: SessionRepository,
        attempts: LoginAttemptRepository,
        tokens: Optional[TokenService] = None,
        audit: Optional[AuditLogger] = None,
        password_policy: Optional[PasswordPolicy] = None,
    ):
        self._users = users
        self._sessions = sessions
        self._attempts = attempts
        self._tokens = tokens or get_token_service()
        self._audit = audit or get_null_audit_logger()
        self._policy = password_policy or DEFAULT_PASSWORD_POLICY

    # Login -----------------------------------------------------------------

    def login(self, email: str, password: str, ip_address: str,
              user_agent: Optional[str] = None) -> LoginResult:
        """
        Authenticate a user and start a server-side session.

        Raises AuthenticationError with a generic message for every failure
        mode (unknown account, inactive, locked, bad password) so callers
        cannot enumerate accounts or infer lockout state.
        """
        normalized = normalize_email(email)
        now = utcnow()
        user = self._users.get_by_email_normalized(normalized)

        if user is None or not user.is_active:
            # Null Object: burn one bcrypt verification to equalise timing.
            verify_password(password, get_dummy_password_hash())
            self._failed_attempt(normalized, ip_address, "unknown_or_inactive_account")
            raise AuthenticationError(AuthErrorCode.INVALID_CREDENTIALS)

        if user.is_locked:
            if self._lockout_expired(user, now):
                self._unlock(user, now)
            else:
                verify_password(password, get_dummy_password_hash())
                self._failed_attempt(normalized, ip_address, "account_locked")
                raise AuthenticationError(AuthErrorCode.INVALID_CREDENTIALS)

        if not verify_password(password, user.password_hash):
            user.failed_login_attempts = (user.failed_login_attempts or 0) + 1
            locked = False
            if user.failed_login_attempts >= settings.lockout_threshold:
                user.is_locked = True
                user.locked_at = now
                locked = True
                self._audit.record(
                    "auth.account_locked",
                    actor_id=user.id,
                    target_type="user",
                    target_id=user.id,
                    attempts=user.failed_login_attempts,
                )
            self._failed_attempt(normalized, ip_address, "bad_password")
            if locked:
                self._audit.record(
                    "auth.login_failed",
                    actor_id=user.id,
                    reason="account_locked_after_failures",
                )
            self._commit()
            raise AuthenticationError(AuthErrorCode.INVALID_CREDENTIALS)

        # Success -----------------------------------------------------------
        user.failed_login_attempts = 0
        user.last_login_at = now

        raw_session_token = generate_session_token()
        session = self._sessions.add(
            Session(
                token_hash=hash_token(raw_session_token),
                user_id=user.id,
                ip_address=ip_address,
                user_agent=user_agent,
                expires_at=now + timedelta(seconds=settings.session_max_age_seconds),
                last_activity_at=now,
            )
        )

        access_token = self._tokens.create_access_token(
            user_id=user.id,
            session_id=session.id,
            token_version=user.token_version,
        )

        self._attempts.record(pseudonymize_email(normalized), ip_address, success=True)
        self._audit.record(
            "auth.login_succeeded",
            actor_id=user.id,
            target_type="session",
            target_id=session.id,
            must_change_password=user.must_change_password,
        )
        self._commit()

        return LoginResult(
            user=user,
            session_token=raw_session_token,
            access_token=access_token,
            expires_in=settings.access_token_expire_minutes * 60,
        )

    # Logout ----------------------------------------------------------------

    def logout_by_session_token(self, session_token: Optional[str]) -> bool:
        """Revoke the session behind a raw cookie token. Idempotent."""
        if not session_token:
            return False
        revoked = self._sessions.revoke_by_token_hash(hash_token(session_token), utcnow())
        if revoked:
            self._audit.record("auth.logout", target_type="session")
        self._commit()
        return revoked

    def logout_by_session_id(self, session_id: int, actor_id: Optional[int]) -> bool:
        """Revoke a session identified by its id (JWT `sid` claim)."""
        session = self._sessions.get_by_id(session_id)
        if session is None or session.revoked_at is not None:
            return False
        self._sessions.revoke(session, utcnow())
        self._audit.record("auth.logout", actor_id=actor_id, target_type="session", target_id=session_id)
        self._commit()
        return True

    def decode_access_token(self, token: str):
        """Validate an access token and return its claims (None when invalid)."""
        return self._tokens.decode(token, expected_type="access")

    # Password management (Template Method) ---------------------------------

    def change_password(self, user: User, current_password: str,
                        new_password: str, keep_session_id: Optional[int] = None) -> None:
        """Self-service password change: verifies the current password first."""
        if not verify_password(current_password, user.password_hash):
            self._audit.record("auth.password_change_failed", actor_id=user.id,
                               reason="wrong_current_password")
            self._commit()
            raise AuthenticationError(
                AuthErrorCode.INVALID_CREDENTIALS, "Current password is incorrect."
            )
        self._apply_password_change(user, new_password, keep_session_id)

    def force_change_password(self, user: User, new_password: str,
                              keep_session_id: Optional[int] = None) -> None:
        """First-login temporary-password change (no current password check)."""
        if not user.must_change_password:
            raise AuthenticationError(
                AuthErrorCode.PASSWORD_CHANGE_REQUIRED, "Password change is not required."
            )
        self._apply_password_change(user, new_password, keep_session_id)

    def _apply_password_change(self, user: User, new_password: str,
                               keep_session_id: Optional[int]) -> None:
        """Shared skeleton for both password-change variants (Template Method)."""
        issues = self._policy.validate(new_password)
        if issues:
            self._audit.record("auth.password_change_failed", actor_id=user.id,
                               reason="weak_password")
            self._commit()
            raise AuthenticationError(
                AuthErrorCode.WEAK_PASSWORD,
                "Password does not meet security requirements.",
            )

        user.password_hash = get_password_hash(new_password)
        user.must_change_password = False
        user.failed_login_attempts = 0
        user.is_locked = False
        user.locked_at = None
        # Invalidate every outstanding JWT for this user (revocation without
        # a per-token denylist).
        user.token_version = (user.token_version or 1) + 1

        revoked = self._sessions.revoke_all_for_user(
            user.id, utcnow(), except_session_id=keep_session_id
        )
        self._audit.record(
            "auth.password_changed",
            actor_id=user.id,
            target_type="user",
            target_id=user.id,
            sessions_revoked=revoked,
        )
        self._commit()

    # Helpers ---------------------------------------------------------------

    def _lockout_expired(self, user: User, now) -> bool:
        locked_at = ensure_utc(user.locked_at)
        if locked_at is None:
            return False
        return now - locked_at > timedelta(minutes=settings.lockout_duration_minutes)

    def _unlock(self, user: User, now) -> None:
        user.is_locked = False
        user.locked_at = None
        user.failed_login_attempts = 0
        self._audit.record("auth.account_unlocked", actor_id=user.id)

    def _failed_attempt(self, normalized_email: str, ip_address: str,
                        reason: str) -> None:
        self._attempts.record(pseudonymize_email(normalized_email), ip_address, success=False)
        self._audit.record("auth.login_failed", reason=reason)

    def _commit(self) -> None:
        """Commit the unit of work; rollback on failure (fail closed)."""
        db = self._users._db  # repositories share the request-scoped session
        try:
            db.commit()
        except Exception:
            db.rollback()
            raise
