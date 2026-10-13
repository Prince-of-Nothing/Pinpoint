# How to Deploy for University Demonstration

Run the MVP locally for an in-person or screen-shared demonstration. The
talk track is in the [MVP Demo Script](./mvp-demo-script.md).

## Option 1: Local Docker demo (recommended)

No cloud account needed, and everything runs offline except the map tiles.

```bash
git clone https://github.com/Bangreedy-2/LocationSharing.git
cd LocationSharing

# Use your own secret even for a demo (any long random value)
export SECRET_KEY="$(python3 -c 'import secrets; print(secrets.token_urlsafe(48))')"

docker compose up --build -d
docker compose exec api python scripts/seed_demo.py
docker compose exec -e ADMIN_PASSWORD='a long admin passphrase' api \
    python -m app.cli create-admin --username ops --email ops@example.com
```

Open <http://localhost:8000/>.

For HTTPS (shows the TLS setup; the browser warns until Caddy's local CA is trusted):

```bash
docker compose --profile https up --build -d     # https://localhost:8443/
```

### Demo accounts

Created by `scripts/seed_demo.py`. All use the password printed by the
script (default `together demo 2026`, override with `DEMO_PASSWORD`).

| Username | Name | Scenario |
|---|---|---|
| `demo_parent` | Maria | Receives three shares: live (exact), stale (approximate), no signal (exact); has blocked `demo_blocked` |
| `demo_child` | Alex | Shares exact location with Maria until revoked; low battery; active check-in; pending request from Sam |
| `demo_friend1` | Sam | Shares approximate location with Maria for 8 hours (last update 20 min ago) |
| `demo_friend2` | Jordan | Shares exact location for 24 hours but last update was 3 hours ago; missed check-in |
| `demo_blocked` | Casey | Blocked by Maria; sees her as a non-existent user |

Reset to the initial state at any time:

```bash
docker compose exec api python scripts/seed_demo.py --reset
docker compose exec redis redis-cli flushall      # clears rate-limit counters
```

## Option 2: Hosted demo (remote review)

The same image runs on any container host with managed PostgreSQL and Redis.

1. Build and push: `docker build -t <registry>/always-together-api:latest backend && docker push …`
2. Create PostgreSQL. Run `docker/postgres/init-roles.sh` once (as the owner,
   with `APP_DB_PASSWORD` set) to create the restricted `app_runtime` role.
3. Set environment variables on the service:

   | Variable | Value |
   |---|---|
   | `ENVIRONMENT` | `production` |
   | `SECRET_KEY` | 48+ random characters from a secrets manager |
   | `DATABASE_URL` | `postgresql+psycopg://app_runtime:…@host/always_together` |
   | `MIGRATION_DATABASE_URL` | owner connection string (used only by `alembic upgrade head`) |
   | `REDIS_URL` | managed Redis URL |
   | `FORWARDED_ALLOW_IPS` | the load balancer's address range |

4. Put the service behind HTTPS (platform TLS or Caddy with a real domain).
   In production mode the API refuses insecure settings, disables `/docs`
   and sends HSTS.
5. Seed demo data with `ENVIRONMENT=development` in a one-off job only; the
   seed script refuses to run in production mode.

## Pre-demonstration checklist

- [ ] `curl http://localhost:8000/api/v1/health` returns `{"status":"ok",…}`
- [ ] Demo data seeded (`seed_demo.py --reset`) and Redis flushed
- [ ] Log in as `demo_parent`: the map shows three markers (green, orange, grey)
- [ ] The map tiles load (internet access to `tile.openstreetmap.org`)
- [ ] A second browser profile or private window is ready for `demo_child`
- [ ] Terminal ready for `sh backend/scripts/demo_security.sh`
- [ ] `backend/`: `pytest` passes, and the latest `docs/evidence/security-test-report.md` is open
- [ ] Backup: screen recording of the full demo in case of network problems

## Troubleshooting during the demo

| Problem | Quick fix |
|---|---|
| Map is grey | No internet access to OSM tiles. The markers and list still work; switch to the recorded video for the map part |
| Login keeps failing with 429 | Rate limit hit: `docker compose exec redis redis-cli flushall` |
| A demo account is locked | Log in as the admin and unlock it (`POST /api/v1/admin/users/{id}/unlock`), or re-run `seed_demo.py --reset` |
| Reload logs you out | Use `http://localhost` (not an IP address), or the HTTPS profile |
| Locations show as stale | Expected over time. Re-seed, or report a new position (Map → Reporting → Manual → click the map) |
