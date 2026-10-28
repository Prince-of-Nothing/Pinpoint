"""
Create demonstration data (development only).

    python scripts/seed_demo.py            # create demo users if missing
    python scripts/seed_demo.py --reset    # delete and recreate them

All demo accounts share one password: DEMO_PASSWORD from the environment, or
the default printed at the end. The script refuses to run in production.

Scenario (log in as demo_parent to see all three freshness states):

    demo_child   -> demo_parent  exact, until revoked, live, low battery, active check-in
    demo_friend1 -> demo_parent  approximate, 8 h, stale (20 min old)
    demo_friend2 -> demo_parent  exact, 24 h, dead (3 h old), missed check-in
    demo_parent  -> demo_child   approximate, 1 h (sharing is one-way per direction)
    demo_friend1 -> demo_child   pending friend request
    demo_parent  blocked demo_blocked
"""

import argparse
import os
import sys
from datetime import timedelta
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from sqlalchemy import delete, select  # noqa: E402

from app.core.config import get_settings  # noqa: E402
from app.core.timeutils import utcnow  # noqa: E402
from app.db.session import SessionLocal  # noqa: E402
from app.models import (  # noqa: E402
    Block,
    CheckIn,
    Friendship,
    FriendshipStatus,
    LatestLocation,
    LocationShare,
    User,
    pair_key,
)
from app.services.passwords import hash_password, password_policy_violations  # noqa: E402

DEFAULT_PASSWORD = "together demo 2026"  # noqa: S105  # nosec B105 - development demo data only

USERS = {
    "demo_parent": "Maria",
    "demo_child": "Alex",
    "demo_friend1": "Sam",
    "demo_friend2": "Jordan",
    "demo_blocked": "Casey",
}

# Around University Square, Bucharest
PLACES = {
    "demo_parent": (44.43552, 26.10214),
    "demo_child": (44.43497, 26.10154),
    "demo_friend1": (44.44150, 26.09720),
    "demo_friend2": (44.42680, 26.08730),
}


def reset(db) -> None:
    ids = db.scalars(select(User.id).where(User.username.in_(USERS))).all()
    if ids:
        db.execute(delete(User).where(User.id.in_(ids)))
        db.commit()
        print(f"Removed {len(ids)} demo users.")


def seed(db, password: str) -> None:
    now = utcnow()
    password_hash = hash_password(password)
    users = {}
    for username, display_name in USERS.items():
        user = User(
            username=username,
            email=f"{username}@demo.invalid",
            password_hash=password_hash,
            display_name=display_name,
        )
        db.add(user)
        users[username] = user
    db.flush()
    u = {name: user.id for name, user in users.items()}

    def befriend(a: str, b: str) -> None:
        db.add(
            Friendship(
                requester_id=u[a],
                addressee_id=u[b],
                pair_key=pair_key(u[a], u[b]),
                status=FriendshipStatus.ACCEPTED.value,
                responded_at=now - timedelta(days=30),
            )
        )

    befriend("demo_parent", "demo_child")
    befriend("demo_parent", "demo_friend1")
    befriend("demo_parent", "demo_friend2")
    db.add(
        Friendship(
            requester_id=u["demo_friend1"],
            addressee_id=u["demo_child"],
            pair_key=pair_key(u["demo_friend1"], u["demo_child"]),
        )
    )
    db.add(Block(blocker_id=u["demo_parent"], blocked_id=u["demo_blocked"]))

    def share(owner: str, recipient: str, precision: str, hours: float | None, purpose: str):
        db.add(
            LocationShare(
                owner_id=u[owner],
                recipient_id=u[recipient],
                precision=precision,
                purpose=purpose,
                expires_at=now + timedelta(hours=hours) if hours else None,
            )
        )

    share("demo_child", "demo_parent", "exact", None, "Family safety")
    share("demo_friend1", "demo_parent", "approximate", 8, "Meeting up later")
    share("demo_friend2", "demo_parent", "exact", 24, "Hiking trip")
    share("demo_parent", "demo_child", "approximate", 1, "Picking you up")

    def locate(username: str, age: timedelta, battery: int, charging: bool, accuracy: float):
        lat, lon = PLACES[username]
        db.add(
            LatestLocation(
                user_id=u[username],
                latitude=lat,
                longitude=lon,
                accuracy_m=accuracy,
                battery_pct=battery,
                is_charging=charging,
                recorded_at=now - age,
                received_at=now - age,
            )
        )

    locate("demo_parent", timedelta(seconds=30), 76, True, 8)
    locate("demo_child", timedelta(seconds=45), 14, False, 12)  # live, low battery
    locate("demo_friend1", timedelta(minutes=20), 55, False, 25)  # stale
    locate("demo_friend2", timedelta(hours=3), 5, False, 40)  # dead

    db.add(
        CheckIn(
            user_id=u["demo_child"],
            note="Walking home from school",
            due_at=now + timedelta(minutes=20),
        )
    )
    db.add(
        CheckIn(
            user_id=u["demo_friend2"],
            note="Back from the ridge trail by 16:00",
            due_at=now - timedelta(hours=1),
            grace_minutes=15,
            created_at=now - timedelta(hours=5),
        )
    )
    db.commit()


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--reset", action="store_true", help="delete and recreate the demo users")
    args = parser.parse_args()

    if get_settings().environment == "production":
        print("Refusing to create demo data in production.", file=sys.stderr)
        return 1
    password = os.environ.get("DEMO_PASSWORD", DEFAULT_PASSWORD)
    problems = sorted(
        {
            problem
            for username in USERS
            for problem in password_policy_violations(
                password, username=username, email=f"{username}@demo.invalid"
            )
        }
    )
    if problems:
        print("DEMO_PASSWORD rejected: " + " ".join(problems), file=sys.stderr)
        return 1

    with SessionLocal() as db:
        if args.reset:
            reset(db)
        if db.scalar(select(User.id).where(User.username == "demo_parent")):
            print("Demo users already exist (use --reset to recreate them).")
            return 0
        seed(db, password)

    print("Demo data created. Accounts (all with the same password):")
    for username, display_name in USERS.items():
        print(f"  {username:<13} {display_name}")
    print(f"Password: {password}")
    print("Log in as demo_parent to see live, stale and no-signal friends on the map.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
