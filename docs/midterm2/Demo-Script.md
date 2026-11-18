# Pinpoint - Midterm 2 Demo Script

**Format:** live demo, 6-8 minutes (a 3-minute cut is at the end).  
**System:** https://pinpoint-uj4y.onrender.com (v2.6.1).  
**Phone APK:** https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1  
**Deck:** [`Presentation.md`](./Presentation.md) and [`slides.html`](./slides.html).

The demo uses **two real accounts registered on the live system**. There are no
seeded users, no demo pins and no shared passwords in the repository.

---

## 1. One-time setup (T-15 minutes)

1. Warm the service: open `https://pinpoint-uj4y.onrender.com/health` and
   confirm `"version":"2.6.1"`.
2. Open **Window A** in Chrome profile 1. Open **Window B** in Edge (or Chrome
   guest). Each browser profile has its own `session_id`.
3. Register **A** (presenter) and **B** (friend) with your own emails. Note the
   generated handles (for example `calm-otter-42`).
4. A -> Friends -> enter B's handle -> **Send request**. B -> reload (Ctrl+R)
   -> **Accept**. A -> reload.
5. Maps: in **A**, install **Moldova**, wait for "Moldova is installed", then
   reload (A needs the pack for the offline step). In **B**, choose **Use online
   map**; B does not need the pack.
6. Prepare the new screens:
   - Map in A -> publish `47.0105, 28.8638` and `47.0180, 28.8400`.
   - Map in B -> publish `47.0245, 28.8322` (this is the point A will see).
   - Today in A -> confirm two points (Ctrl+R if the page is stale) and press
     **Show trail** (the current web client draws the trail when the points are
     present; the native client always draws it on the map).
   - Private notes in A -> add a note with a landmark photo (no faces or
     personal images) and a short message.
7. Run the route once in A so the route panel is warm: search **Chișinău** ->
   first result -> **Set as origin**; search **Bălți** -> first result ->
   **Directions**. Expect **136.4 km / 2 h 04**.
8. Phone: install the **2.6.1 APK**, sign in as B, open the Today tab once.

**Do not use anonymous accounts for the live demo.** On the multi-replica
deployment they can collide or expire immediately.

## 2. Run of show (6-8 min)

### 0:00 - Opening (30 s)

> "Pinpoint is a privacy-first location-sharing app. The rule is simple:
> friendship is not consent. You see my location only when I explicitly share
> it, at the precision I choose, for as long as I choose - and I can stop it
> at any moment."

Show the login page (`assets/01-login.png`) and sign in as A.

### 0:30 - Map, search, route (90 s)

1. Turn on the **Online map** if it is off; the map shows Chișinău.
2. Search **Chișinău** -> first result -> **Set as origin**.
3. Search **Bălți** -> first result -> **Directions**.
4. Point at **136.4 km / 2 h 04**, the turn list, and the GPX/KML/GeoJSON
   buttons.

> "The map is OpenStreetMap data. The online basemap is opt-in; the offline
> pack is the alternative when there is no connection."

### 2:00 - The consent story (2 min)

1. In **Window B**: Friends -> A -> **Sharing**.
2. Enable sharing, choose **Exact location**, **1 hour**.
3. Read the confirmation summary aloud before accepting: friend, precision,
   expiry.
4. Save and confirm the browser dialog.
5. In **Window A**: Ctrl+R. Open the drawer -> **Friend locations**. Point at
   B's exact `47.0245, 28.8322` and the **exact** label.
6. In **Window B**: **Who can see me** -> show A, Exact, **ACTIVE**, expiry.
7. Press **Pause**. In A: Ctrl+R -> B's pin/location is gone.
8. Press **Resume**. In A: Ctrl+R -> back.
9. Press **Revoke**. In A: Ctrl+R -> gone for good. Reload B: the share is no
   longer active.

> "Friendship alone shows nothing. The server checks the owner, the accepted
> friendship, the active flag, the expiry and the precision on every read."

Optional: create a **City only** share and show `47.0, 28.8` in A; the precision
change is visible.

### 4:00 - Today and private notes (90 s)

1. In A: **Today** -> point at the two-point trail and press **Show trail**.
2. Open **Private notes** -> the photo note -> **Show on map**.
3. Say: "The trail is mine only; it is kept for 24 hours. The note and photo
   are on my account, not a third-party service. Only my account can read or
   delete them through the app; they are not end-to-end encrypted."
4. Delete the note after showing it (optional) to demonstrate owner-only
   delete.

### 5:30 - Offline (60 s)

1. In A: DevTools -> Network -> **Offline**. Do **not** reload: `/api/v1/auth/me`
   is not cached, so a reload returns to Sign in.
2. The installed Moldova map stays on screen. Clear the search field first,
   then search **Bălți**; it returns local results.
3. **Do not press Directions here** - offline web routing is not implemented.
4. Go back online.

> "Map and search work offline; routes are online in the web client. The native
> map server can route on the installed graph."

### 6:30 - Phone and close (45 s)

1. On the phone: open the **Today** tab and show the trail and a text note.
2. Show the theme/map and the signed 2.6.1 APK.
3. Close on the live `/status` page or the health endpoint.

## 3. Expected values

| Check | Expected |
|---|---|
| Route | 136.4 km, 2 h 04, driving, 34 steps, 2 alternatives |
| B exact location | `47.0245, 28.8322` |
| Approximate | `47.02, 28.83` |
| City only | `47.0, 28.8` |
| Trail | 2 points, newest first, 24-hour window |
| Offline pack | Moldova installed in web OPFS (~220 MB); native pack 138.5/204 MB |
| Live health | `"version":"2.6.1"` |

## 4. Fallbacks

- **Blank map:** the "Choose a map" card is intentional. Press **Use online
  map**, or install Moldova and reload.
- **No friend rows after an action:** the app does not poll. Ctrl+R on both
  windows.
- **Old cached UI:** Ctrl+Shift+R once; the service worker cache version is
  `pinpoint-offline-v3`.
- **Route fails:** use the pre-run route panel; it stays in the page. The
  online route uses public OSRM.
- **Offline after a reload:** `/api/v1/auth/me` is not cached, so the app
  returns to Sign in. Do not reload offline; go offline after the map and pack
  are loaded.
- **Share sheet confirmation:** the browser `confirm` dialog is deliberate; the
  Cancel path sends no request.
- **Phone widgets:** the release APK is the ARM build. Test one tap before the
  talk; if the phone is unusual, fall back to the web demo.

## 5. 3-minute cut

1. Map + route (45 s).
2. B shares exact 1 hour; A sees exact; B pauses; A reloads and sees nothing;
   B revokes (60 s).
3. A: Today trail + one private note (45 s).
4. A: go offline (no reload), map + search Bălți (30 s).

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
- No seeded accounts or demo data; real users register.
- MFA can be used at login but is not enrolled in the web UI.
