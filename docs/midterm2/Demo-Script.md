# Pinpoint - Midterm 2 Demo Script

**Format:** live demo, 6-8 minutes (a short cut is at the end).  
**System:** https://pinpoint-uj4y.onrender.com (v2.6.1).  
**Phone APK:** https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1  
**Deck:** [`Presentation.md`](./Presentation.md) and [`slides.html`](./slides.html).  
**Rehearsed:** 18 Nov 2026 against the live system with two fresh email
accounts. Every account created for the rehearsal was deleted afterwards.
Timings in section 9 are measured, not estimated.

The demo uses **two real accounts registered on the live system**. There are no
seeded users, no demo pins and no shared passwords in the repository.

---

## 1. One-time setup (T-15 minutes)

1. Warm the service: open `https://pinpoint-uj4y.onrender.com/health` and
   confirm `"version":"2.6.1"`.
2. Open **Window A** in Chrome profile 1. Open **Window B** in Edge (or Chrome
   guest). Each browser profile has its own `session_id`.
3. Register **A** (presenter) and **B** (friend) with your own emails. The
   generated handles (for example `calm-otter-42`) appear as `@handle` beside
   the email in the top bar. Friend rows show the **display name**, not the
   handle. The drawer (**☰**, bottom-right on the map) holds Friends, Who can
   see me, Friend locations and Private notes.
4. A -> Friends -> enter B's handle -> **Send request**. B -> reload (Ctrl+R)
   -> wait for the **PENDING** badge and the **Accept** button -> **Accept**.
   A -> reload -> wait for **ACCEPTED** and the **Sharing** button. A reload
   fills the lists asynchronously: allow 8-20 seconds on the live deploy.
5. Maps: in **A**, install **Moldova** (A needs the pack for the offline step).
   The button says 139 MB, but the current manifest is 220,262,575 bytes
   (~220 MB / 210 MiB) and took 23-31 s on the live deploy; the page reloads
   automatically when the install finishes and the download banner disappears.
   In **B**, choose **Use online map**; B does not need the pack. The "Choose a
   map" card is intentional. The canvas is ready in about 3 s warm, up to 10 s
   on the first request after idle.
6. Prepare the new screens:
   - In **A**: Map -> publish `47.0105, 28.8638`, then `47.0180, 28.8400`.
     **Clear each number field before typing (Ctrl+A, then type).** Clicking a
     `type=number` field and typing appends digits: the rehearsal produced
     `47.0105470245` on the second publish and the trail showed `47.0105`
     twice. If that happens, clear both fields and publish again.
   - Today updates after each publish; there is **no Refresh button**. Confirm
     **2 points**, newest first (Ctrl+R if the list is stale, then wait), then
     press **Show trail**; the button flips to **Hide trail** and an orange
     line is drawn on the map (the native client always draws the trail).
   - Private notes -> **pan the map first** (the note is saved at the map
     centre) -> add a label, a landmark photo (JPEG/PNG, max 8 MB; no faces or
     personal images) and a message -> **Add note here**. Wait for
     **Private note saved.** and the note row.
   In **B**: Map -> publish `47.0245, 28.8322` with the same field-clearing
   rule. This is the location A will see as B's exact location; without it the
   **Friend locations** list stays empty even while the share is active.
7. Run the route once in A: search **Chișinău** -> first result ->
   **Set as origin** -> **close the place sheet (×)** -> search **Bălți** ->
   first result -> **Directions**. The sheet covers the first search result,
   so the close is required; clicking through it leaves the origin as the
   destination (the rehearsal got `0 m · 1 min`).
   Expect **136.4 km · 2 h 4 min · driving**, 34 steps, and two route buttons
   (`136.4 km · 2 h 4 min` and `133.4 km · 2 h 17 min`).
8. Phone: install the **2.6.1 APK** and sign in as **A**, not B. The 2-point
   trail and the private note belong to A; B's account has only the single
   published point. Open the Today tab once. (Not covered by this rehearsal.)

**Do not use anonymous accounts for the live demo.** On the multi-replica
deployment they can collide or expire immediately.

## 2. Run of show (6-8 min)

### 0:00 - Opening (30 s)

> "Pinpoint is a privacy-first location-sharing app. The rule is simple:
> friendship is not consent. You see my location only when I explicitly share
> it, at the precision I choose, for as long as I choose - and I can stop it
> at any moment."

Show the login page (`assets/01-login.png`) and sign in as A. Sign-in measured
about 7 seconds; wait for the map before talking over it.

### 0:30 - Map, search, route (90 s)

1. The map shows Chișinău: A uses the installed Moldova pack, B uses the
   online basemap. Turn on **Online map** in B if it is off.
2. Search **Chișinău** -> first result -> **Set as origin**.
3. **Close the place sheet (×)**, then search **Bălți** -> first result ->
   **Directions**.
4. Point at **136.4 km · 2 h 4 min · driving** (the UI does not zero-pad the
   minutes), the 34-step turn list, the two route buttons, and the
   GPX/KML/GeoJSON buttons.

> "The map is OpenStreetMap data. The online basemap is opt-in; the offline
> pack is the alternative when there is no connection."

Search took about 6 s for the first local query in A (the pack index initialises
on first use) and under 1 s for Bălți once it is warm; the route summary
appeared about 4 s after **Directions**.

### 2:00 - The consent story (2 min 30 s)

1. In **Window B**: Friends -> A -> **Sharing**.
2. Enable sharing, choose **Exact location**, **1 hour**.
3. Read the summary aloud before accepting: friend, precision, duration. Do
   not read the exact clock minute: the server stamps the expiry when the
   request lands, so the saved row can differ from the dialog by 10-15 s.
4. **Save sharing** and confirm the browser dialog (the exact text is
   `Share exact location with <A> until <date/time>?`).
5. Wait for B's **Who can see me** row to show **ACTIVE** and the **Pause**
   button (up to about 12 s), then in **Window A**: Ctrl+R, wait 8-9 s, open
   the drawer -> **Friend locations**. Point at B's row:
   `<B display name> · 47.0245, 28.8322 · exact`.
6. In **Window B**: **Who can see me** -> show A, **Exact**, **ACTIVE**,
   expiry.
7. Press **Pause**. Wait for the row badge to become **PAUSED** and the button
   to become **Resume** (up to about 25 s). In A: Ctrl+R, wait 8-9 s -> B's
   row is gone ("No friend locations visible.").
8. Press **Resume**. Wait for **ACTIVE** again. In A: Ctrl+R, wait 8-9 s -> B's
   row is back.
9. Press **Revoke**. Wait for "You are not sharing your location." In A:
   Ctrl+R, wait 8-9 s -> gone for good. Reload B: the share is no longer
   active.

> "Friendship alone shows nothing. The server checks the owner, the accepted
> friendship, the active flag, the expiry and the precision on every read."

The share actions measured: save to active row 12 s; B share to A seeing the
exact row 17 s; pause to gone 29 s; resume to back 20 s; revoke to gone 37 s
with a pre-revoke B reload (~29 s when B is already on the row). If you are
behind, skip Resume: pause and revoke already tell the consent story.

Optional: create a **City only** share and show `47.0000, 28.8000` in A; the
precision change is visible. (Code-derived, not rehearsed: the server rounds
to `47.0, 28.8` and the list prints four decimals.)

### 4:30 - Today and private notes (90 s)

1. In A: **Today** -> point at the two-point trail, newest first -> press
   **Show trail** (a reload resets the toggle to Show).
2. Open **Private notes** -> the photo note -> **Show on map**.
3. Say: "The trail is mine only; it is kept for 24 hours. The note and photo
   are on my account, not a third-party service. Only my account can read or
   delete them through the app; they are not end-to-end encrypted."
4. Delete the note after showing it (optional) to demonstrate owner-only
   delete.

The note with a photo took about 19 s from submit to the row appearing.

### 6:00 - Offline (60 s)

1. In A, **with the app already signed in and the map open**: DevTools ->
   Network -> **Offline**.
2. **Clear the search field first**, then search **Bălți**: the installed
   Moldova pack answers from its local index (Bălți, BALTI, Baltika,
   Bălți Oraș, ...). The map canvas stays up.
3. **Do not reload while offline.** A cold reload needs `/api/v1/auth/me`;
   with the network truly down it lands on **Sign in**, even though the app
   shell and "Moldova is installed" load from the service worker. (Verified by
   blocking DNS, not only DevTools Offline.)
4. **Do not press Directions here** - offline web routing is not implemented.
5. Go back online. If you did reload, sign in again and toggle Offline without
   reloading.

> "Map and search work offline; routes are online in the web client. The native
> map server can route on the installed graph."

### 7:00 - Phone and close (45 s)

1. On the phone: open the **Today** tab and show A's trail and text note.
2. Show the theme/map and the signed 2.6.1 APK.
3. Close on the live `/status` page or the health endpoint.

## 3. Expected values

| Check | Expected |
|---|---|
| Route | `136.4 km · 2 h 4 min · driving`, 34 steps, 2 route buttons (`136.4 km · 2 h 4 min`, `133.4 km · 2 h 17 min`) |
| Chișinău first result | `47.0245117, 28.8322923` |
| Bălți first result | `47.7623952, 27.9286022` |
| B exact location | `47.0245, 28.8322` (B must publish it first) |
| Approximate | `47.0200, 28.8300` on screen (server value `47.02, 28.83`) |
| City only | `47.0000, 28.8000` on screen (server value `47.0, 28.8`) |
| Trail | 2 points, newest first, 24-hour window |
| Offline pack | Web OPFS: 220,262,575 bytes (~220 MB / 210 MiB), version `moldova-20261116`, install 23-31 s; the button label says 139 MB. Native pack: 138.5 MB transfer / 204 MB installed. |
| Live health | `"version":"2.6.1"` |

B's exact point is the same as the Chișinău search result (`47.0245,
28.8322`), so the route origin and B's pin coincide. A's trail points are
different. The **Friend locations** row is the proof of the share.

## 4. Fallbacks

- **Blank map:** the "Choose a map" card is intentional. Press **Use online
  map** (verified: hides the card and renders the canvas), or install Moldova.
- **No friend rows after an action:** the app does not poll. Ctrl+R on both
  windows and wait 8-20 s; the lists fill asynchronously. A fixed 1-2 s wait
  will read empty lists.
- **Old cached UI:** Ctrl+Shift+R once; the service worker cache version is
  `pinpoint-offline-v4`.
- **Route fails or says "Routing is busy, try again.":** click **Directions**
  again. The server allows two concurrent routes and caches identical
  endpoints for 5 minutes; a repeated request returned in 2.6 s. The route
  panel only stays in the page until a reload; after that, **Recent routes**
  re-plans it (still online).
- **B share not visible in A:** wait 8-9 s after A's Ctrl+R, then reload B and
  check **Who can see me**. If the share row exists there, reload A again.
  Writes persist; the UI is just slow to refresh.
- **Offline reload lands on Sign in:** expected on a true cold reload. Go
  online, sign in, and toggle Offline **without reloading**; the installed
  pack and local search then work in the loaded tab. The shell and
  "Moldova is installed" still load from the service worker.
- **Share sheet confirmation:** the browser `confirm` dialog is deliberate;
  the Cancel path sends no request (only grant/widen prompts).
- **Phone widgets:** the release APK is the ARM build. Test one tap before the
  talk; if the phone is unusual, fall back to the web demo. Not rehearsed.

## 5. Short cut (about 3:30 measured)

1. Map + route (45 s).
2. B shares exact 1 hour; A sees exact; B pauses; A reloads and sees nothing;
   B revokes (90 s).
3. A: Today trail + one private note (45 s).
4. A: offline toggle + search Bălți, no reload (20 s).

## 6. If the admin console is requested

Admin is **not** demo-ready without database access. To enable it, promote an
already-registered account:

```powershell
$env:DATABASE_URL = "postgres://..."
cargo run --bin pinpoint-admin -- --promote-email owner@example.com
```

Then sign out/in and open **Admin**. Show **Overview** and **Audit** only; the
Users table contains real emails. Do not project it.

## 7. What not to claim

- No continuous background tracking; the trail records explicit publishes.
- No friend-visible notes; notes are owner-only.
- No native photo picker yet.
- No offline web Directions.
- No offline cold start: a true offline reload shows the shell but lands on
  Sign in; map and local search work only in an already-loaded tab.
- No seeded accounts or demo data; real users register.
- No instant list updates: reload and wait 8-20 s.
- MFA can be used at login but is not enrolled in the web UI.

## 8. Selectors (for automation)

| Step | Selector |
|---|---|
| Register | `#register-toggle`, `#register-email`, `#register-name`, `#register-password`, `#register-form button[type=submit]` |
| Sign in | `#login-email`, `#login-password`, `#login-form button[type=submit]` |
| Map | `#empty-online`, `#online-map-toggle`, `#map canvas` |
| Search / place | `#search-input`, `#search-results button.result`, `#place-origin`, `#place-close`, `#place-directions` |
| Route | `#route-from`, `#route-to`, `#route-summary`, `#route-steps li`, `#route-alternatives button`, `#route-gpx`, `#route-kml`, `#route-geojson` |
| Publish | `#publish-lat`, `#publish-lon`, `#publish-form button[type=submit]` |
| Today | `#today-count`, `#trail-list li`, `#trail-toggle` |
| Friends | `#friend-username`, `#friend-form button[type=submit]`, `#friends-list li .badge`, `#friends-list button` |
| Share | `#share-enabled`, `#share-precision`, `#share-duration`, `#share-summary`, `#share-save`, `#shares-list li .badge`, `#shares-list button` |
| Friend locations | `#locations-list li` |
| Notes | `#note-label`, `#note-message`, `#note-photo`, `#note-preview`, `#note-form button[type=submit]`, `#notes-list details` |
| Offline | `#offline-map-banner`, `#offline-map-download`, `#offline-map-status`, `#map-empty-state` |
| Status line | `#status` |

## 9. Rehearsal record (18 Nov 2026)

**Result: PASS** for the web flow: map, search, route, friend request/accept,
exact 1-hour share, exact coordinates, pause/resume/revoke with reloads, Today
trail, note with photo, and offline local search. The phone/APK path and the
on-screen approximate/city-only values were not exercised.

| Action | Measured on the live deploy |
|---|---|
| Register one email account | ~25 s (setup; bcrypt + first request) |
| Sign in / sign out | ~7 s / ~8 s |
| Blank map to online map canvas | 3 s warm; 10 s first request |
| Publish one point | 12-13 s (5 s on a warm second account) |
| Two trail points | 26 s |
| Search Chișinău / Bălți | 6.4 s / 0.3 s with A's pack (5.6 s / 2.5 s online) |
| Directions to route summary | ~4.5 s |
| Ctrl+R to populated lists | 8-9 s warm; 18-20 s first after signup |
| Save share to ACTIVE row | 12 s |
| B share to A seeing exact | 17 s |
| Pause to gone in A | 29 s (including B's refresh and A's reload) |
| Resume to back in A | 20 s |
| Revoke to gone in A | 37 s (including B's pre-revoke reload) |
| Note with photo saved | 19 s |
| Install Moldova pack | 23-31 s (220,262,575 bytes), then auto-reload |
| True offline reload | ~5 s, lands on Sign in |
| Offline search | local results at the first 0.5 s poll |

Timings are wall-clock on the free Render deployment and will vary with
latency; the first request after idle is the slowest. The 6-8 minute run of
show above already budgets for the measured reload waits.
