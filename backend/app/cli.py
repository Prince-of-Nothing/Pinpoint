"""
Operator command line.

    python -m app.cli create-admin --username ops --email ops@example.com

The password is read from the ADMIN_PASSWORD environment variable or asked
for interactively; it is never accepted as a command-line argument (which
would leak into shell history and process listings).
"""

import argparse
import getpass
import os
import sys

from sqlalchemy import or_, select

from app.db.session import SessionLocal
from app.models import User, UserRole
from app.schemas.auth import normalise_username
from app.services import audit
from app.services.passwords import hash_password, password_policy_violations


def create_admin(username: str, email: str) -> int:
    username = normalise_username(username)
    email = email.strip().lower()
    password = os.environ.get("ADMIN_PASSWORD") or getpass.getpass("Admin password: ")
    problems = password_policy_violations(password, username=username, email=email)
    if problems:
        print("Password rejected: " + " ".join(problems), file=sys.stderr)
        return 1
    with SessionLocal() as db:
        if db.scalar(select(User.id).where(or_(User.username == username, User.email == email))):
            print("A user with this username or email already exists.", file=sys.stderr)
            return 1
        user = User(
            username=username,
            email=email,
            password_hash=hash_password(password),
            role=UserRole.ADMIN.value,
        )
        db.add(user)
        db.flush()
        audit.record(
            db, "admin.created_via_cli", actor_id=user.id, target_type="user", target_id=user.id
        )
        db.commit()
    print(f"Administrator '{username}' created.")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m app.cli")
    sub = parser.add_subparsers(dest="command", required=True)
    admin = sub.add_parser("create-admin", help="Create an administrator account")
    admin.add_argument("--username", required=True)
    admin.add_argument("--email", required=True)
    args = parser.parse_args(argv)
    if args.command == "create-admin":
        return create_admin(args.username, args.email)
    return 2


if __name__ == "__main__":
    sys.exit(main())
