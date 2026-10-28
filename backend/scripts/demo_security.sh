#!/bin/sh
# Live security demonstration against a running API with demo data.
#
#   python scripts/seed_demo.py --reset     # (inside the api container or venv)
#   sh scripts/demo_security.sh [BASE_URL]  # default http://localhost:8000
#
# Each step prints what is attempted, the HTTP status and the error code.
# The rate-limit step runs last because it exhausts the login limit for
# this IP for one minute.
set -eu
BASE="${1:-http://localhost:8000}/api/v1"
PASS="${DEMO_PASSWORD:-together demo 2026}"
J='content-type: application/json'

json() { python3 -c "import sys, json; print(json.load(sys.stdin)$1)"; }
status() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
say() { printf '\n== %s\n' "$1"; }

login() {
  curl -s -X POST "$BASE/auth/login" -H "$J" \
    -d "{\"login\":\"$1\",\"password\":\"$PASS\",\"token_delivery\":\"body\"}"
}

PARENT=$(login demo_parent)
P_TOKEN=$(echo "$PARENT" | json "['access_token']")
CHILD_TOKEN=$(login demo_child | json "['access_token']")
F2=$(login demo_friend2)
F2_TOKEN=$(echo "$F2" | json "['access_token']")

say "1. Directional consent: demo_parent sees friends who chose to share"
curl -s "$BASE/locations/visible" -H "Authorization: Bearer $P_TOKEN" |
  python3 -c "import sys,json; [print('  ', i['user']['username'], i['precision'], i['freshness']) for i in json.load(sys.stdin)]"

say "2. demo_blocked is not visible and cannot find demo_parent"
BLOCKED_TOKEN=$(login demo_blocked | json "['access_token']")
echo "   friend request to demo_parent -> $(status -X POST "$BASE/friendships" -H "$J" \
  -H "Authorization: Bearer $BLOCKED_TOKEN" -d '{"username":"demo_parent"}')"
echo "   friend request to nobody_here -> $(status -X POST "$BASE/friendships" -H "$J" \
  -H "Authorization: Bearer $BLOCKED_TOKEN" -d '{"username":"nobody_here"}')  (indistinguishable)"

say "3. IDOR: demo_friend2 tries to read the share demo_child gave demo_parent"
SHARE_ID=$(curl -s "$BASE/location-shares" -H "Authorization: Bearer $CHILD_TOKEN" | json "['outgoing'][0]['id']")
echo "   GET /location-shares/$SHARE_ID as demo_friend2 -> $(status "$BASE/location-shares/$SHARE_ID" -H "Authorization: Bearer $F2_TOKEN")"
echo "   (identical to a missing record; recorded as authz.denied in the audit log)"

say "4. Mass assignment: demo_child tries to make themselves admin"
echo "   PATCH /users/me {\"role\":\"admin\"} -> $(status -X PATCH "$BASE/users/me" -H "$J" -H "Authorization: Bearer $CHILD_TOKEN" -d '{"role":"admin"}')"

say "5. RBAC: a normal user calls an admin endpoint"
echo "   GET /admin/users as demo_child -> $(status "$BASE/admin/users" -H "Authorization: Bearer $CHILD_TOKEN")"

say "6. Input validation: impossible coordinates and NaN"
echo "   latitude 91    -> $(status -X PUT "$BASE/locations/me" -H "$J" -H "Authorization: Bearer $CHILD_TOKEN" -d '{"latitude":91,"longitude":0}')"
echo "   latitude NaN   -> $(status -X PUT "$BASE/locations/me" -H "$J" -H "Authorization: Bearer $CHILD_TOKEN" -d '{"latitude":"NaN","longitude":0}')"

say "7. Refresh-token theft: the old token is replayed after rotation"
OLD_REFRESH=$(echo "$F2" | json "['refresh_token']")
NEW=$(curl -s -X POST "$BASE/auth/refresh" -H "$J" -d "{\"refresh_token\":\"$OLD_REFRESH\"}")
NEW_ACCESS=$(echo "$NEW" | json "['access_token']")
echo "   legitimate refresh                 -> 200 (token rotated)"
echo "   attacker replays the old token     -> $(status -X POST "$BASE/auth/refresh" -H "$J" -d "{\"refresh_token\":\"$OLD_REFRESH\"}")"
echo "   victim's new access token now      -> $(status "$BASE/users/me" -H "Authorization: Bearer $NEW_ACCESS") (whole session revoked)"

say "8. Logout invalidates the access token immediately"
echo "   before logout -> $(status "$BASE/users/me" -H "Authorization: Bearer $P_TOKEN")"
curl -s -o /dev/null -X POST "$BASE/auth/logout" -H "Authorization: Bearer $P_TOKEN"
echo "   after logout  -> $(status "$BASE/users/me" -H "Authorization: Bearer $P_TOKEN")"

say "9. Brute force: repeated wrong passwords from one IP"
i=1
while [ $i -le 11 ]; do
  code=$(status -X POST "$BASE/auth/login" -H "$J" -d '{"login":"demo_friend1","password":"guess-number-'$i'"}')
  printf '   attempt %2d -> %s\n' "$i" "$code"
  i=$((i + 1))
done
echo "   (401 until the per-IP limit, then 429 with Retry-After; the account"
echo "    itself locked for 15 minutes after 5 failures)"
