# Proofread - Midterm 2 package

**Checked:** 2026-11-18 11:10 +02:00, branch `docs/midterm2`, HEAD `5285bc6b`
plus uncommitted edits. Code at tag `v2.6.1` (`345aa37f`); live `/health`
returned `{"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}`.
**Scope:** `Midterm2-Report.md`, `Demo-Script.md`, `Presentation.md`,
`README.md`, `slides.html`; cross-checked against `src/`, `tests/`, `web/`,
the live service, GitHub Actions and the existing reviews in this folder.
**Method:** read-only. I re-ran the library harness (`328` tests), the named
integration suites, `node web/app/test/sharing-ui.mjs` (17/17), live `/health`,
a live OSRM query, and the GitHub Actions API. The offline findings reuse the
live network-cut evidence in `reviews/offline.md`; the APK findings reuse
`reviews/artifact.md`.
**Line numbers** are from the snapshot above. The files are being edited in
parallel, so re-anchor against the quoted text, not the number alone.

**Verdict:** the consent core is accurate and the prose is much improved, but
the package is **not submission-ready**. The demo script cannot show B's
location because B never publishes one; the advertised offline reload fails
under a real network cut; the docs deny an Android background-sharing feature
that ships in v2.6.1; the public privacy page says the opposite of the report;
and two author placeholders remain.

---

## P0 - blocks the demo or the submission

### P0.1 B has no location or trail, so the consent and phone steps fail

**Files/lines:** `Demo-Script.md:25-31`, `:35`, `:69-70`, `:80-81`, `:106`,
`:115`; `README.md:45-47`; `slides.html:305-311`; `Presentation.md:165-171`.

**Current text**

- `Demo-Script.md:25` "Seed the new screens in A:" and `:26` publishes A's two
  points. B never publishes anything.
- `Demo-Script.md:69-70` "In Window A ... Point at B's exact
  `47.0245, 28.8322`".
- `Demo-Script.md:35` "Phone ... sign in as B, open the Today tab once";
  `:106` "open the Today tab and show the trail and a text note."
- `Demo-Script.md:115` expects "B exact location `47.0245, 28.8322`".

**Why it is wrong**

`GET /api/v1/locations/friends` only returns users who have a published
`pinpoint_locations` row (`src/server/locations_handlers.rs:576-607`).
Registration creates no location (`src/server/auth_handlers.rs:174-268`).
`47.0245, 28.8322` is A's second seeded point, not B's. In the current script A
sees nothing at `2:00`, and B's phone Today tab is empty at `6:30`.

**Correction (smallest complete fix)**

In `Demo-Script.md` section 1, add to the seed block:

```text
- In B: Map -> publish 47.0245, 28.8322 (this is the point A will see).
```

Then change `:35` and `:106` to use A on the phone, because A owns the
two-point trail and the photo note:

```text
8. Phone: install the 2.6.1 APK, sign in as A, open the Today tab once.
...
1. On the phone: open the Today tab and show A's trail and text note.
```

Update `Presentation.md:170-171` from "B's account" to "A's account".
Update `slides.html:311` to "Close on the phone Today tab (signed in as A)".
If B must be on the phone, seed B with two points and a text note as well and
change every expected value to B's actual point. Do not leave the A coordinate
labelled as B's.

### P0.2 The offline reload step fails under a real network cut

**Files/lines:** `Demo-Script.md:95-98`, `:132-133`, `:145`;
`Midterm2-Report.md:265-266`, `:295`, `:312`; `slides.html:293`, `:310`;
`Presentation.md:145`, `:156`.

**Current text**

- `Demo-Script.md:95` "In A: DevTools -> Network -> Offline -> reload." and
  `:96` "The app shell loads, the installed Moldova map renders, and search
  Bălți returns local results."
- `Demo-Script.md:132-133` "Offline reload fails: load the app twice online
  before going offline so the service worker and OPFS are warm."
- `Midterm2-Report.md:265-266` "Offline web: 7/7 manual assertions (Moldova
  install, offline reload, offline map, offline search)".
- `slides.html:293` "install · reload offline · map · search" and `:310`
  "A: offline reload + search Bălți."

**Why it is wrong**

A real network cut plus reload lands on **Sign in**, not the map: the boot path
calls `GET /api/v1/auth/me`, and `web/sw.js` deliberately does not cache
`/api/` responses. This was reproduced with a whole-browser dead proxy in
`reviews/offline.md:3-11,61-95,240-271`. `page.setOfflineMode(true)` can hide
the bug because the service worker still reaches the network for `/auth/me`;
that is a false positive, not an offline demo. The "load twice online" fallback
does not fix a reload.

**Correction**

Replace `Demo-Script.md:95-99` with a no-reload sequence:

```text
1. Stay on the already signed-in map page in A. Do not reload after going
   offline.
2. DevTools -> Network -> Offline (or disable Wi-Fi for a real network cut).
3. Pan/zoom: the installed Moldova tiles render from OPFS.
4. Search Bălți -> the local index returns the results.
5. Do not press Directions here - offline web routing is not implemented.
6. Go back online and reload.
```

Replace `:132-133` with:

```text
- **Offline reload fails:** the app returns to Sign in because
  `/api/v1/auth/me` is not cached. Use the no-reload sequence, or sign in again.
```

Change `:145` to "A: offline map + search (no reload)".

Change the report's `:265-266` to:

```text
- Offline web: offline pack, map and search pass on an already signed-in page;
  a true offline reload lands on Sign in (known gap). Transcript in
  `reviews/offline.md`.
```

Change `:295` "offline reload + search" to "offline map + search (no reload)";
change `:312` "offline 7/7" to "offline map/search pass; reload gap".
In `slides.html`, change `:293` to "install · offline map · offline search"
with result "pass*" and a footnote "reload returns to Sign in", and remove
"reload" from `:310`. In `Presentation.md`, change the "7/7 offline" claims to
"offline map/search pass; reload gap". Do not call the reload a 7/7 pass.

### P0.3 The public privacy policy and the native Profile screen deny the trail

**Files/lines:** `web/privacy.html:12`, `:18`, `:27`, `:37`, `:39`, `:48`,
`:53-55`; `src/ui/profile.rs:201`, `:213`.

**Current text**

- `web/privacy.html:12` "Effective 16 November 2026 · Version 1.5.0".
- `:18` "no location history".
- `:27` "Overwritten by the next publish; no history stored".
- `:37` "No location history: the server stores only the latest publish."
- `:39` "No friend data sent to OpenStreetMap. Only your own searches, routes
  and map tiles reach the OSM services."
- `:48` "Location is latest-only."
- `:53-55` "there is no location history".
- `src/ui/profile.rs:201` "Location is latest-only; no history is kept."
  `:213` "Location is latest-only; no history is stored."

**Why it is wrong**

v2.6.1 stores a private 24-hour trail (24 hours returned, pruned to 7 days on
the next publish: `src/server/locations_handlers.rs:361-407`,
`src/server/state/locations.rs:232-277`) and private notes. The native Profile
screen contradicts the native Today screen. The policy also says no friend data
reaches OSM, but the explicit "Navigate to friend" action sends the friend's
received coordinates as the route destination (`src/ui/map.rs:360-404`,
`src/ui/route.rs:752-770`). The report now handles this correctly at
`Midterm2-Report.md:170-175`; the public page still says the opposite.

**Correction**

Update `web/privacy.html` to v2.6.1 and add a row:

```text
| Private 24-hour trail | Owner-only points published manually or by the opt-in
Android interval service | 24 hours visible; pruned to 7 days on the next publish |
```

Replace the "no location history" statements with "latest location plus an
owner-only 24-hour trail". Replace the OSM sentence with:

```text
No friend list, account data or trail is sent to OpenStreetMap. A search sends
only the query; a route sends only the endpoints being routed. If you choose
"Navigate to friend", that friend's received coordinates are the route
endpoint.
```

Add trail and notes to the erasure list at `:78`. Change
`src/ui/profile.rs:201,213` to "Latest location plus your private 24-hour trail
(pruned to 7 days); no always-on tracking unless you start Android background
sharing." This is outside the five named files, but it invalidates the report's
privacy claims and the login page links to it.

### P0.4 Author placeholders remain

**Files/lines:** `Midterm2-Report.md:6` (`**Author:** [Your name]`);
`Presentation.md:21` (`I'm [name]`); `Midterm2-Report.pdf` page 1 still prints
`[Your name]`. `Q-and-A.md:236` still contains the instruction "Edit this
answer to match your actual contribution."

**Correction:** put the real author name in the report Markdown, the speaker
notes, and the report PDF; remove the editing instruction from the Q&A answer.
Regenerate the PDF after the change. A submitted report with a bracketed
placeholder looks unfinished.

---

## P1 - factual contradictions and overclaims

### P1.1 Presentation still quotes the native pack size for the web OPFS install

**File/line:** `Presentation.md:110-111`.

**Current:** "Moldova is about 139 megabytes to download and 204 megabytes
installed; the web client stores it in OPFS..."

**Why:** the web manifest has three files: 93,172,399 B PMTiles + 6,770,688 B
search SQLite + 120,319,488 B Valhalla tar = **220,262,575 B (220.3 MB)**
(`web/app/packs/moldova.json`, served by `src/server/offline_handlers.rs:40-49`
and downloaded in full by `web/app/offline.js:599+`). The 138.5/204 figures are
the native two-file catalog (`web/catalog.json:11-33`). The report, demo
expected values and slide 7 already use 220 MB; Presentation is the outlier.

**Correction:**

```text
The offline pack is a published artifact, not a mock. The web OPFS pack is
about 220 MB (93 MB map, 7 MB search index, 120 MB routing graph); the native
pack is 138.5 MB transfer / 204 MB installed. The web client stores it in OPFS
and the service worker serves it with range requests.
```

The slide 7 caption is already correct at `slides.html:233`; align the speaker
notes with it.

### P1.2 The docs say "no background tracking", but Android ships opt-in background sharing

**Files/lines:** `Midterm2-Report.md:170-171`, `:330-331`, `:357-360`;
`Demo-Script.md:162`; `Presentation.md:186`, `:202-203`; `slides.html:333`;
`Q-and-A.md:72-76`; `Handout.md:72`.

**Current:** "It does not store continuous background tracking." "No
continuous background tracking. The trail records explicit publishes, not a
24/7 GPS trace." "There is no 24/7 background GPS and no background upload."
Next step: "Add a background-location design..."

**Why it is wrong:** v2.6.1 has an Android-only, user-started foreground
service that publishes the current location every 15-3600 seconds while the app
is in the background: `src/ui/profile.rs:134-180` ("Background sharing",
Start/Stop, interval), `android/.../PinpointLocationService.java:17-22,65,119`,
and `src/platform.rs:532-538,924-960`. It publishes through
`app.publish_location`, so each fix becomes a trail point. It is opt-in, has a
persistent notification, and can be stopped. It is not "always on by default",
but the absolute "no background upload" and "explicit publishes only" claims
are false.

**Correction (turns a denial into a consent feature):**

Add a scope row:
```text
| Background sharing | Android-only opt-in foreground service; publishes the
latest location every 15-3600 s with a persistent notification and stop
control; each publish is a trail point | Live (Android) |
```

Change the limitation to:
```text
**No always-on tracking by default.** The Android client has an opt-in
foreground service that publishes on an interval while running, with a
persistent notification and a stop control; each publish is a trail point.
Web and desktop do not run it.
```

Change `Demo-Script.md:162` to "No always-on tracking by default; Android
background sharing is opt-in, notified and stoppable. The trail records manual
or interval publishes." Change Presentation `:186` and `:202-203` and slide
`:333` the same way. Change Q&A `:72-76` and Handout `:72`. Replace next step 8
with "Harden the existing Android background sharing (retention, consent copy,
visible indicator, battery/doze behavior, physical-device test)."

### P1.3 Report next step 7 contradicts its own CI evidence

**File/line:** `Midterm2-Report.md:357`.

**Current:** "Fix the macOS installer test race and make all CI jobs
deterministic."

**Why:** the v2.6.1 CI runs (`44265766973`, `44265793804`) show the `rust` job
failing at `Dependency vulnerability audit (cargo-audit)`; `client-macos`,
`wasm` and `release-windows` pass. The report's own 6.4 says this at `:276-279`.
There is no macOS `offline_install` failure in those runs. `audit.toml` records
nine advisories as accepted risk, but the audit job still fails; "accepted"
does not make CI green.

**Correction:**

```text
7. Resolve the dependency audit: RUSTSEC-2026-0543 (chrono) and the other
   accepted advisories must be fixed, pinned or wired into the audit job's
   ignore list, then re-run CI. The macOS job passes on v2.6.1.
```

If a separate macOS flake is still reproducible, cite the failing run;
otherwise remove the macOS claim. Align `Presentation.md:187-188` and
`slides.html:334`: "The audit job is red; nine advisories are documented as
accepted risk in `audit.toml`, but the job still fails." Do not imply CI is
green.

### P1.4 The offline 7/7 verification is contradicted by the live test

See P0.2. The specific count claims to change are
`Midterm2-Report.md:265-266`, `:312`; `slides.html:293`;
`Presentation.md:145`, `:156`. The current evidence supports "offline map and
search pass", not "offline reload passes".

### P1.5 "14/14" is internally contradicted and the 9/9 / 7/7 counts do not match their lists

**Files/lines:** `Midterm2-Report.md:251-256`, `:262-266`;
`Presentation.md:145`, `:153-155`; `slides.html:284`.

**Current:** "14/14 functional checks passed ... The only failed assertion was
the script's expected version string." "Live web UI: 9/9 ... (seven items
listed)." "Offline web: 7/7 ... (four items listed)."

**Why:** a run with a failed assertion is not clean 14/14; the report's own
bullet list in 6.2 has nine bullets, not 14; the UI parenthetical lists seven
checks for a 9/9 count; the offline parenthetical lists four checks for 7/7
and the reload item is false (P0.2). The 24/24 v2.5.2 number is also
unarchived at `:256`, though it is now labelled as such.

**Correction:**

```text
Latest live run on v2.6.1: 14 functional checks were run; one separate harness
assertion failed because it expected 2.6.0 while the server returned 2.6.1.
The transcript is not archived. The earlier v2.5.2 run reported 24/24, also
unarchived.
```

Then either list all nine UI checks and all seven offline checks, or drop the
counts and say "manual checks, transcript not archived". A count that does not
match its list is weaker evidence than no count. On the slide, use "14 checks"
rather than "14/14" and add a footnote "one version-string assertion failed".

### P1.6 Session revocation is only immediate on one replica

**Files/lines:** `Midterm2-Report.md:164-165`; `Presentation.md:231`.

**Current:** "Sessions are revocable; 'sign out everywhere' and password changes
invalidate them."

**Why:** `get_session` returns a locally cached session without re-reading
Postgres (`src/server/state/sessions.rs:107-116`), and `converge` refreshes
users/friendships/shares/locations but not sessions (`src/server/mod.rs:131-145`).
A logout-all or password change on replica A can leave a session cached on
replica B valid until its idle timeout. This was already recorded in
`reviews/report-security-factcheck.md:32`.

**Correction:** scope the sentence to the single-process demo and add a
limitation:

```text
In the single-process demo, logout, logout-all and password changes revoke
sessions immediately. Across replicas, a session cached by another replica can
survive until its idle timeout; this is a known convergence gap.
```

### P1.7 "Append-only audit" and "PII-free reports" are overstated

**Files/lines:** `Midterm2-Report.md:85`, `:200-201`.

**Current:** "append-only audit"; "User-initiated PII-free reports".

**Why:** the admin API has no audit update/delete route, but account erasure
deletes rows where the erased user was the actor or anonymises them
(`src/server/state/users.rs:278`, `:300-302`). Diagnostic reports store the
reporter's numeric id and public handle when signed in
(`src/server/diagnostic_handlers.rs:114-123`), so they are pseudonymous, not
literally PII-free.

**Correction:**

```text
| pinpoint_diagnostic_reports | User-initiated pseudonymous reports (public
handle + numeric id; no email, IP, coordinates or device id) | Admin triage |
...
| pinpoint_admin_audit | Append-only through the admin API; erasure deletes or
anonymises the actor's rows | Read-only audit |
```

### P1.8 The 7-day retention table is still written as a guaranteed window

**File/line:** `Midterm2-Report.md:197`; also `:170-171`.

**Current:** "7 days; 24 hours returned."

**Why:** the PostgreSQL delete runs only when the same user publishes again
(`src/server/state/locations.rs:269-276`); there is no scheduled sweep, and the
in-memory path does not prune at all. The scope table at `:80` already says
"pruned to 7 days on the next publish", but the data table does not.

**Correction:**

```text
| pinpoint_location_history | Owner-only trail points | 24 hours returned; rows
older than 7 days are pruned on the next publish (no scheduled sweep; the
in-memory store does not prune) |
```

Then change `:170-171` from "7 days retained" to "7-day retention target,
pruned on publish". In `Demo-Script.md:87`, change "kept for 24 hours" to
"shown for 24 hours and pruned to 7 days on my next publish"; the web copy at
`web/app/index.html:330` and the `03-today-card` screenshot say the same
24-hour-only claim and should be updated.

### P1.9 Same-origin and loopback-only HTTP are broader than the code

**Files/lines:** `Midterm2-Report.md:177-178`.

**Current:** "plain HTTP is only accepted for loopback development" and "The
web client is same-origin."

**Why:** `src/api.rs` accepts plain HTTP for hosts explicitly listed in
`PINPOINT_ALLOW_PLAINTEXT_HOSTS` as well as loopback. The CSP allows
`https://tiles.openfreemap.org` for the opt-in basemap, so "same-origin" is true
for app assets/API/OSM proxies, not for every request. `reviews/report-security-factcheck.md:73-75`
has the detail.

**Correction:**

```text
Plain HTTP is accepted for loopback and for hosts explicitly listed in
`PINPOINT_ALLOW_PLAINTEXT_HOSTS`; production relies on the TLS terminator. The
app assets, API and OSM proxies are same-origin; the opt-in online basemap
loads OpenFreeMap tiles directly. No third-party scripts or trackers are
loaded.
```

### P1.10 "24 test binaries" is imprecise

**Files/lines:** `Midterm2-Report.md:222`; `Presentation.md:143`, `:150`;
`slides.html:283`; `Handout.md:84`.

**Current:** "328 unit tests and 24 test binaries."

**Why:** under `cargo test --locked` there are 23 test executables (the
`pinpoint-offline-index` binary is feature-gated behind `offline-build`), plus
the doc-test harness; 24 is the number of test-capable Cargo targets. The
library count of 328 is correct (I re-ran `cargo test --lib -- --list`).

**Correction:** say "328 library tests and 24 test targets (13 binary targets,
10 integration suites and the library; `pinpoint-offline-index` is
feature-gated)" or "23 test binaries plus doc-tests". Do not say "24 binaries"
without the composition.

### P1.11 "Offline-first maps" overclaims the product

**File/line:** `Midterm2-Report.md:69`.

**Current:** "**Offline-first maps.** A country pack can be installed
on-device..."

**Why:** the app is online-first; the pack is optional, and the demo itself says
the online basemap is opt-in and the offline pack is the alternative
(`Demo-Script.md:59-60`). "Offline-first" suggests offline is the default.

**Correction:** "**Offline-capable maps.** A country pack can be installed
on-device..." or "**Optional offline maps.**"

### P1.12 The photo-note screenshot does not show a photo; the native screenshot does not show Today

**Files/lines:** `Midterm2-Report.md:310`; `Presentation.md:118`;
`slides.html:252-253`, `:264-265`, `:272`.

**Current:** the report cites `assets/04-note-card.png` as evidence for
"Private text/photo notes"; the slide alt says "Private photo note on the map";
the caption says "Photo downscaled in the browser..." The native caption says
"Today tab with the trail and text notes."

**Why:** the current `04-note-card.png` shows the "Cafe" label, the message, the
map, the coordinates and the action buttons - but no photo. `web/app/app.js:443-448`
renders a photo when `note.photo` exists; this note has none. The
`06-native.png` is the v2.6.0 desktop shell, signed out, with an empty offline
map; its tab strip predates the Today tab (see `reviews/native-today.md:188-198`).
The slide implies the screenshot shows the v2.6.1 Today screen.

**Correction:** either add a real photo to the note and recapture
`04-note-card.png`, or change the alt/caption to "Private note card (text); the
photo path is covered by `tests/today.rs` and the browser check, not this
screenshot." For the native slide, change the caption to "v2.6.0 desktop shell
(signed out); the v2.6.1 Today tab is not pictured", or capture a real native
Today screenshot with an OS-level window capture. `05-mobile.png` is the web
mobile view, not the APK; it shows the fake handle `@lively-forest-5583` and the
old "139 MB" label, so do not use it as a native fallback without redaction and
a label.

### P1.13 The web still has the "stale dot looks live" problem the report opens with

**File/lines:** `Midterm2-Report.md:52-53`, `:64-65`; compare
`src/models.rs:326-336`, `src/map.rs:712-733`, `web/app/app.js:382-384`.

**Current:** the problem section says stale data looks live; the "Honest state"
goal only covers expired/paused/revoked shares.

**Why:** the native map has a live/fresh/stale/dead badge; the web friend list
shows only name, coordinates and precision, no timestamp or freshness state.
A web user still sees an old dot with no age. This is a missing limitation, not
a code bug the docs can ignore.

**Correction:** add to section 9:

```text
**Web freshness.** The native map labels a friend location live/fresh/stale/dead
(`src/models.rs:326-336`); the web friend-locations list shows only the
coordinates and precision, with no age or badge (`web/app/app.js:382-384`). The
web still has the stale-dot problem described in section 1.
```

Add "add the freshness badge to the web friend list" to next steps.

### P1.14 Check-ins and emergencies are native/API-only; the scope table omits that

**File/lines:** `Midterm2-Report.md:24-25`, `:159-160`.

**Current:** the abstract lists check-ins and emergencies; the delivered-scope
table has no safety row; the demo does not show them.

**Why:** the main web app has no check-in or emergency tab; the native client
has Trips and Emergency screens (`src/ui/trips.rs`, `src/ui/emergency.rs`), and
the API routes exist. A reader may assume the web demo covers them.

**Correction:** add a scope row:

```text
| Safety | Owner-scoped check-ins and emergency alerts (API plus native Trips
and Emergency screens); the main web app has no check-in or emergency tab |
Live (native/API) |
```

Add one line to section 9: "The main web app has no check-in or emergency tab;
those flows are native/API-only. The kiosk web client is a prototype, not part
of this demo." This matches `reviews/wiki-alignment.md:48-56`.

### P1.15 The route numbers are stale and over-specified

**Files/lines:** `Demo-Script.md:34`, `:56`, `:114`; `README.md:46`.

**Current:** "136.4 km / 2 h 04", "34 steps, 2 alternatives".

**Why:** a live OSRM query for the same Chișinău -> Bălți endpoints now returns
**138.1 km / 2 h 07 / 36 steps / 2 routes** (one alternative), and OSRM data
changes over time. The UI shows the turn list, not a step count, and the
alternatives panel lists both routes, so "2 alternatives" is off by one. The
native offline screenshot is a different engine (134.4 km / 1 h 38), which
should be labelled as such.

**Correction:** use "about 136-138 km / 2 h 04-2 h 07; verify on the morning of
the demo" and remove the exact step/alternative counts, or say "the turn list
and the GPX/KML/GeoJSON buttons are the evidence". In `README.md:46`, use the
same range. In the report traceability `:311`, label `08-route.png` as the
native local-routing screenshot, not the web OSRM result.

---

## P2 - missing caveats and evidence gaps

### P2.1 The demo double-toggles "Show trail"

`Demo-Script.md:27-29` has the presenter press **Show trail** in pre-flight;
`:85` then says "press **Show trail**" again. If the page was not reloaded, the
button now reads **Hide trail**, so the second press hides the line. Change
`:85` to "confirm the trail is visible (the button should read **Hide trail**;
if it reads **Show trail**, press it)". The parenthetical at `:28` is also
wrong: the web client draws the trail only after **Show trail** and with at
least two points, not merely because points exist.

### P2.2 "Kept for 24 hours" is a privacy disclosure mismatch

`Demo-Script.md:87` says "it is kept for 24 hours"; the database keeps 7 days
and prunes on publish. The web copy at `web/app/index.html:330` and the
`03-today-card` screenshot repeat the 24-hour claim. Use "shown for 24 hours;
pruned to 7 days on the next publish" in the demo, the web copy and any
recaptured screenshot. This is the same issue as P1.8, but it is user-facing
copy, not just the report table.

### P2.3 The README's screenshot-rebuild section is not reproducible

`README.md:53-59` points at
`C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\today-ui-check.mjs` and
`today-ui-live.mjs` "on the author's machine; they are not committed". An
assessor cannot rerun this, and the absolute path exposes a local username.
Either commit sanitized scripts under `docs/midterm2/tools/` with `.invalid`
accounts, or replace the section with "Screenshots are archival, captured
18 November 2026; the capture scripts use temporary profiles/test accounts and
are not committed." Rename "Rebuild the screenshots (optional)" to "Screenshot
provenance" in the second case. Also narrow `README.md:3` "Everything needed"
accordingly.

### P2.4 README and demo claim "no shared passwords in the repository"

`README.md:50-51` and `Demo-Script.md:8-9` say there are no shared passwords in
the repository. `tests/live_api.rs:1-12` still documents the removed Go API and
`alice@example.com / DemoPass123!` demo seed; even though the test is
`#[ignore]`d, the literal claim is false. Narrow it to "no shared live
credentials; test fixtures use `.invalid` accounts", and clean up the stale
`tests/live_api.rs` and `tests/contract.rs` headers that still refer to the Go
backend (`tests/contract.rs:3-8`).

### P2.5 README's `start` command is Windows-only

`README.md:25-27` uses `start docs\midterm2\slides.html`. The project is
multiplatform; say "Open `docs/midterm2/slides.html` in any browser (on Windows,
`start ...`)."

### P2.6 Presentation title slide says "live URL" but the deck has no URL

`Presentation.md:17-18` says the slide 1 visual includes "v2.6.1; live URL";
`slides.html:127-134` has no URL (only slide 13 at `:358`). Either add
`pinpoint-uj4y.onrender.com` to the title meta or remove "live URL" from the
speaker-note visual.

### P2.7 Presentation's CSRF sentence is too broad

`Presentation.md:96` says "every unsafe request carries a CSRF token". The
session middleware checks CSRF only when a session cookie is present
(`src/server/mod.rs:84-102`); login and registration have no session and no
CSRF token. Say "every authenticated unsafe request" or "every unsafe request
made with a session". Slide `:215` says "Session-bound CSRF token on every
unsafe method"; the same qualification applies.

### P2.8 "Account deletion purges both" is ambiguous

`Presentation.md:232` lists three things (latest location, trail, notes) and
then says "account deletion purges both". Say "purges all of it: profile,
friendships, shares, locations, trail, notes, check-ins, emergencies and
sessions."

### P2.9 "Phone widgets" is the wrong term

`Demo-Script.md:136` "**Phone widgets:** the release APK is the ARM build."
There are no widgets in the demo; this is a phone-install note. Change to
"**Phone install:** the release APK is the universal ARM build
(`arm64-v8a` + `armeabi-v7a`). Test one tap before the talk..."

### P2.10 Exact coordinates are rounded in the UI

`Midterm2-Report.md:308` says "Exact coordinates are not rounded" and
`Demo-Script.md:115` expects four decimals. The server payload is unrounded
(`tests/location_consent.rs:630-677`), but the web list formats with
`toFixed(4)` (`web/app/app.js:383`) and the native lists use 4-5 decimals
(`src/map.rs:374`, `src/ui/today.rs:64`). Add "the server preserves full
precision; the UI displays 4-5 decimals" to the traceability row and the demo
expected value. Otherwise a reviewer may think "exact" is a 4-decimal grid.

### P2.11 The native screenshot is v2.6.0 and the README calls the set "current"

`README.md:19` says "Current screenshots"; `Midterm2-Report.md:400-401` says
the native screenshot is the v2.6.0 theme build. Align: "Screenshots (v2.6.0 /
v2.6.1); the native shot is the v2.6.0 shell." Also `README.md:55` says the
screenshots came from "the live web app and the native map harness"; the report
says "demo environment". Pick one provenance sentence and use it in both.

### P2.12 The slide's native screenshot does not show the feature the caption claims

This is the same asset issue as P1.12, but specifically for
`slides.html:264-265,272`: the image is a signed-out v2.6.0 shell with an empty
offline map, while the caption says "Map-first desktop shell" and "Today tab".
Either capture the real v2.6.1 Today screen or say "v2.6.0 shell; the v2.6.1
Today tab is not pictured." The `06-native.png` image also shows "No friends
yet" and no map tiles, so "map-first" is not visually evidenced.

---

## P3 - style, grammar and AI-slop patterns

These are not blockers; they are the lines that read as generated or
over-rehearsed.

1. **`Presentation.md:126` "the personal, on-device-feeling side of the map"** -
   "on-device-feeling" is not idiomatic. Change to "the private, personal side
   of the map - not a social feed."
2. **Repeated "X, not Y" antithesis.** The pattern is used at
   `slides.html:150` ("A share is a decision, not a friendship"), `:167`
   ("One Rust core, three clients..."), `:229` ("A country pack, not a mock"),
   `:280` ("Evidence, not claims"), `:324` ("Explicit edges, honest next
   steps"), `:356` ("Privacy is the product"), and `Presentation.md:214`
   ("privacy can be the product, not a settings page"). Keep the core rule
   ("friendship is not consent") and the closing line; cut "not a mock" and
   vary the rest. The prose is otherwise free of the usual "delve/robust/
   leverage/seamless" vocabulary - the tic is structural, not lexical.
3. **`slides.html:229` and `Presentation.md:110` "not a mock"** is defensive;
   no one claimed a mock. Say "a published country pack" once.
4. **`Presentation.md:32` "genuinely useful"** - cut "genuinely"; it adds
   nothing.
5. **`slides.html:161` "No arrow is the default."** Cryptic. Say "Sharing the
   other way is not implied."
6. **`slides.html:167` "One Rust core, three clients"** is also an overclaim:
   the web client is vanilla JS, not the Rust core. Say "One consent contract,
   three client surfaces" or "One Rust core for native, one web client."
7. **`Presentation.md:135` "The signed 2.6.1 APK ships arm64 and armeabi-v7a"**
   - the signature covers the APK; the ABIs are the packaged native libraries.
   Say "The 2.6.1 APK packages native libraries for arm64-v8a and armeabi-v7a
   and is v2/v3-signed."
8. **"honest" motif.** "Honest state" (`Midterm2-Report.md:64`), "honest
   freshness state" (`:51`), "honest next steps" (`slides.html:324`) - one is
   enough. This is a minor repetition, not a factual issue.
9. **`README.md:26` / P0.4 placeholders** are covered above; they are the
   most visible grammar problem in the package.

---

## Cross-file notes (outside the five named files)

These are not edits to the five files, but they contradict the package and
should be fixed before the defence:

| File:line | Problem | Fix |
|---|---|---|
| `web/privacy.html:12,18,27,37,39,48,53-55` | v1.5.0 policy says no history and no friend data to OSM; false for v2.6.1 | Rewrite as P0.3; bump version/date |
| `src/ui/profile.rs:201,213` | Native Profile says "no history is stored" | Say latest + private 24h trail, pruned to 7 days |
| `Q-and-A.md:72-76` | "no 24/7 background GPS and no background upload" | Android opt-in interval service exists; rewrite as P1.2 |
| `Q-and-A.md:236` | "Edit this answer to match your actual contribution." | Replace with the final answer |
| `Handout.md:72` | "No continuous background tracking" | Same background-sharing caveat |
| `Handout.md:83-85` | 14/14, 9/9, 7/7 offline reload | Align with P1.4/P1.5 |
| `Checklist.md:60` | "Offline reload and search" | Use the no-reload sequence |
| `tests/live_api.rs:1-12` | Header still describes the removed Go API and demo seed accounts | Mark legacy/delete, or rewrite for the Rust API |
| `tests/contract.rs:3-8` | Header still describes the Go server | Update to the Rust wire contract |

The `reviews/` folder is correctly listed in `README.md:18` as supporting
evidence, not submitted prose. If the examiner reads it, note that
`reviews/proofread.md` (this file) is the copy-level review; the deeper
fact-checks are `reviews/artifact.md`, `reviews/report-security-factcheck.md`,
`reviews/report-evidence-factcheck.md`, `reviews/offline.md`,
`reviews/native-today.md` and `reviews/deck-qa.md`.

---

## What I verified, and what I did not

- **Verified:** `cargo test --lib` reports 328 tests; the named suites report
  10/2/11/11/17/1/1; `node web/app/test/sharing-ui.mjs` reports 17/17; live
  `/health` is 2.6.1 with PostgreSQL; APK size/versionCode/ABIs/signature match
  `reviews/artifact.md`; CI run 44265793804 has `rust` failing at cargo-audit
  and `client-macos` passing; live OSRM returns 138.1 km / 2 h 07 / 36 steps;
  the Android background service is in the v2.6.1 tag; `04-note-card.png` has
  no photo; `06-native.png` has no Today tab.
- **Not re-run:** a new authenticated two-account live run, the 9/9 UI and 7/7
  offline runs, or a physical APK install. The existing reviews cover those;
  the offline review's network cut is the strongest evidence that the reload
  step fails.
- **Not edited:** no source, code, asset or other doc was changed. This is the
  only file written.

*Snapshot 2026-11-18 11:10 +02:00. Re-anchor line numbers after further edits;
the quoted text is the stable reference.*

