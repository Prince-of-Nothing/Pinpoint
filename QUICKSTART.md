# Pinpoint — Quick Start

**Release 1.0.0 (2026-11-13).** The web UI has accounts, map, publishing,
friends, shares and sessions; **web search/routing is not in 1.0.0** (the
desktop client has it). See [`RELEASE_NOTES.md`](https://github.com/Prince-of-Nothing/Pinpoint/blob/main/RELEASE_NOTES.md) for the
functional-state matrix and the [roadmap](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Roadmap) for what's next.

This guide gets the pure-Rust stack running: API server (`pinpoint-server`) +
desktop client (`pinpoint-desktop`). The historical Go/Python/Kotlin/web
prototypes were removed from the working tree; consult git history or
ADR-0007/0008 if you need them.

## Overview

Pinpoint is a privacy-first, consent-based location-sharing platform. Friends
can share their real-time location with explicit, revocable permission.

### Architecture

```
┌──────────────────┐   HTTPS /api/v1   ┌──────────────────────┐
│  Rust Client     │ ────────────────▶ │   Rust Server        │
│  (desktop GUI    │                   │   pinpoint-server    │
│   map-first)     │                   │   (port 8000)        │
└──────────────────┘                   │                      │
                                       │   • Auth (login)     │
                                       │   • Friendships      │
                                       │   • Location sharing │
                                       │   • Server-side consent enforcement
                                       └──────────────────────┘
```

## Prerequisites

| Component | Version | Purpose |
|-----------|---------|---------|
| Rust | 1.70+ | Client + server |
| Web Browser | Optional | Browse `http://localhost:8000/` health check |
| Docker / Podman | Optional | Container deployment |

## Step 1: Install Rust

If you don't have Rust installed:

**Windows:**
1. Download `rustup-init.exe` from https://rustup.rs/
2. Run the installer
3. Restart your terminal
4. Verify: `cargo --version`

**macOS/Linux:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify: `cargo --version` should show 1.70+.

## Step 2: Start the Backend

```bash
cargo run --bin pinpoint-server
```

The backend starts on **http://localhost:8000/** with:
- No demo accounts are seeded; register an account on the login screen
- In-memory store (no database needed)
- Pure-Rust Axum server (`src/server/`, `src/bin/server.rs`)

## Step 3: Open the Desktop Client

The Rust client is GUI (`eframe`). Run it in a second terminal:

```bash
cargo run -- --backend http://localhost:8000
```

The map opens immediately; sign in to load friends and publish. The demo
locations are Alice and Bob around Chișinău, Moldova.

The health check `curl http://localhost:8000/health` returns `{"status":"healthy","version":"1.0.0"}` when the server is up.

### What You Can Do Without Logging In

| Feature | Description |
|---------|-------------|
| View map | See demo friend locations with precision indicators |
| Pan/zoom | Interactive map navigation |
| Search places | Search for cafés, pharmacies, parks via OpenStreetMap |
| Navigate | Click "Navigate" to open OpenStreetMap directions in your browser |
| Open on OSM | Open any location directly on OpenStreetMap.org |

### What Requires Sign-In

Click **Sign in** in the top-right to unlock:

| Feature | Description |
|---------|-------------|
| Publish your location | Share your lat/lon with friends |
| Send friend requests | Invite friends by email |
| Accept/reject requests | Manage incoming friend requests |
| Create location shares | Grant friends access to your location (exact/approximate/city) |
| Pause/resume shares | Temporarily stop sharing without deleting |
| Admin panel | (administrator role only) |

## Step 4: Invite Friends

Friends don't need to install anything — they can use the web client on their phone:

1. Start the backend on your computer
2. Find your computer's IP address: `ipconfig` (Windows) or `ifconfig`/`ip addr` (Linux/macOS)
3. Tell friends to visit: `http://<YOUR-IP>:8000/` on their phone browser
4. They can sign in and you can send them friend requests

### Finding Your IP Address

**Windows:**
```powershell
ipconfig | findstr IPv4
```

**macOS/Linux:**
```bash
ifconfig | grep inet
# or
ip addr show
```

Look for an address like `192.168.x.x` or `10.x.x.x`.

## Step 5: Android App (Optional)

If you want a native Android app instead of the web client:

### Prerequisites
- JDK 17
- Android SDK 35
- Android Studio (recommended)

### Build
```bash
cd mobile
./gradlew :app:assembleDebug
```

The APK will be at `app/build/outputs/apk/debug/app-debug.apk`.

Install on your phone:
```bash
adb install app/build/outputs/apk/debug/app-debug.apk
```

The debug APK points to `http://10.0.2.2:8000/api/v1/` (Android emulator). For a real device, change `API_BASE_URL` to your computer's IP.

## Step 6: Docker Deployment (Optional)

If you prefer Docker:

```bash
docker compose up --build
```

This starts:
- Go backend on port 8000
- Web client served via nginx on port 8080

Access: http://localhost:8080/

### Docker Compose Profiles

| Profile | Purpose |
|---------|---------|
| (default) | Go backend + web client |
| python | Python/FastAPI backend alternative |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "go: command not found" | Install Go 1.23+ and add to PATH |
| Backend won't start | Check port 8000 is free: `netstat -ano \| findstr :8000` (Windows) or `lsof -i :8000` (Linux/Mac) |
| Web client won't load | Make sure backend is running: `curl http://localhost:8000/health` |
| Can't connect from phone | Ensure firewall allows port 8000; use your computer's IP, not localhost |
| Login fails | Register a new account. Passwords need at least 8 characters and are case-sensitive. |
| Map tiles don't load | Internet required for tile loading; attribution is shown per ODbL |
| Search doesn't work | Nominatim rate limit (1 req/s); wait and try again |

## Security Notes

- ✅ No trackers, no analytics, no advertising SDKs
- ✅ All consent decisions enforced server-side
- ✅ Session cookies are HttpOnly, SameSite=Lax
- ✅ CSRF double-submit on unsafe methods
- ✅ Plain HTTP only for loopback (production requires HTTPS)
- ✅ Friend locations are latest-only (no history stored)
- ✅ Directional consent: sharer controls precision and can revoke anytime
- ✅ Bookmarks stored on-device only, never synced

## Data & Privacy

| Data | Stored Where | Retention |
|------|-------------|-----------|
| Account credentials | Server (hashed) | Until account deletion |
| Friend locations | Server (latest only) | Overwritten on publish |
| Bookmarks | Your device only | Until you delete them |
| Search/route queries | Session only | Not stored |

## Support

- **User Guide:** `docs/USER_GUIDE.md`
- **Troubleshooting:** `docs/TROUBLESHOOTING.md`
- **Full Documentation:** [Pinpoint wiki](https://github.com/Prince-of-Nothing/Pinpoint/wiki)
- **Features:** `docs/FEATURES.md`
- **End of Service:** `docs/END_OF_SERVICE.md`

## License

MIT © 2026 Pinpoint
