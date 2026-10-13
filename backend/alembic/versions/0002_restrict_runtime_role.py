"""Least privilege for the runtime database role

The API connects as ``app_runtime`` (created by
docker/postgres/init-roles.sh), which receives only row-level DML through
default privileges. This migration additionally makes the audit trail
append-only for that role and hides Alembic's bookkeeping table from it.
It is a no-op on databases where the role does not exist (e.g. SQLite).

Revision ID: 0002_runtime_role
Revises: 0001_initial
"""

from collections.abc import Sequence

from alembic import op

revision: str = "0002_runtime_role"
down_revision: str | None = "0001_initial"
branch_labels: str | Sequence[str] | None = None
depends_on: str | Sequence[str] | None = None

RUNTIME_ROLE = "app_runtime"


def _when_role_exists(statement: str) -> None:
    if op.get_bind().dialect.name != "postgresql":
        return
    op.execute(
        f"""
        DO $$
        BEGIN
            IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '{RUNTIME_ROLE}') THEN
                {statement}
            END IF;
        END
        $$;
        """
    )


def upgrade() -> None:
    _when_role_exists(
        f"REVOKE UPDATE, DELETE, TRUNCATE ON audit_events FROM {RUNTIME_ROLE}; "
        f"REVOKE ALL ON alembic_version FROM {RUNTIME_ROLE};"
    )


def downgrade() -> None:
    _when_role_exists(f"GRANT UPDATE, DELETE ON audit_events TO {RUNTIME_ROLE};")
