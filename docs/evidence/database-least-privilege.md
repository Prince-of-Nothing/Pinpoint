# Database Least Privilege and Proxy Evidence

**Controls:** SC-06 (data protection, least privilege), SC-04/SC-08 (tamper-resistant audit trail), SC-05 (transport).
**Recorded:** 2026-09-29 against `docker compose --profile https up` (PostgreSQL 16, Caddy 2).

## Setup being tested

- `docker/postgres/init-roles.sh` creates `app_runtime`, which gets only
  `SELECT/INSERT/UPDATE/DELETE` on tables through default privileges and
  cannot create objects in the `public` schema.
- The API connects as `app_runtime` (`DATABASE_URL`); migrations run as the
  schema owner (`MIGRATION_DATABASE_URL`).
- Migration `0002_runtime_role` revokes `UPDATE, DELETE, TRUNCATE` on
  `audit_events` and all access to `alembic_version` from `app_runtime`.
- Caddy terminates HTTPS on `https://localhost:8443`; Uvicorn trusts
  `X-Forwarded-For` only from Caddy's fixed address (`172.28.0.10`).

## Commands

```bash
docker compose --profile https up -d --build
curl -sk -X POST https://localhost:8443/api/v1/auth/register \
  -H 'content-type: application/json' \
  -d '{"username":"proxytest","email":"p@example.com","password":"correct horse battery staple"}'

Q() { docker compose exec -T -e PGPASSWORD=local-dev-runtime db \
      psql -h localhost -U app_runtime -d always_together -tAc "$1"; }
Q 'select count(*) from users'
Q 'select ip_address from audit_events order by id desc limit 1'
Q "update audit_events set outcome='x'"
Q 'delete from audit_events'
Q 'create table evil(id int)'
Q 'drop table users'
Q 'alter table users add column x int'
Q 'select * from alembic_version'
```

## Observed results

| Attempt as `app_runtime` | Result |
|---|---|
| Register over HTTPS through Caddy | `201` |
| `select count(*) from users` | `1` (normal reads work) |
| Latest audit event IP | `172.28.0.1`, the client as seen by the proxy, not Caddy's `172.28.0.10` |
| `update audit_events ...` | `ERROR: permission denied for table audit_events` |
| `delete from audit_events` | `ERROR: permission denied for table audit_events` |
| `create table evil(id int)` | `ERROR: permission denied for schema public` |
| `drop table users` | `ERROR: must be owner of table users` |
| `alter table users add column x int` | `ERROR: must be owner of table users` |
| `select * from alembic_version` | `ERROR: permission denied for table alembic_version` |

## Remaining limitations

- The schema owner can still modify audit rows; production should ship
  audit events to separate append-only storage as well.
- Encryption at rest depends on the hosting provider's disk/volume
  encryption; it is not configured in the local Docker setup.
- Caddy's internal CA is for demonstrations; production needs a publicly
  trusted certificate (Caddy can obtain one automatically for a real domain).
