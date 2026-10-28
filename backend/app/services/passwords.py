"""
Password hashing (Argon2id) and password policy.

The policy follows NIST SP 800-63B: a minimum length, a maximum that still
allows passphrases, a deny-list of common passwords and no password that
contains the account's own identifiers. Composition rules ("one symbol")
are deliberately not used because they push users to predictable patterns.
"""

from functools import lru_cache

from argon2 import PasswordHasher
from argon2.exceptions import InvalidHashError, VerificationError, VerifyMismatchError

from app.core.config import get_settings

MIN_PASSWORD_LENGTH = 10
MAX_PASSWORD_LENGTH = 128

# Small embedded deny-list of the most common passwords (lower-cased).
COMMON_PASSWORDS = frozenset(
    """
    password password1 password12 password123 password1234 passw0rd p@ssw0rd p@ssword
    123456 1234567 12345678 123456789 1234567890 0123456789 987654321 111111 000000
    qwerty qwerty123 qwertyuiop 1q2w3e4r 1q2w3e4r5t 1qaz2wsx zaq12wsx asdfghjkl
    iloveyou letmein welcome welcome1 welcome123 admin admin123 administrator
    monkey dragon football baseball sunshine princess shadow superman michael
    trustno1 abc123 abcd1234 changeme changeme123 secret secret123 login
    alwaystogether always_together locationsharing
    """.split()
)


@lru_cache
def _hasher() -> PasswordHasher:
    settings = get_settings()
    return PasswordHasher(
        time_cost=settings.password_hash_time_cost,
        memory_cost=settings.password_hash_memory_kib,
        parallelism=settings.password_hash_parallelism,
    )


@lru_cache
def _dummy_hash() -> str:
    return _hasher().hash("dummy-password-for-timing-equalisation")


def hash_password(password: str) -> str:
    return _hasher().hash(password)


def verify_password(password_hash: str, password: str) -> bool:
    try:
        return _hasher().verify(password_hash, password)
    except (VerifyMismatchError, VerificationError, InvalidHashError):
        return False


def needs_rehash(password_hash: str) -> bool:
    return _hasher().check_needs_rehash(password_hash)


def burn_verification_time(password: str) -> None:
    """Run a verification against a dummy hash so unknown accounts take as long as real ones."""
    verify_password(_dummy_hash(), password)


def password_policy_violations(password: str, *, username: str, email: str) -> list[str]:
    problems: list[str] = []
    if len(password) < MIN_PASSWORD_LENGTH:
        problems.append(f"Password must be at least {MIN_PASSWORD_LENGTH} characters long.")
    if len(password) > MAX_PASSWORD_LENGTH:
        problems.append(f"Password must be at most {MAX_PASSWORD_LENGTH} characters long.")
    lowered = password.lower()
    if lowered in COMMON_PASSWORDS:
        problems.append("Password is too common.")
    if len(set(password)) < 4:
        problems.append("Password is too repetitive.")
    identifiers = {username.lower(), email.lower().split("@", 1)[0]}
    if any(len(ident) >= 3 and ident in lowered for ident in identifiers):
        problems.append("Password must not contain your username or email.")
    return problems
