# Offline demo verification - Midterm 2

**Verdict: FAIL for the 5:30 offline step as written.** The live app shell is
served from the service-worker cache, but a real network cut plus reload lands
on **Sign in**; the signed-in map and the in-app search box are not available.

**The offline data path itself passes.** With the network genuinely cut, the
installed Moldova pack is read from OPFS, PMTiles range requests return `206`,
and the local SQLite search index returns eight results for "Balti", including
Bălți. The standalone pack manager renders Moldova tiles from OPFS.

**The corrected no-reload path also passes.** Keeping the already signed-in
page open, then cutting the network, gives eight local "Balti" results in the
real app search box and keeps the map on the installed pack. This is the
recommended demo sequence (see §6.1); the only extra step is to clear the
search field before typing the offline query.

Tested on 18 November 2026 against `https://pinpoint-uj4y.onrender.com`
(`/health` returned `{"database":"connected","mode":"postgres","status":"healthy","version":"2.6.1"}`).

---

## 1. What was tested

- Reused an existing OPFS profile instead of downloading 220 MB again. No
  install button was clicked in this run. The profile contained
  `moldova-20261116`, installed at
  `2026-11-17T20:15:55.010Z`, with the three live manifest files:
  - `moldova.pmtiles` - 93,172,399 bytes
  - `moldova-search.sqlite` - 6,770,688 bytes
  - `moldova-valhalla.tar` - 120,319,488 bytes
  - total: **220,262,575 bytes** (the manifest's own sum).
- Registered a fresh throwaway email account through the UI
  (`pinpoint-offline-review-1794992138533@example.invalid`) and deleted it at
  the end. Three more throwaway accounts were used for the no-reload and
  diagnostic runs; all were deleted. A direct login request for every test
  email now returns `401 INVALID_CREDENTIALS`.
- Did **not** press Directions offline. No route or `place-directions` control
  was touched after going offline.
- Did not test friend/sharing flows; they are outside this review.

The profile and screenshots are temporary artifacts, not committed:

- Run directory:
  `C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\pinpoint-verify\offline-review-20261118-105116`
- Profile copy (reused OPFS pack):
  `...\offline-review-20261118-105116\profile`
- Main script: `C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\offline-review.mjs`
- Timing script: `C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\offline-timing.mjs`
- Cleanup script: `C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\offline-cleanup.mjs`
- Machine-readable evidence: `report.json`, `cleanup.json`, `timings.json`,
  `module-check.json`, `warm-final.json` in the run directory.

---

## 2. Results

| Check | Result | Detail |
|---|---|---|
| Live app root loads | PASS | `Pinpoint - private offline map` in 199 ms |
| Throwaway email registration | PASS | submit to app panel in 8,928 ms |
| Existing OPFS pack is recognised | PASS | Moldova `moldova-20261116`, 220,262,575 bytes, 3 files |
| "Choose a map" card after install | PASS | `#offline-map-banner`, `#offline-map-download` and `#map-empty-state` all hidden |
| Online map canvas | PASS | 1440 × 940 CSS px, SW controlling |
| Online search "Balti" | PASS | 8 results in 2,549 ms |
| Online reload keeps the session | PASS | app panel visible, pack still listed |
| App root is cached by the SW | PASS | `caches.match("/")` returned a 200 `basic` response |
| Offline navigation loads a document | PASS | 164 ms; title `Pinpoint - private offline map` |
| **Signed-in app survives an offline reload** | **FAIL** | `#login-panel` visible, `#app-panel` hidden |
| `/api/v1/auth/me` during that load | expected failure | `net::ERR_FAILED` (no cached API response; `/api/` is excluded from `sw.js`) |
| PMTiles Range from OPFS while offline | PASS | `206`, 65 bytes, magic `PMTiles` |
| SQLite Range from OPFS while offline | PASS | `206`, 16 bytes, magic `SQLite format 3` |
| Offline search engine | PASS | 8 hits in 55-61 ms; first hit Bălți at 47.7624, 27.9286 |
| App's own `getOfflineSearch` wrapper | PASS | 8 hits in 80 ms with the network cut |
| Standalone offline map from OPFS | PASS | `Moldova - offline vector tiles`; SW `networkFetches: 0` |
| Warm app, no reload: search after the cut | PASS | 8 results in the real `#search-results` list; field must be cleared first |
| Account deletion | PASS | UI returned `Account deleted.`; login API returned 401 for all test emails |

Offline error log from the failing step:

```text
failed request: https://pinpoint-uj4y.onrender.com/api/v1/auth/me  net::ERR_FAILED
#login-panel visible, #app-panel hidden
navigator.serviceWorker.controller = https://pinpoint-uj4y.onrender.com/sw.js
```

The page gives no offline explanation: `#status` is empty and the login form
looks like a normal signed-out reload.

Why this happens:

- `web/app/app.js` (initial boot, around lines 1182-1191) calls
  `GET /api/v1/auth/me`; on any failure it shows `#login-panel` and hides
  `#app-panel`.
- `web/sw.js` deliberately does not cache `/api/` responses (`cacheable` in
  `serveShell` excludes `url.pathname.startsWith("/api/")`).
- Therefore the service worker can serve the HTML/JS/CSS shell and the OPFS
  pack, but it cannot reconstruct the session from the network. A real
  offline reload is a login screen, not the map.

**Important test trap:** `page.setOfflineMode(true)` (the same page-level CDP
network condition DevTools uses) applies to the page target; it does not by
itself guarantee that the service worker's network access is cut. A page-only
"offline" reload can therefore pass for the wrong reason. That is not evidence
that the demo works without a connection. This review cut the network for the
whole browser with a dead forward proxy
(`--proxy-server=http://127.0.0.1:1 --proxy-bypass-list=<-loopback>`), so the
service worker's own network fetches failed too.

---

## 3. Timings

| Step | Time |
|---|---|
| Live root load (warm Render instance) | 199 ms |
| Register -> app panel | 8,928 ms |
| Online search "Balti" (first local SQLite open) | 2,549 ms |
| True offline navigation from SW cache | 164 ms |
| Offline SQLite open + `search("Balti", 8)` | 55-61 ms |
| Offline manager ready (`window.__offlineDemo.ready`) | 296 ms |
| Offline Moldova map ready (`window.__offlineMapReady`) | 215 ms |
| Recenter to Chișinău + tiles loaded | 418 ms |

The first search is the slow one because it loads the 6.8 MB SQLite index and
the SQLite WASM engine. Later searches in the same page are local and fast.

---

## 4. Exact steps and selectors

All steps used Puppeteer 25.16 with Chrome for Testing 156.0.8078.47, headless,
viewport 1440 × 1000. The profile was copied from the prior offline run.

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
node C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\offline-review.mjs <run-dir>
```

The warm-page check and the timing check are separate scripts in the same
temporary directory (`offline-warm-final.mjs`, `offline-timing.mjs`).

### 4.1 Online setup

1. `page.goto("https://pinpoint-uj4y.onrender.com/")`.
2. Clear the profile's old session:
   `const cdp = await page.createCDPSession(); await cdp.send("Network.clearBrowserCookies");`
   then `page.reload()` and wait for `#login-panel:not(.hidden)`.
3. Click `#register-toggle`; fill `#register-email`, `#register-name`,
   `#register-password`; click `#register-form button[type=submit]`; wait for
   `#app-panel:not(.hidden)`.
4. Read the pack with the app's own module:
   `const { listPacks } = await import("/app/offline.js"); await listPacks();`
5. Check the installed state:
   - `#offline-map-banner` has class `hidden`
   - `#offline-map-download` has `hidden = true`
   - `#map-empty-state` has class `hidden`
6. Search online to warm the search modules: click `#search-input`, type
   `Balti`, press `Enter`, wait for `#search-results button.result`.
7. Reload online once and wait for `#app-panel:not(.hidden)`; check
   `navigator.serviceWorker.controller` and
   `caches.match("/")` (both present).

### 4.2 Real offline reload

1. Close the browser, keep the same profile.
2. Relaunch with
   `--proxy-server=http://127.0.0.1:1 --proxy-bypass-list=<-loopback>`.
   Requesting a dead proxy fails every network request, including fetches made
   by the service worker.
3. `page.goto("https://pinpoint-uj4y.onrender.com/")` with a 60 s timeout.
   The navigation is served from the SW cache in 164 ms.
4. Wait 6 s. Check `#login-panel` / `#app-panel`; the observed state is
   `login=true`, `app=false`. The failed request is
   `GET /api/v1/auth/me -> net::ERR_FAILED`.

### 4.3 Offline pack checks (no UI needed)

Run in the page context with the network still dead:

```js
const { listPacks, openPack } = await import("/app/offline.js");
const packs = await listPacks();                       // Moldova, 220262575 bytes

const pm = await fetch("/offline-packs/moldova/moldova.pmtiles", {
  headers: { Range: "bytes=0-64" },
});
const pmBytes = new Uint8Array(await pm.arrayBuffer()); // 206, "PMTiles"

const sq = await fetch("/offline-packs/moldova/moldova-search.sqlite", {
  headers: { Range: "bytes=0-15" },
});
const sqBytes = new Uint8Array(await sq.arrayBuffer()); // 206, "SQLite format 3"

const { openSearch } = await import("/map-assets/search.js");
const engine = await openSearch({
  sqliteUrl: "/offline-packs/moldova/moldova-search.sqlite",
});
const hits = engine.search("Balti", 8);                 // 8 local hits
```

The ASCII query `Balti` is enough; the index folds diacritics and returns
`Bălți`. The app's own wrapper was checked too:

```js
const { getOfflineSearch } = await import("/offline-search.js");
const search = await getOfflineSearch();                 // 80 ms
const hits = search.search("Balti", 8);                  // 8 local hits
```

### 4.4 Offline map render

1. Navigate to `/app/offline-demo.html` (precached by the SW).
2. Wait for `window.__offlineDemo.ready === true`.
3. `window.__offlineDemo.open("moldova")`; wait for
   `window.__offlineMapReady === true`.
4. Recenter: `window.__offlineDemo.map.getMap().jumpTo({ center: [28.8638, 47.0105], zoom: 12 })`;
   wait for `map.loaded() && map.areTilesLoaded()`.
5. The status is `Moldova - offline vector tiles`. SW stats:
   `packRequests: 14`, `pack206: 13`, `pack200: 1`, `pack404: 0`,
   `pack416: 0`, `shellCacheHits: 30`, `networkFetches: 0`,
   `offlineFallbacks: 1`.

The manager's left manifest panel still says Monaco because the standalone
demo boots with the Monaco fixture selected; the map status and the
"Installed on this device" row say Moldova. That is a cosmetic demo quirk, not
an app failure.

### 4.5 Warm-page no-reload path (the recommended correction)

An additional throwaway account was used for this check.

1. Register and wait for `#app-panel:not(.hidden)` and the map canvas.
2. Warm the local search engine with the app's wrapper:
   `const { getOfflineSearch } = await import("/offline-search.js"); await getOfflineSearch().search("Balti", 8);`
3. Cut the network for both the page and the service-worker target:
   `page.setOfflineMode(true)` plus
   `Network.emulateNetworkConditions({ offline: true, ... })` on the
   `service_worker` target. The app's probe
   `fetch("/api/v1/auth/me", { cache: "no-store" })` failed with
   `TypeError: Failed to fetch`. The whole-browser dead-proxy run in §4.2 is
   the definitive network-cut test; this run checks the no-reload UI path.
4. Clear the search field (`Ctrl+A`, `Backspace`), type `Balti`, press `Enter`.
5. The app stayed signed in and returned eight results:
   `Bălți | BALTI | BALTI | Baltika | Bălți Oraș | Bălți Oraș | Metan Bălți | AUTO MALL Bălți`.
   `fetch("/offline-packs/moldova/moldova.pmtiles", { headers: { Range: "bytes=0-64" } })`
   still returned `206`.
6. Restore the network and delete the account.

The warm app screenshot shows the already-rendered map, centred on Chișinău
rather than the Monaco fixture, so the installed Moldova pack is the active
source. New tile reads after the cut were proved separately by the standalone
manager recenter (§4.4) and the `206` PMTiles probe; the warm-page run did not
pan the app map.

Do not skip step 4. The field is not cleared automatically after a search; an
earlier run that reused the field submitted `BaltiBalti` and returned
"No places found near here." Clear it before the offline query.

### 4.6 Cleanup

1. Relaunch online with the same profile.
2. Sign in with the throwaway email/password (the session cookie is a browser
   session cookie and did not survive the browser restart).
3. Open the drawer if needed (`#drawer-toggle`), scroll to `#account-card`,
   click `#account-card details.danger-zone summary`.
4. Type `DELETE` into `#delete-confirm`, type the password into
   `#delete-password`, click `#delete-account`; accept the browser `confirm`.
5. Wait for `#login-panel:not(.hidden)`; status is `Account deleted.`.
6. A separate login request returns `401 INVALID_CREDENTIALS`.

---

## 5. Screenshots

All paths are under
`C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\pinpoint-verify\offline-review-20261118-105116\shots\`.

| File | What it shows |
|---|---|
| `01-login.png` | Fresh login page after clearing the reused profile's cookie |
| `02-registered-map.png` | Throwaway account signed in; Moldova pack installed; map canvas |
| `03-online-search.png` | Online "Balti" search with eight results |
| `04-online-warm.png` | Online reload; signed in; pack still installed |
| `05-true-offline-reload.png` | **The failure:** real offline reload lands on Sign in |
| `06-offline-demo-map.png` | Offline standalone manager rendering Moldova tiles from OPFS |
| `07-delete-account-form.png` | Delete-account confirmation form |
| `08-account-deleted.png` | `Account deleted.` and the login panel |
| `12-warm-debug-offline-search.png` | Negative control: same cut, field became `BaltiBalti`; "No places found near here." |
| `13-warm-offline-search-final.png` | **The fix:** warm signed-in page, network cut, eight local "Balti" results in the real app |

The key pair is `05-true-offline-reload.png` (the app gate fails) and
`13-warm-offline-search-final.png` (the no-reload demo path works).

---

## 6. Corrections for `Demo-Script.md`

### 6.1 The offline step cannot use a reload

Replace the current §5:30 steps 1-3:

```text
1. In A: DevTools -> Network -> Offline -> reload.
2. The app shell loads, the installed Moldova map renders, and search
   Bălți returns local results.
3. Do not press Directions here - offline web routing is not implemented.
4. Go back online.
```

with a version that does not destroy the session:

```text
1. Stay on the already signed-in map page in A. Do not reload after going
   offline.
2. DevTools -> Network -> Offline (or disable Wi-Fi for a real network cut).
3. Clear the search field (Ctrl+A, Backspace), type Balti, press Enter.
   The local index returns the Bălți results.
4. Pan/zoom: the installed Moldova tiles are read from OPFS.
5. Do not press Directions here - offline web routing is not implemented.
6. Go back online.
```

This corrected sequence was verified with the page and service-worker targets
set offline: the real app stayed signed in and `#search-results` returned
eight rows, starting with Bălți. The field must be cleared first; reusing the
old query submitted `BaltiBalti` and gave "No places found near here." The
0:30 route step already searches Bălți, which also warms the local search
engine; keep that step before the offline step.

The reload is not a shell-cache problem. The shell is cached and loads in
164 ms; the app then calls `/api/v1/auth/me`, which has no offline response,
so it deliberately shows the login panel. "Load the app twice online" in the
fallback section does not fix this. Either remove the reload from the script,
or change the app so an established session can boot offline (cache a minimal
user record for offline display, or add an explicit "offline session" state;
keep all server-side authorization as it is). A code change needs a new test
and a re-run of this check.

The 3-minute cut has the same problem: its step 4 says
`A: offline reload + search Bălți (30 s)`. Change it to "A: stay on the
signed-in page, go offline, clear the search field, search Bălți".

Also replace the fallback "Offline reload fails: load the app twice online
before going offline so the service worker and OPFS are warm" with: "An
offline reload shows the login screen because `/api/v1/auth/me` is not cached;
do not reload - use the no-reload sequence above."

### 6.2 The pack size: doc is right, the live UI is still wrong

The live manifest is **220,262,575 bytes** (about 220 MB). The current
`Demo-Script.md` expected-values row already says "Moldova installed in web
OPFS (~220 MB); native pack 138.5/204 MB", so that row needs no change; the
native pack was not tested here.

The live web UI still advertises the old size:

- `web/app/index.html` labels `Install Moldova (139 MB)` and
  `Download map (139 MB)` -> 220 MB.
- Any slide or README text that still says 139 MB / 204 MB for the **web**
  pack.

The web manifest includes all three files, including the 120,319,488-byte
`moldova-valhalla.tar`. The web client has no offline routing path, so the
web install downloads 120 MB that it does not use. Either drop the tar from
the web catalog (the map+search install would be about 100 MB) or keep it and
label the button 220 MB. The profile's storage estimate was
**225,798,620 bytes** (about 226 MB) including the shell cache, so
"~220 MB pack / ~226 MB browser storage" is the accurate web wording.

### 6.3 Keep "do not press Directions offline"

This is correct. The review did not exercise Directions offline. The offline
pack contains a Valhalla tar, but the web client has no offline routing path
wired to it.

### 6.4 Pre-flight: A must have the pack installed

The current one-time setup says "Choose a map in both profiles: **Use online
map**, or install **Moldova**". The 5:30 offline step assumes an installed
Moldova pack. If A chose "Use online map", there is no OPFS pack and the
offline step cannot work. Change the pre-flight to require Moldova in A
(or start from a profile with the pack already installed); the online map is
fine for B or for the online segment only.

---

## 7. Bottom line

- **Offline map data and offline place search: PASS.** OPFS + service-worker
  Range serving work with zero network fetches; "Balti" returns eight local
  results in 55-80 ms, including through the app's own `getOfflineSearch`
  wrapper.
- **Offline demo as scripted with a reload: FAIL.** A real network cut makes
  the app show the login panel, so the map and search box are not reachable.
  The DevTools-only page emulation that hides this must not be used as proof.
- **Corrected no-reload demo: PASS.** With the warm signed-in page left open
  and the page/service-worker targets set offline, the app stayed signed in,
  and the real search box returned eight Bălți rows. Use this sequence and clear
  the search field first. Fixing the offline boot path would be the only way
  to keep the reload version.
