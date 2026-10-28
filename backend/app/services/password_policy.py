"""
Password strength policy.

Design patterns applied:
- Strategy: each PasswordRule is a swappable validation algorithm.
- Chain of Responsibility: rules are chained; the policy asks every rule in
  order and collects all failures (fail-closed: a broken/unknown rule means
  the password is rejected, never silently accepted).
"""

from abc import ABC, abstractmethod

# Small embedded deny-list for the MVP. Production should load a fuller
# breached-password corpus (e.g. top-100k from a known breach list); this is
# tracked as a known limitation, not a complete defence.
COMMON_PASSWORDS = frozenset(
    {
        "password",
        "password1",
        "password123",
        "123456",
        "12345678",
        "123456789",
        "qwerty",
        "qwerty123",
        "abc123",
        "admin",
        "letmein",
        "welcome",
        "welcome1",
        "iloveyou",
        "monkey",
        "dragon",
        "football",
        "baseball",
        "sunshine",
        "princess",
        "alwaysTogether",
        "alwaystogether",
        "together",
        "pinpoint",
    }
)

SPECIAL_CHARACTERS = "!@#$%^&*()_+-=[]{}|;:,.<>?"


class PasswordRule(ABC):
    """A single password rule (Strategy)."""

    @property
    @abstractmethod
    def code(self) -> str:
        """Stable machine-readable issue code."""

    @abstractmethod
    def check(self, password: str) -> str | None:
        """Return a human-readable issue, or None when satisfied."""


class MinLengthRule(PasswordRule):
    def __init__(self, minimum: int = 10):
        self.minimum = minimum

    @property
    def code(self) -> str:
        return "min_length"

    def check(self, password: str) -> str | None:
        if len(password) < self.minimum:
            return f"Password must be at least {self.minimum} characters long"
        return None


class MaxLengthRule(PasswordRule):
    def __init__(self, maximum: int = 128):
        self.maximum = maximum

    @property
    def code(self) -> str:
        return "max_length"

    def check(self, password: str) -> str | None:
        # Guards against bcrypt's 72-byte truncation and memory abuse.
        if len(password) > self.maximum:
            return f"Password must not exceed {self.maximum} characters"
        return None


class CharacterClassRule(PasswordRule):
    """Require at least one character from a class (upper/lower/digit/symbol)."""

    _CHECKS = {
        "an uppercase letter": str.isupper,
        "a lowercase letter": str.islower,
        "a digit": str.isdigit,
        "a special character": lambda c: c in SPECIAL_CHARACTERS,
    }

    def __init__(self, kind: str):
        if kind not in self._CHECKS:
            raise ValueError(f"Unknown character class: {kind}")
        self.kind = kind

    @property
    def code(self) -> str:
        return f"requires_{self.kind.replace(' ', '_')}"

    def check(self, password: str) -> str | None:
        if not any(self._CHECKS[self.kind](char) for char in password):
            return f"Password must contain at least one {self.kind}"
        return None


class NoWhitespaceEdgesRule(PasswordRule):
    @property
    def code(self) -> str:
        return "no_whitespace_edges"

    def check(self, password: str) -> str | None:
        if password != password.strip():
            return "Password must not start or end with whitespace"
        return None


class CommonPasswordRule(PasswordRule):
    @property
    def code(self) -> str:
        return "common_password"

    def check(self, password: str) -> str | None:
        if password.lower() in COMMON_PASSWORDS:
            return "Password is too common"
        return None


class PasswordPolicy:
    """Chain of Responsibility over a list of PasswordRule strategies."""

    def __init__(self, rules: list[PasswordRule]):
        self._rules = list(rules)

    def validate(self, password: str) -> list[str]:
        """Return every failing rule's message (empty list == accepted)."""
        issues: list[str] = []
        for rule in self._rules:
            issue = rule.check(password)
            if issue:
                issues.append(issue)
        return issues

    def is_valid(self, password: str) -> bool:
        return not self.validate(password)


def build_default_policy() -> PasswordPolicy:
    """Compose the default MVP password policy."""
    return PasswordPolicy(
        [
            MinLengthRule(10),
            MaxLengthRule(128),
            CharacterClassRule("an uppercase letter"),
            CharacterClassRule("a lowercase letter"),
            CharacterClassRule("a digit"),
            CharacterClassRule("a special character"),
            NoWhitespaceEdgesRule(),
            CommonPasswordRule(),
        ]
    )


# Shared policy instance (stateless, so a single instance is safe).
DEFAULT_PASSWORD_POLICY = build_default_policy()
