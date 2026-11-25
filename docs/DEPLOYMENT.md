# Deployment guide — Pinpoint

**Current release:** 3.0.0 · **Stack:** one Rust crate (`pinpoint-desktop`) with an Axum server, a desktop client, an Android app and a web client served by the same server.

Release assets: the signed universal Android APK, `pinpoint-desktop-linux-x64-3.0.0.zip`, `pinpoint-desktop-windows-x64-3.0.0.zip`, and `pinpoint-server-windows-x64-3.0.0.zip`. The server zip includes the web client; run it and open `http://127.0.0.1:8080/`. The tag workflow builds the Windows ZIPs on `windows-latest`, Linux on `ubuntu-latest` and macOS on `macos-latest`, checks the binaries/READMEs, so future releases attach verified assets automatically.

This page covers the supported paths: local in-memory, local PostgreSQL, the Docker Compose stack, a production checklist, the desktop/Android builds and the verification gate. The archived Go/Python/Kotlin prototypes are not buildable from this tree; see ADR-0007.

---

## 1. Local (single binary, in-memory)

```bash
cargo run --bin pinpoint-server
# http://127.0.0.1:8000/
```

- `BIND_ADDR` defaults to `127.0.0.1:8000`.
- Without `DATABASE_URL` the store is in-memory: data is lost on restart. Fine for a demo, not for friends.
- The web client is embedded in the binary; there is no separate frontend build.
- No demo accounts are seeded. Register on the login screen.
- `GET /health` answers with a small JSON status.

## 2. Local with PostgreSQL

```bash
export DATABASE_URL=postgresql://pinpoint:pinpoint-local-dev@localhost:5432/pinpoint
cargo run --bin pinpoint-server
```

- Migrations v1–15 are applied at startup.
- Sessions become PostgreSQL-authoritative: every authenticated `/api/` request reads the session row, so a revocation on another replica takes effect on the next request.
- Session lifetime: `PINPOINT_SESSION_IDLE_MINUTES` (default 720 = 12 h) and `PINPOINT_SESSION_MAX_DAYS` (default 30).
- The private trail is pruned hourly after 7 days.

## 3. Full stack with Docker Compose

```bash
docker compose up --build -d
docker compose ps
curl -fsS http://127.0.0.1:8000/health
```

- `gateway` (nginx) is the only client entrypoint: `:8000` HTTP and `:8443` HTTPS. It routes `/api/v1/auth`, `/api/v1/users`, `/api/v1/locations`, `/api/v1/messages`, `/map` and `/` to healthy replicas.
- Services: `auth` (:8001), `users` (:8002), `locations` (:8003), `map` (:8004), `messages` (:8007, via the gateway), `diag` (:8005), `admin` (:8006), `db` (Postgres 16) and `db-backup`.
- Each app service runs two replicas by default; the gateway retries another healthy replica.
- `diag` and `admin` publish their own ports for the dedicated consoles; the other services stay on the internal network.
- First boot: wait until the services have created their tables, then apply the least-privilege grants and the cross-domain foreign keys as the superuser:

```bash
docker exec -i pinpoint-db psql -U pinpoint -d pinpoint < db/02-grants.sql
docker exec -i pinpoint-db psql -U pinpoint -d pinpoint < db/init/03-fks.sql
```

- Optional demo clients (real headless clients, not server-side seeds):

```bash
docker compose --profile demo up -d alice bob
```

- `db-backup` runs `pg_dump` every five minutes and keeps the last 24 dumps. Postgres is a single primary; streaming replication/Patroni is a production step beyond this file.
- For a self-signed TLS dev gateway: `./docker/gen-certs.sh && docker compose -f docker-compose.yml -f docker-compose.https.yml up -d`.
- The internal gateway name is allowed plaintext only through `PINPOINT_ALLOW_PLAINTEXT_HOSTS=gateway`; every other host requires HTTPS.

### Encrypted backups and restore

The `db-backup` volume holds plaintext dumps. For an encrypted, off-host copy use the wrapper:

```powershell
$env:PINPOINT_BACKUP_PASSPHRASE = "a long passphrase kept outside the repository"
powershell -ExecutionPolicy Bypass -File scripts/backup-db.ps1 -OutDir D:\pinpoint-backups
```

It streams `pg_dump --format=custom` from the db container, encrypts with AES-256-CBC + PBKDF2 (200,000 iterations) through openssl, decrypts the result and compares SHA-256 hashes, and only then deletes the plaintext temporary file. Without `PINPOINT_BACKUP_PASSPHRASE` it refuses to run; it never writes an unencrypted backup. `-SelfTest` checks the crypto path without Docker.

Restore into the running stack:

```powershell
$env:PINPOINT_BACKUP_PASSPHRASE = "the same passphrase"
powershell -ExecutionPolicy Bypass -File scripts/restore-db.ps1 `
  -File D:\pinpoint-backups\pinpoint-20260101T120000.dump.enc -ConfirmRestore
```

The restore script checks the `PGDMP` archive header and runs `pg_restore --clean --if-exists --no-owner`, which **replaces the current objects**. Stop the application services first, rehearse on a disposable database, and keep the passphrase stored separately from the backups. When the encrypted wrapper is the only accepted path, stop the plaintext loop with `docker compose stop db-backup` and protect or delete its volume.

## 4. Production checklist

- Terminate TLS at the gateway (Caddy/nginx) and enable HSTS. HTTPS sessions use `__Host-pinpoint_session` with `Secure`, `HttpOnly`, `SameSite=Lax`, `Path=/` and no `Domain`.
- Set `DATABASE_URL` to a managed Postgres instance; enable backups and monitoring. The server applies migrations.
- Set `PINPOINT_SESSION_IDLE_MINUTES=30` for a public host; the 12 h default is demo-friendly. Keep `PINPOINT_SESSION_MAX_DAYS=30`.
- Set `PINPOINT_TILE_URL` to a self-hosted ODbL tile source. Never use `tile.openstreetmap.org`; the bundled fallback is demo-only.
- Promote the first administrator deliberately; no demo accounts are seeded:

```sql
update pinpoint_users set role = 'administrator' where email = '<real email>';
```

- Keep `PINPOINT_SEARCH_AUTOCOMPLETE` off unless a self-hosted Nominatim autocomplete endpoint is configured.
- Rate limits are per replica: login 30/5 min, register 10/h, anonymous/recovery 20/10 min, diagnostics 10/h, authenticated writes 120/min/account, tiles 1200/min, search/reverse 60/min, route 120/min.
- Run `cargo audit` in CI and review the accepted advisories in `audit.toml`.
- Put a WAF/CDN in front, enable log rotation, and run a pen-test before sharing with real users.

## 5. Desktop build

```bash
cargo build --release --locked
```

The Windows binary is `target\release\pinpoint-desktop.exe`. Run it with:

```bash
pinpoint-desktop.exe --backend https://pinpoint.example
```

- Plain HTTP is accepted only for loopback (`localhost`, `127.0.0.1`, `::1`, Android emulator `10.0.2.2`). Any other host must be HTTPS (ADR-0006).
- `--screenshot <path>` captures a headless smoke screenshot after 120 frames.
- The CI Windows artifact is `pinpoint-desktop-windows.zip` containing `pinpoint-desktop.exe`.
- A packaged build that uses the desktop web assets resolves `web/` next to the executable, from the current directory, or through `PINPOINT_WEB_ROOT`.

## 6. Android

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"
$env:JAVA_HOME = "C:\Program Files\Eclipse Adoptium\jdk-21.0.13+9"
cd android
.\gradlew.bat assembleDebug -Pabi=arm64-v8a
adb install -r -d -t app\build\outputs\apk\debug\app-debug.apk
adb shell am start -n com.pinpoint.desktop/.PinpointActivity
```

- The emulator reaches the host at `http://10.0.2.2:8080`; run `BIND_ADDR=0.0.0.0:8080 cargo run --bin pinpoint-server`.
- The APK is assembled by Gradle, not by cargo-apk's generated APK (ADR-0017). Java is only used for platform APIs with no Rust binding.
- The debug manifest sets `allowBackup=0` and `usesCleartextTraffic=0`; release signing needs the keystore secrets.
- Pushing a `vX.Y.Z` tag that matches `Cargo.toml` builds one signed universal APK (`arm64-v8a` + `armeabi-v7a`), verifies the certificate and creates a draft GitHub release.
- Before the draft, the workflow runs `scripts/check-release.ps1 -Apk` so a versionName/versionCode/ABI mismatch fails the release job.
- The release job checks out the tag, re-runs the same APK check on the downloaded artifact and uses `RELEASE_NOTES.md` as the draft body. It also extracts both Windows ZIPs and checks their exe names and README version.
- A manual `Release` run performs the same checkout/aapt/downloaded-APK check and skips only the draft creation, so the gate can be exercised without a tag.
- The published demo release uses that signed universal `pinpoint-android-3.0.0.apk`; the Gradle debug arm64 APK remains the local device build.

## 7. iOS

The iOS target shares `src/` through `cfg(target_os)` and is packaged with `cargo-xcodebuild`. It needs Xcode and the iOS Rust targets and is not verified in this environment.

## 8. Verification

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
cargo audit
```

- `tests/contract.rs` pins the wire contract; `tests/router_probe.rs` pins routes, authorization, headers and the security behaviors.
- Manual smoke: register two accounts, send and accept a friend request, start a share, publish, read the friend location, revoke it, then verify the friend location disappears.
- Never commit secrets or real user data; tests use fictional `@example.invalid` accounts.

## 9. Honest limitations

- Docker is not available in this environment, so the compose stack is reviewed but not executed here.
- No physical Android device or release keystore is available; the debug arm64 APK is the verified artifact.
- Vercel previews are rate-limited externally; merges are gated on the Rust CI job.
- iOS needs Xcode and is not verified.
- Postgres HA (streaming replica/Patroni) is beyond `docker-compose.yml`; `db-backup` is point-in-time only.
- A hosted Render deployment may lag the repository; redeploy manually after a merge.
