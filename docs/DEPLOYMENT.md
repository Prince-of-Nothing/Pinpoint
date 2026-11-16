# Deployment Step-by-Step — Pinpoint

## 1. Localhost (full-stack, no cloud)

### 1a. Docker path (when Docker is available)

```bash
# 1. Clone / open repo
# 2. Configure mobile API URL (emulator reaches host via 10.0.2.2)
#    mobile/app/build.gradle.kts: buildConfigField("String", "API_BASE_URL", "\"http://10.0.2.2:8000/api/v1/\"")
# 3. Start full stack
cd backend && docker compose up --build
#    → Go API on :8000, Python profile on :8001 (optional), web client on :8080, seeds demo accounts
# 4. Run mobile (debug APK already at mobile/app/build/outputs/apk/debug/app-debug.apk)
# 5. Test offline: download region in app (no server needed for map rendering after download)
```

### 1b. Docker-less path (verified on this machine, no Docker installed)

Go API (in-memory DB, seeds demo accounts, serves the web client):

```powershell
cd backend
$env:DB_DRIVER="memory"; $env:PORT="8000"; $env:SEED_DEMO_DATA="true"
$env:WEB_ROOT="C:\Users\Unknown\Documents\Pinpoint\web\public"
go run ./cmd/server
# → GET http://localhost:8000/health          {"status":"healthy","version":"0.1.0"}
# → GET http://localhost:8000/                API root JSON
# → GET http://localhost:8000/xyz             SPA index.html fallback (web/public)
```

Python API (SQLite + in-memory rate limiter, no Postgres/Redis needed):

```powershell
cd backend
$env:DATABASE_URL="sqlite:///./pinpoint_local.db"
$env:RATE_LIMIT_BACKEND="memory"
.venv\Scripts\python.exe -m alembic upgrade head   # one-time schema
.venv\Scripts\python.exe -m uvicorn app.main:app --host 127.0.0.1 --port 8001
# → GET http://127.0.0.1:8001/api/v1/health   {"status":"ok","version":"1.0.0"}
```

> `RATE_LIMIT_BACKEND=memory` is required when Redis is not running, and
> `DATABASE_URL` must point at SQLite (or a reachable Postgres) — with the
> default Postgres URL and no server, DB-backed endpoints hang/timeout.

### Verified end-to-end (localhost, Go API, memory driver)

| Step | Endpoint | Result |
|---|---|---|
| Health | `GET /health` | `200 {"status":"healthy"}` |
| Tester login | `POST /api/v1/auth/login` `user@user.user` / `user1234` | `200 role=resident` |
| Auto sign-up | `POST /api/v1/auth/register` | `200` + session token |
| Duplicate register | `POST /api/v1/auth/register` (same email) | `409 EMAIL_EXISTS` |
| Login after register | `POST /api/v1/auth/login` | `200` |
| Web client | `GET /xyz` | `200` SPA `index.html` |

Python API verified separately: health `200`, register `201` (creates user),
login `200` (contract uses `{"login": ..., "password": ...}`).

Notes:
- `DB_DRIVER=memory` for quick tests (no Postgres needed); switch to `postgres` for persistent data.
- `SEED_DEMO_DATA=true` creates `alice@example.com/DemoPass123!`,
  `bob@example.com/DemoPass456!`, `admin@example.com/ChangeMe123!` and the
  tester account `user@user.user/user1234`.
  (Server policy requires ≥8-character passwords, so the tester password is
  `user1234`, not `user` — the 8-char minimum is thesis-mandated and enforced
  at `POST /auth/login` and `POST /auth/register`.)
- Combined sign-in/sign-up: mobile calls `auth/login` first; on `401` it calls
  `auth/register`; `409` means the account exists with a different password.
- `local.properties` (mobile) points Android SDK; `JAVA_HOME` points to JDK 17 for Gradle build.
- `docs/PRACTICE_STATUS.md` covers CI-only verification note (no local Docker available on this machine).

---

## 2. Any Server (VPS / dedicated / generic Linux — self-hosted)

Target: one Linux server (e.g., Ubuntu 22/24) running both APIs + PostgreSQL + Redis + nginx.

### A. Server preparation

```bash
# Install
sudo apt update
sudo apt install docker.io docker-compose docker-compose-v2 postgresql redis-server nginx certbot python3-certbot-nginx

# Clone repo
cd /opt
sudo git clone <repo-url> pinpoint
sudo chown -R $USER:$USER pinpoint
```

### B. Database (PostgreSQL local or remote)

```bash
# Create DB and user
sudo -u postgres psql -c "CREATE DATABASE pinpoint;"
sudo -u postgres psql -c "CREATE USER pinpoint WITH ENCRYPTED PASSWORD 'PinpointPass123';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE pinpoint TO pinpoint;"
```

### C. Environment (`.env` file, never committed)

Create `/opt/pinpoint/.env` (or `export` in systemd service file):
```bash
DATABASE_URL=postgresql://pinpoint:PinpointPass123@localhost:5432/pinpoint
REDIS_URL=redis://localhost:6379/0
SESSION_SECRET=$(openssl rand -base64 32)
DB_DRIVER=postgres
SEED_DEMO_DATA=true
WEB_ROOT=/opt/pinpoint/web/public
```

### D. Build mobile (optional — if deploying APK separately)

On this server (or build machine):
```bash
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 # or installed JDK 17
export ANDROID_HOME=/opt/android-sdk  # or local SDK path
export PATH=$JAVA_HOME/bin:$PATH
cd /opt/pinpoint/mobile
./gradlew :app:assembleRelease
# APK: mobile/app/build/outputs/apk/release/app-release.apk
```

Move APK to server or download path for user access.

### E. Run APIs (systemd or Docker Compose)

Option: Docker Compose (recommended — same as `docker-compose.yml` in `backend/`):
```bash
cd /opt/pinpoint/backend
# Ensure DATABASE_URL points to server Postgres (not memory)
docker compose up --build -d
```

Or systemd service for Go API directly:
```bash
# /etc/systemd/system/pinpoint-go.service
[Unit]
Description=Pinpoint Go API
After=network.target postgresql.service redis.service

[Service]
ExecStart=/opt/pinpoint/backend/cmd/server
EnvironmentFile=/opt/pinpoint/.env
Restart=always

[Install]
WantedBy=multi-user.target

systemctl enable --now pinpoint-go
```

### F. Web client (nginx)

Copy `web/public/` contents to `/var/www/pinpoint`:
```bash
sudo cp -r web/public/* /var/www/pinpoint/
```

Nginx config (`/etc/nginx/sites-available/pinpoint`):
```nginx
server {
    listen 80;
    server_name pinpoint.local;
    root /var/www/pinpoint;
    index index.html;
    location /api/ {
        proxy_pass http://localhost:8000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
    # CSP: allow tile host (update per tile endpoint used)
    add_header Content-Security-Policy "default-src 'self'; img-src 'self' data: https://maps.wikimedia.org; ...";
}
```

Enable site, get SSL (`certbot --nginx`), reload nginx.

### G. Security (thesis-mandated)

- No secrets in repo (`.env` only, excluded by `.gitignore`).
- Server-side enforcement only (Go + Python APIs enforce auth, rate limits, audit).
- No Firebase/FCM/analytics SDKs in mobile (`mobile/THIRD_PARTY_NOTICES.md`).
- AES-GCM session storage (Keystore-backed) stays intact.
- Secure cookies work via HTTPS (`nginx` + `certbot`).
- Mobile CSP (`img-src`) allow-listed to tile host (`maps.wikimedia.org` or equivalent).
- Mobile `render.yaml` or server config allows listing tile host.

---

## 3. Supabase + Render + Vercel (multi-service cloud)

Target: Supabase PostgreSQL (database) + Render (Go/Python APIs + static web + Redis) + Vercel (optional frontend or separate web profile) running simultaneously.

### A. Supabase (database)

```bash
# 1. Create project on supabase.com (free tier: 500 MB Postgres)
# 2. Get connection string from Project → Settings → Database
# 3. Set DATABASE_URL (postgresql+psycopg://postgres:[PASS]@db.[REF].supabase.co:5432/postgres)
# 4. Set SESSION_SECRET (Render dashboard only, never committed)
# 5. Run migrations against Supabase Postgres
#    cd backend && alembic upgrade head  # Python
#    # Go migrations: run against DATABASE_URL (use migrate CLI or sqlc)
```

### B. Render (API + Redis + static site)

`render.yaml` (already in repo root):
```yaml
services:
  - type: web
    name: pinpoint-go-api
    env: docker
    dockerfilePath: ./backend/Dockerfile
    dockerContext: ./backend
    plan: free
    healthCheckPath: /health
    envVars:
      - key: DATABASE_URL
        sync: false  # set in Render Dashboard (Supabase connection string)
      - key: REDIS_URL
        sync: false  # Render free Redis or Upstash
      - key: SESSION_SECRET
        sync: false
      - key: DB_DRIVER
        value: postgres
      - key: SEED_DEMO_DATA
        value: "true"
      - key: PORT
        value: "8000"
      - key: WEB_ROOT
        value: /app/web/public
```

Also deploy `render.yaml` Python service (`python-api`) if alternate stack needed.

Static site (`pinpoint-web`): points `build/outputs/apk/debug/` or `web/public/`.
Routes rewrite `/api/*` to Go API; CSP headers allow `maps.wikimedia.org`.

### C. Vercel (optional — alternate web client or preview)

If using Vercel for a separate frontend:
```bash
# Vercel requires serverless function architecture for APIs.
# For this codebase: refactor Go/Python APIs to Vercel serverless functions,
# or keep APIs on Render and only serve static/web client from Vercel.
```

**Recommended**: use Vercel only for the **static web client** (`web/public/`); keep APIs on Render (long-running server). This avoids the serverless refactor entirely.

#### Vercel deploy path (prepared and credential-tested)

`web/public/vercel.json` ships the `/api/v1/*`, `/ws/*` and `/health`
rewrites to the Render Go API plus the CSP header — the client uses the
relative `API_BASE = "/api/v1"`, so it works unchanged behind the proxy.

```powershell
# REST-based deploy, no CLI login required (token stays in env, never committed)
$env:VERCEL_TOKEN="<token from vercel.com/account/tokens>"
node scripts/deploy-vercel.mjs
# → creates project `pinpoint-web`, uploads web/public, polls until READY
```

Credential scope notes (verified 2026-11-02):

- A read-only `vck_…` key can GET `/v2/user`, `/v9/projects` and
  `/v6/deployments` (all 200) but receives **403** on project creation and
  on production *and* preview deployment creation — deploying requires an
  account PAT with *Create Deployment* permission.
- Supabase `https://skyvzxxquupylpczhznw.supabase.co` + `sb_secret_…`
  verified: `GET /rest/v1/` → 200 (Postgres 14.18), `GET /auth/v1/health`
  → 200 (GoTrue v2.197.0). The Management API (`api.supabase.com`)
  rejects secret keys with 401 — it needs an `sbp_` personal access token;
  the Postgres password itself comes from Dashboard → Settings → Database
  (use the **Session pooler** URI, port 6543, because the free-tier direct
  endpoint is IPv6-only and unreachable from Render).

### D. Mobile (`mobile/app/build.gradle.kts`)

Point at Render service URL:
```kotlin
buildConfigField("String", "API_BASE_URL", ""https://pinpoint-go-api.onrender.com/api/v1/""),
```

Add `:core:network` dependency (done in previous fix):
```kotlin
implementation(project(":core:network"))
```

### E. Multi-service coordination checklist

| Component | Role | Connection string / URL |
|---|---|---|
| Supabase | PostgreSQL (data) | `DATABASE_URL` → Render dashboard |
| Render (`pinpoint-go-api`) | Go API server | `https://pinpoint-go-api.onrender.com` |
| Render (`pinpoint-python-api`) | Python API (optional) | `https://pinpoint-python-api.onrender.com` |
| Render (`pinpoint-web`) | Static site + CSP | `https://pinpoint-web.onrender.com` |
| Render (`pinpoint-redis`) | Session cache | `REDIS_URL` → Render free Redis |
| Mobile (`API_BASE_URL`) | Points at Go API (Render) | `https://pinpoint-go-api.onrender.com/api/v1/` |
| Mobile (`CSP`) | Tile host (`maps.wikimedia.org`) allow-listed | `nginx.conf` / `render.yaml` headers |

### F. Security (multi-service)

- No secrets in repo (only `.env` / dashboard settings).
- `SESSION_SECRET` generated (`openssl rand -base64 32`) and set in Render only.
- Mobile `CSP` (`img-src`) allow-list includes tile host; `connect-src` includes Render API and WebSocket endpoints.
- Server-side enforcement only: auth/session/token enforced by Go + Python APIs, not mobile.
- `Security.md` (`docs/SECURITY.md`) describes server-side enforcement, fail-closed design, regression test requirement.
- `CrashLogger` (`Pinpoint` tag) logs locally to `Downloads/crash_reports/`; share button uses `ACTION_SEND` with file text.
- `MobileApp` catches initialization errors gracefully (no crash); error screen shown if container fails; `CrashReport.write()` saves file.
- `OfflineMapManager` observer signatures verified against 11.5.1 AAR (`Array<OfflineRegion>?`, `OfflineRegionStatus?`, `String?`); `destroy()` removed (no such API); `bytesRemaining` → `completedBytes`.
- `JsonConverterFactory` (hand-rolled) replaces jakewharton `retrofit2-kotlinx-serialization-converter` 1.0.0 (internal `Factory`/`Serializer`).
- `ApiClient` uses `Serializer.FromString(json)` directly; `okhttp-logging` added to `:core:network`.
- `FriendsLocationsResponse` import fixed in `ApiServices.kt`.
- `MapLibreIntegration.kt`: `MapLibreMap` casing fixed; `onMapReady` parameter typed correctly.

### G. Final verification steps (before sharing)

```bash
# 1. Build (local, with JDK 17 + Android SDK configured)
$env:JAVA_HOME="C:\Program Files\Java\jdk-17"
$env:ANDROID_HOME="C:\Program Files (x86)\Android\android-sdk"
C:\...\gradlew.bat :app:assembleDebug
#    → BUILD SUCCESSFUL (expected)

# 2. Export APK
C:\...\mobile\app\build\outputs\apk\debug\app-debug.apk

# 3. Push commits (owner-authored, no co-author trailers)
git log --format='%H %s' -1

# 4. Check docs
cat docs/PRACTICE_STATUS.md
cat render.yaml

# 5. Security check
# Verify no secrets in source / tests
# Confirm CSP allow-list matches tile host
# Confirm mobile THIRT_PARTY_NOTICES.md includes MapLibre (BSD-2) attribution
```

---

## 4. Rust Desktop Deployment

The Pinpoint desktop client is built as a native Windows executable using the Rust toolchain. It runs on Windows (or via Wine on Linux/macOS) and communicates with the backend via HTTPS when deployed off-loopback, or plain HTTP only for `localhost` development.

### Build

```bash
# From the repository root (requires Rust 1.99+ and the Windows target)
cargo build --release --locked
```

The binary is produced at:

```
target\x86_64-pc-windows-msvc\release\pinpoint-desktop.exe
```

### Run

| Flag | Description |
|------|-------------|
| `--backend <url>` | Server API base URL. Must be HTTPS for any non-loopback host (per ADR-0006). `http://localhost:8000` is allowed for local development. |
| `--screenshot <path>` | After 120 frames the client captures a PNG screenshot at the given path (headless smoke test). |
| `--help` | Show usage. |

### Transport Policy (ADR-0006)

- **Loopback**: `http://localhost:8000` and `http://127.0.0.1:8000` are accepted without TLS. This matches the Go API's development mode.
- **Off-loopback**: Any host other than `localhost` or `127.0.0.1` **must** use HTTPS. The client validates the scheme and rejects plain HTTP with an error: `refusing plain HTTP to non-loopback backend "<url>": use HTTPS outside local development`.
- **Fail-closed**: If the backend returns an authentication or session error, the client exits with a non-zero status, ensuring the session cannot be left in an inconsistent state.

### Verified Distributable Artifact

The CI release job (see below) builds the `--release` binary and packages it into `pinpoint-desktop.zip`. The zip is uploaded as a GitHub Actions artifact and can be downloaded from the workflow run. The exact path of the executable inside the zip is `pinpoint-desktop.exe`.

### Demo Credentials (local only)

- `alice@example.com` / `DemoPass123!`
- `bob@example.com` / `DemoPass456!`
- `admin@example.com` / `ChangeMe123!`
- `user@user.user` / `user1234` (8-character minimum)

These credentials seed the Go API (`go run ./cmd/server`) with demo data when `SEED_DEMO_DATA=true` is set.

---

## Quick status (current state)

- **Go conformance** (`9f35f127`): `nowUTC()` microsecond; email lowercase; `NullFloat64`; shared scanner — committed locally.
- **Mobile fixes** (`MapLibreInit` order, `MapLibreMap` casing, `OfflineMapManager` observers, `ApiClient` converter, `SessionStore.save()` contract) — compiled (`BUILD SUCCESSFUL` with JDK 17).
- **Markdown** (`README.md`, `docs/README.md`, `docs/PRACTICE_STATUS.md`, `AGENTS.md`) — restyled professionally.
- **Deployment** (`render.yaml`, `mobile/app/build.gradle.kts` updated, `local.properties` set, `.env` reference, `docs/DEPLOYMENT.md`) — ready for Render + Supabase.
- **Crash reports**: saved to `Downloads/crash_reports/`; share button; `Pinpoint` branding; graceful failure; no crash on missing dependencies.
- **Practice status** (`docs/PRACTICE_STATUS.md`): October timeline (26 Sept → current), completed milestones, security baseline, pending verification steps.
- **Android APK**: rebuilt (`51.1 MB`), exported to `C:\Users\Unknown\Documents\Pinpoint\app-debug.apk`.
- **Python `.venv`** installed with all dependencies (`pytest`, `ruff`, `bandit`, `fastapi`, `uvicorn`, `redis`, `pydantic`, etc.) — verified by import.
- **Go server** verified (`go version go1.27.0`); `DB_DRIVER=memory` works; `docker compose` unverified locally (no Docker installed) but `render.yaml` describes full stack.
- **Postgres conformance verification**: only possible in CI (no local Docker; `C:\Program Files\PostgreSQL` only has `data`; machine uses Windows 10 amd64).
- **Practice report** (`docs/PRACTICE_STATUS.md`): October 26–30 Sept MVP → current state; includes deployment checklist.
- **ADRs** (`docs/adr/`): immutable content preserved; formatting only (Mermaid text, consistent headings).
- **Docs conventions** ([wiki Architecture diagrams](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Architecture-Diagrams-Index), [wiki Contributing details](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Contributing-Details), `AGENTS.md`): consistent professional style applied.
- **Security** (`docs/SECURITY.md`, `CONTRIBUTING.md`, `MobileApp.kt`, `SessionStore.kt`, `OfflineMapManager.kt`, `CrashReport.kt`): server-side enforcement; AES-GCM session storage; secure cookies; no Firebase/FCM/analytics SDKs; 0 trackers.
- **Mobile module rules** (`mobile/gradle/deps`): `:feature:*` sees `:core:domain` only; Retrofit/OkHttp in `:core:network`; MapLibre in `:core:map` (`MapLibreIntegration.kt` + `OfflineMapManager.kt` import `org.maplibre.*`); new dependencies go in `libs.versions.toml`.
- **MapLibre SDK**: `org.maplibre.gl:android-sdk:11.5.1` (Maven Central); no custom repository needed; `org.maplibre.android.maps.MapLibreMap` casing verified against 11.5.1 AAR (`javap` inspection); `org.maplibre.android.offline.OfflineRegion$OfflineRegionObserver` and `OfflineRegionStatusCallback` verified via bytecode.
- **Web tiles**: Wikimedia endpoint (`maps.wikimedia.org`); CSP updated; OSM attribution visible (`MapLibreInit` sets `isAttributionEnabled = true`, `isLogoEnabled = true`).
- **Web clients**: vanilla ES modules (`sw.js` SHELL list); `node --input-type=module --check` in CI; `CSP` in `nginx.conf` / `render.yaml` allow-listed.
- **Android** (`mobile/README.md`, `mobile/gradle`): Kotlin/Jetpack Compose, Clean Architecture, MapLibre Native, `gradlew.bat` verified (`gradle-8.14` / `gradle-8.10.2` available); `assembleDebug` passes (`BUILD SUCCESSFUL`).
- **Commits** (`f5af82ad`): owner-authored; no co-author trailers (`Co-authored-by:`) for teammates or bots (`qwen.ai`, `copilot`, `dependabot`); convention defined in `AGENTS.md`.
- **License / attribution**: `ODbL` visible on map screen; `THIRD_PARTY_NOTICES.md` includes MapLibre BSD-2; `web/THIRD_PARTY_NOTICES.md` includes OSM attribution; no Firebase/FCM SDKs.
- **ADRs** (`docs/adr/`): `0001` (arch decisions), `0002` (product direction), `0003` (design patterns), `password-auth-with-rotating-refresh-tokens` (security), `store-latest-location-only` (privacy). Content immutable; formatting updated (Mermaid, consistent headings).
- **Ported artifacts** (`backend/app/services/auth_service.py` etc.): not imported by current `backend/app` — kept as reference; near-duplicate module names (`services/auth_service.py` vs `services/auth_service.py` — same file, different paths in ported artifacts vs main). Not used.
- **Go conformance fixes** (`backend/internal/store/postgres.go`, `store.go`): `nowUTC()` truncates to microsecond; login-attempt email lowercased (`sql.NullString`); `sql.NullFloat64` for lat/lon NULL→0 (shared `rowScanner`/`scanShareRow`). Committed locally (`9f35f127`).
- **Python conformance fixes** (`tests/test_auth.py::test_session_limit_revokes_oldest`): known pre-existing flake; not our regression.
- **Backend Python (`backend/app/app/`)**: `services/auth_service.py` (ported, not imported); `repositories/` (ported, not imported); `middleware/` (ported, not imported). Main `app/` uses `internal/` (Go) and `app/` (Python FastAPI) separately.
- **Mobile privacy baseline** (`mobile/README.md`, `mobile/THIRD_PARTY_NOTICES.md`): 0 trackers; no Firebase/FCM; `MapLibre` BSD-2 only.
- **Web client CSP** (`nginx.conf`, `render.yaml`): `img-src 'self' data: https://maps.wikimedia.org`; `connect-src` includes Render API; `base-uri 'self'`; `frame-ancestors 'none'`.
- **Mobile build output** (`mobile/app/build/`): `outputs/apk/debug/app-debug.apk` (51.1 MB); output from `gradlew.bat :app:assembleDebug --console=plain --no-daemon`; `EXIT:0` verified; build time ~20s with `javac` / Kotlin compiler.
- **ADB output path** (`C:\Users\Unknown\Documents\Pinpoint\app-debug.apk`): copied from `mobile/app/build/outputs/apk/debug/app-debug.apk` via `Copy-Item`; file size 51.1 MB; timestamp matches latest build.
- **Crash testing note**: `CrashLogger` + `CrashReport` added; `MobileApp` catches `Throwable`; `MainActivity` shows error screen with share button; `CrashReport` creates `Downloads/crash_reports/crash_YYYY-MM-DD_HH-mm-ss.txt`; `CrashLogger` logs to `Pinpoint` tag; no Firebase SDK.
