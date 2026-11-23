# Pinpoint — Troubleshooting Guide

This document covers common problems, their causes, and how to fix them.
If you encounter an issue not listed here, check the application error
messages first — Pinpoint surfaces errors in the header bar and as toasts.

---

## 1. App Won't Start

### Symptom: "Cannot start: ..." or immediate exit

**Cause:** The backend URL is invalid or the API client cannot be created.

**Fix:**
1. Check that `--backend` points to a valid URL (e.g. `http://localhost:8000`).
2. Plain HTTP is only allowed for loopback addresses (`localhost`,
   `127.0.0.1`, `::1`). Production backends require HTTPS.
3. If the backend is down, Pinpoint starts in **offline mode** — the map
   still works with demo locations, but publishing/sharing requires a server.

### Symptom: Window opens but is blank or white

**Cause:** GPU/driver issue with egui/glow rendering.

**Fix:**
1. Try setting the environment variable `WEBGPU_BACKEND=dx12` (Windows) or
   `WEBGPU_BACKEND=vulkan` (Linux).
2. Update your graphics drivers.
3. On Linux, ensure `libgl1-mesa-dev` is installed.

---

## 2. Map Issues

### Symptom: No friend locations on the map

**Cause:** Either no friends have shared their location with you, or you're
in offline mode with demo data.

**Fix:**
1. Sign in to see real friend locations (if friends have shared with you).
2. Offline mode shows demo locations (Alice, Bob, Carol) around Chicago.
3. Check that friends have created active location shares with you.

### Symptom: Search returns no results

**Cause:** No internet connection, or Nominatim rate limit.

**Fix:**
1. Verify your internet connection.
2. Try a simpler query (e.g. "café" instead of "Italian restaurant near me").
3. Wait a moment and try again — Nominatim has a 1 request/second limit.

### Symptom: "Navigate" button does nothing

**Cause:** Your default browser is not configured, or pop-ups are blocked.

**Fix:**
1. Set a default browser in your OS settings.
2. Check if your browser blocked a pop-up and allow pop-ups for this app.
3. Try manually copying the URL from the error message (if shown).

---

## 3. Sign-In Issues

### Symptom: "Invalid email or password"

**Cause:** Incorrect credentials or account doesn't exist.

**Fix:**
1. Double-check your email and password.
2. If you don't have an account, click **Register** to create one.
3. Contact your administrator if your account was deactivated.

### Symptom: "Server accepted the credentials but issued no session cookie"

**Cause:** The backend is misconfigured or returning an unexpected response.

**Fix:**
1. Verify the backend URL is correct.
2. Contact the backend administrator — the server must issue session cookies
   on successful login.

### Symptom: Stuck on sign-in screen after clicking "Sign in"

**Cause:** Network request is slow or the server is not responding.

**Fix:**
1. Check your internet connection.
2. Verify the backend is running: `curl http://localhost:8000/health`.
3. Try again — the request may have timed out.

---

## 4. Location Publishing Issues

### Symptom: "Publish" button does nothing

**Cause:** You're not signed in, or the coordinates are invalid.

**Fix:**
1. Sign in first — publishing requires authentication.
2. Enter valid coordinates: latitude between -90 and 90, longitude between
   -180 and 180.
3. Check the error message in the header for details.

### Symptom: Published location doesn't appear on friends' maps

**Cause:** No active location share exists between you and your friend.

**Fix:**
1. Create a location share in the Sharing screen (requires an accepted
   friendship).
2. Ensure the share is active (not paused).
3. Ensure the friend has a `from_friend` or `mutual` share with you.

---

## 5. Friend Request Issues

### Symptom: Friend request sent but never received

**Cause:** The recipient hasn't accepted yet, or the email is wrong.

**Fix:**
1. Check **Sent Requests** in the Friends screen — if it shows
   "awaiting response", the recipient hasn't acted yet.
2. Verify the email address is correct.
3. Ask the recipient to check their **Received Requests**.

### Symptom: Can't accept/reject a request

**Cause:** You're not the recipient of the request, or you're not signed in.

**Fix:**
1. Sign in — you must be authenticated to respond to friend requests.
2. Only the receiver can accept/reject a request.
3. Check **Received Requests** in the Friends screen.

---

## 6. Location Sharing Issues

### Symptom: "Add an accepted friend first" when creating a share

**Cause:** You don't have any accepted friendships.

**Fix:**
1. Send a friend request and wait for it to be accepted.
2. Or ask someone to send you a request and accept it.

### Symptom: Share created but friend can't see your location

**Cause:** The share direction may be `from_friend` (you see them, not vice
versa), or the share is paused.

**Fix:**
1. Check the share direction — `to_friend` means they see you;
   `from_friend` means you see them; `mutual` means both.
2. Ensure the share status is **ACTIVE** (not **PAUSED**).
3. Click **Resume** if the share is paused.

---

## 7. Build & Compile Issues (Developers)

### Symptom: `linker 'link.exe' not found` (Windows)

**Cause:** Visual Studio Build Tools are not installed.

**Fix:**
1. Install Visual Studio Build Tools 2022 with "Desktop development with
   C++" workload.
2. Or use the provided `get_msvc.py` script to download MSVC + Windows SDK
   directly from Microsoft manifests.
3. Configure `.cargo/config.toml` with the linker path and `/LIBPATH:`
   flags.

### Symptom: `cargo clippy` fails with `dead_code` warnings

**Cause:** Unused fields or functions in the code.

**Fix:**
1. Run `cargo clippy --all-targets` to see all warnings.
2. Fix each warning — clippy warnings are treated as errors (`-D warnings`).

### Symptom: `cargo fmt --check` fails

**Cause:** Code is not formatted according to rustfmt standards.

**Fix:**
1. Run `cargo fmt --all` to automatically format all code.
2. Re-run `cargo fmt --all -- --check` to verify.

### Symptom: Tests fail after pulling changes

**Cause:** Dependency tree changed or lock file is out of sync.

**Fix:**
1. Run `cargo update` to update the lock file.
2. Run `cargo build --locked` to verify compilation.
3. Run `cargo test --locked` to run tests.

---

## 8. Docker & Container Issues

### Symptom: Docker engine returns 500 errors

**Cause:** Docker Desktop engine is unhealthy or WSL2 backend is not running.

**Fix:**
1. Start Docker Desktop from the Start menu and wait for it to initialize.
2. Run `docker context use default` to reset the context.
3. If WSL2 is not installed, run `wsl --install` (requires admin).
4. As a fallback, build and run natively without Docker.

### Symptom: `podman` command not found

**Cause:** Podman is not installed.

**Fix:**
1. Install Podman from https://podman.io/.
2. Or use Docker instead — both Dockerfile and Containerfile.podman are
   provided.

### Symptom: Container can't reach the host backend

**Cause:** Wrong hostname for host-to-container communication.

**Fix:**
1. Docker: Use `http://host.docker.internal:8000` as the backend URL.
2. Podman: Use `http://host.containers.internal:8000`.
3. Or use the `mock-api` service in docker-compose for offline development.

---

## 9. Data & Privacy Issues

### Symptom: I want to delete my data

**Cause:** Pinpoint stores the latest publish plus a private, owner-only trail retained for 7 days and pruned hourly.

**Fix:**
1. Location data: each publish overwrites the latest location; the private
   trail is retained for 7 days, pruned hourly, and is visible only to you.
2. Bookmarks: Stored on-device only (localStorage/DataStore). Clear them
   from the app or delete the app data directory:
   - Desktop: `~/.pinpoint/bookmarks.json`
   - Mobile: App DataStore (cleared on app uninstall)
3. Account: delete it yourself in Profile & settings → Danger zone (native)
   or Account → Delete account (web); this erases the profile, shares, trail,
   notes, messages and sessions.

---

## 14. Getting Help

- Check the error messages in the app header first.
- Review this troubleshooting guide.
- Check `docs/USER_GUIDE.md` for feature usage.
- Check `docs/SETUP.md` for installation instructions.
- Check `docs/END_OF_SERVICE.md` for end-of-service reuse options.
- For backend issues, contact your server administrator.

---
