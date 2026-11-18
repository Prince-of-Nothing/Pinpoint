# Wiki alignment review - Midterm 2

**Checked:** 18 November 2026, branch `docs/midterm2`, package v2.6.1.
**Baseline:** local wiki clone `pinpoint-wiki` (four Task 1 pages plus the SDLC
pages they link; wiki source pinned at commit `2f39d4f`).
**Verdict:** the Midterm 2 package is stronger than the prior Task 1 package on
delivered evidence and live deployment, but it drops three structural elements
the prior assessment used: an architecture segment, a live security segment and
requirement-ID traceability. The wiki also still describes the 4 November Rust
scaffold and the FastAPI/Python MVP, so it now contradicts the report. None of
this blocks the submission; the P0 items below are the ones to fix before the
defence.

**Scope:** the four named Task 1 pages plus the SDLC pages they link; this is
not a full wiki audit. No wiki file was edited, and the wiki snapshot is pinned
at `2f39d4f`, so recheck the P0 rows if the wiki has moved.

## 1. Prior structure vs Midterm 2

| Prior element | Where it lived | Midterm 2 status | Gap |
|---|---|---|---|
| Dated progress since the last milestone | Task 1 November, "Progress since September" | Missing | Report starts at v2.6.1; no table showing September MVP -> Rust transition -> v2.6.1 |
| Product at a glance with current limitations | Task 1 November, "Product and system at a glance" | Partial | Scope table omits freshness and check-ins/emergencies; statuses are not consistently Live/Partial/Deferred |
| Security risks with priority and action | Task 1 November, "Security risks and priorities" | Partial | Section 9 is a bullet list without impact, closure action or horizon |
| Plan with windows and exit evidence | Task 1 November, "Plan for the coming months" | Partial | Section 10 has 8 undated next steps; no exit evidence per item |
| Gates before the production path | Task 1 November, gates section | Missing | Only the Q&A answer says "not production-ready" |
| References to ADRs / SDLC / tests | Task 1 November, References | Missing | Report links only live app, source and release |
| Requirement -> business goal -> design -> implementation -> evidence | SDLC-15 matrix | Degraded | Report section 8 uses prose requirements, no FR IDs, no BG column, no status, no deferred list |
| Architecture demonstration | MVP Demo Script section 1 | Missing | Demo starts on the map; no architecture beat |
| Live security demonstration | MVP Demo Script section 3 | Missing | No negative checks, no cookie/CSP/audit walkthrough, no test evidence |
| Sessions and check-ins in the run of show | MVP Demo Script section 2 | Missing | Report claims them; demo shows neither |
| Evidence walkthrough | MVP Demo Script section 3 | Partial | Demo closes on `/health`; no test/CI/release page |
| Limitations and next steps close | MVP Demo Script section 4 | Partial | "What not to claim" exists; no plan close |
| Continuous speech with word counts | MVP Presentation Speech | Partial | Presentation.md has per-slide notes, not a word-counted transcript |
| Recorded demo fallback | MVP Presentation Speech | Missing | Screenshots are the fallback; no video. Optional unless an async submission is required |
| Realistic timing | MVP Demo Script, 15 minutes | Fixed | Header says ~8 minutes; current per-slide timings sum to 7:55 including the 110 s demo. Recheck after any slide edit |

## 2. Exact additions

### 2.1 `Midterm2-Report.md`

1. **Header.** Replace `[Your name]` with the author. Add the submission date
   next to the version (the date is already there; keep it).
2. **Section 2, progress table.** Add a five-row table: 29-30 September MVP;
   October Rust transition; 13-14 November navigation/native work; 18 November
   v2.6.1 release; next milestone. Use release tags/PRs as evidence, not
   adjectives.
3. **Section 2, scope table.** Add two rows and make the status column honest:
   - `Safety`: check-ins and emergency alerts, owner-scoped; API plus the
     native "Trips & check-ins" screen; the main web app has no check-in tab (the `web/webk` kiosk is a prototype, not part of this demo).
   - `Freshness`: native map badge (`live`/`fresh`/`stale`/`dead`, from
     `src/models.rs::freshness_state`); the web friend-locations list shows
     `recorded_at` as local time but no badge.
   Mark partial items as `Partial`, not `Live`: web freshness badge, native
   photos, offline web routing, MFA enrolment, admin bootstrap, background
   tracking.
4. **Section 2 or 3, decisions.** Add a short table of scope decisions with
   ADRs: ADR-0006 (transport/session), ADR-0007 (Rust multiplatform),
   ADR-0012 (opaque sessions), ADR-0019 (account deletion), ADR-0020/0021
   (offline maps/catalog). One line each, matching the prior "scope decisions
   and why" element.
5. **Section 6.1.** Add the gate date and the `cargo test` summary line (for
   example "328 passed; 0 failed") or a CI run URL. "Latest result: pass" is not
   reproducible evidence.
6. **Section 6.2.** Resolve the 14/14 contradiction. Either fix the harness's
   expected version (`2.6.0` -> `2.6.1`) and rerun for a clean 14/14, or write:
   "14 functional checks passed; one harness assertion failed because it still
   expected 2.6.0 while `/health` correctly returned 2.6.1." Also name the
   script/path used for the live run; if it is local-only, say so.
7. **Section 6.4.** Add the CI run URL and date. Name the jobs that passed and
   keep the macOS flake outside the claim.
8. **New Appendix C (or a short subsection after Section 6), deployment.** Add a five-line topology paragraph: Render web
   service, managed PostgreSQL, multi-replica convergence refresh, no seeded
   accounts, admin promotion via `pinpoint-admin --promote-email` plus
   `DATABASE_URL`. The prior progress page had a deployment/status section; the
   report currently has none.
9. **Section 8, traceability.** Replace the prose table with:
   `| FR | Requirement (short) | Status | Evidence |`, where status is
   `Live / Partial / Deferred / Not in v2.6.1`. At minimum cover FR-01, FR-02,
   FR-03/04, FR-05, FR-06-09, FR-13/14, FR-23-27, FR-29, FR-30, FR-34 and the
   new v2.6.1 rows (private trail, private notes, account deletion, diagnostics/
   admin gate, offline web install, signed APK).
   Add a "Deferred / not evidenced in v2.6.1" list: friend-visible trails and
   corridors, background tracking, push, OAuth login wiring, passkeys/magic
   links, email password reset, web MFA enrolment, native photos, offline web
   routing, admin HTTP bootstrap, E2E encryption (FR-28).
10. **Section 9, limits.** Convert to
    `| Limit | Impact | Closure / next step | When |`. Add the web freshness
    badge and main-web check-in gaps; keep the macOS flake but mark it "not on the
    demo path".
11. **Section 10, next steps.** Keep the eight items but group them into three
    windows with exit evidence, for example:
    - Next 4 weeks: admin bootstrap, web MFA enrolment, email recovery. Exit:
      a promoted account without SQL, enrolment test, reset token test.
    - Next 8 weeks: offline web routing, native photo picker, shared-note
      consent. Exit: offline route check on the web, photo upload on the APK.
    - Next 12 weeks: replace the per-request convergence refresh, load test,
      deterministic CI. Exit: load report and green matrix.
12. **Definition of done.** Add five bullets: gate green; live functional
    checks green; APK signature verified; CI green; no claim outside section 9.
    This is the prior "gates before production" element in smaller form.
13. **References.** Add ADR-0006/0007/0012/0019/0020/0021, SDLC-15,
    `tests/contract.rs`, `tests/location_consent.rs`, `tests/today.rs`,
    `.github/workflows`, live `/health`.
14. **Footer.** Align the screenshot versions: README presents the assets as current; the report footer says the native screenshot is the v2.6.0 theme
    build. State which is which or recapture.

### 2.2 `Demo-Script.md`

1. **Architecture beat (extend the opening to 60 seconds).** Show
   `slides.html` slides 4-5 or the report section 3 diagram, then `/health`:
   "One Rust API, PostgreSQL authoritative, three clients, native map server.
   Every request passes the same server checks; the client never decides
   access." Do not run Docker; the live system is Render.
2. **Optional security proof (60-90 seconds, after the consent story or before
   the close).** The prior 15-minute script spent five minutes on this; the
   Midterm 2 slot is shorter, so use a compact table:
   | Claim | Do | Expected |
   |---|---|---|
   | Friendship is not consent | Before B shares, A reloads; friend drawer is empty | empty list |
   | Fail closed | B pauses/revokes; A reloads | B absent |
   | CSRF | A DevTools: `fetch('/api/v1/locations/me',{method:'POST',credentials:'same-origin',headers:{'Content-Type':'application/json'},body:'{}'})` | `403 CSRF_FAILED` |
   | Session revocation | Account -> active sessions -> Revoke / Sign out everywhere | revoked browser's next request `401` |
   | Cookie / CSP | DevTools -> Application -> Cookies and Network -> response headers | `session_id` is HttpOnly + SameSite; `Content-Security-Policy` present; no third-party scripts |
   | Owner-only write | `cargo test --test location_consent` locally, or a two-account live check | `400 SHARE_NOT_FOUND` (the API uses 400, not 404) |
   The owner-only row can be shown as test evidence rather than a live attack.
   Audit is only demoable after admin promotion; otherwise point at `Admin-Demo.md`
   and the append-only audit test. If the
   assessment explicitly requires live attacks, port the existing two-account
   checks into the demo script; do not hand-wave "the tests cover it".
3. **Sessions step (15 seconds, core or optional).** Account -> active
   sessions -> show the list -> Revoke one or Sign out everywhere. The report
   claims this; the prior demo showed it; the current script does not.
4. **Check-ins step (15 seconds, phone, optional).** On the native client open
   "Trips & check-ins", create one, and say it is owner-only. If the phone step
   is already tight, add one sentence to "what not to claim": "Check-ins are
   API plus native Trips; the main web app has no check-in tab."
5. **Close (30 seconds).** End on report section 9/10, the CI page or the
   release page, not only `/health`. Say the next step in one sentence.
6. **Pre-flight.** Add: open the deck PDF backup; decide whether the security
   proof is in or out; verify `/health`; confirm the report in the folder is
   the current PDF.
7. **Expected values.** Add the `403 CSRF_FAILED` and `400 SHARE_NOT_FOUND`
   rows if the security proof is kept.
8. **Fallbacks.** Add "if asked for architecture, show slides 4-5; if asked to
   prove security, show the table above and the named test files."

### 2.3 `Presentation.md` and `slides.html`

1. **Timing.** The header (~8 minutes) now matches the per-slide timings: they sum to 7:55 including the 110 s demo. Keep the report's "8-minute run" consistent and recheck after any slide edit.
2. **Slide 4.** Add check-ins/emergencies and freshness (with the native-only
   qualifier), or remove the implication that they are part of the web demo.
   Keep "no continuous background tracking" explicit.
3. **Slide 6.** Add a "proved by" footnote mapping each claim to a test:
   sessions/CSRF -> `tests/contract.rs`; consent/precision -> 
   `tests/location_consent.rs`; trail/notes -> `tests/today.rs`; admin gate ->
   `tests/router_probe.rs`. This is the control-to-evidence element the prior
   speech had (SC-01..SC-06).
4. **Slide 10.** The 14/14 caveat is now in the notes; keep it. Add the CI URL and "matrix: report section 8", and label the earlier 24/24 run as v2.5.2.
5. **Slide 12.** Add one sentence: "Not production-ready: admin bootstrap,
   recovery, offline routing, native photos, load testing and CI determinism
   remain." The Q&A says it; the slide does not.
6. **Q&A.** `Presentation.md` now links the 20-question `Q-and-A.md`; add the four missing answers there (traceability matrix, freshness, check-in tab, no live attack demo). The current bank does not answer the matrix or freshness question.
7. **Visuals.** Add asset filenames to each slide's "Visual" line where an
   asset exists (slides 7, 8, 9 already have them; the others describe boxes).
   This is optional but makes the deck rebuildable.
8. **Speech.** Do not duplicate `Presentation.md` into a word-counted
   `Speech.md` unless the course requires a transcript. The prior speech page
   was a separate artifact; the notes here are already written to be spoken.

### 2.4 `README.md`

1. **Package table.** Add the files that are now in the folder but not listed:
   `Checklist.md`, `Handout.md`, `Admin-Demo.md`, `Q-and-A.md`, `Midterm2-Report.pdf`, and `reviews/` (mark reviews as internal, not submitted). Distinguish the two
   PDFs: `Pinpoint-Midterm2.pdf` is the deck export; `Midterm2-Report.pdf` is
   the report export.
2. **Submission order.** Add one line: report PDF, deck PDF, demo script,
   checklist. The checklist already lists these; the README should match.
3. **Screenshot rebuild.** The README points at temp scripts on the author's
   machine. Either commit them under `docs/midterm2/tools/` or state
   "screenshots are archival, captured 18 November; the capture scripts are not
   committed." The prior task's evidence was reproducible; an assessor cannot
   rerun a temp path.
4. **Version note.** Say which asset is v2.6.0 versus v2.6.1, matching the
   report footer.
5. **Before presenting.** Add: open the deck PDF backup; decide whether the
   security proof is in the run; read report section 9; check the admin
   caveat.
6. **Claim hygiene.** "No seeded users, demo pins or shared passwords in the
   repository" is too broad: `tests/live_api.rs` still documents the old Go API
   and `alice@example.com / DemoPass123!`. Either update that ignored test or
   narrow the README sentence to "not used by the live demo; test fixtures are
   local only."

### 2.5 `Checklist.md`, `Handout.md`, `Admin-Demo.md` (if submitted)

- Checklist section 1 should list `Handout.md`, `Admin-Demo.md` and `Q-and-A.md` if they are
  part of the package; section 9 should point to the rewritten report section 8.
- Handout repeats "14/14" without the harness caveat; keep it consistent with
  the report. Add the freshness/check-in qualifiers to its "Known limits".
- Admin-Demo is good and should be linked from the demo script's admin section;
  it already gives the exact `403 FORBIDDEN` role-gate check.
- Both `Checklist.md` and `Handout.md` were untracked when this review was written; commit them
  with the package or remove them from the submission list.

## 3. Wiki pages to update (do not edit here)

### P0 - the wiki currently contradicts v2.6.1

| Page | What is wrong | Exact update |
|---|---|---|
| `Task1-November-Project-Progress-And-Plan.md` | Says Rust does not compile (39 errors), has no Rust tests and no Rust CI job; the bottom line calls it a non-compiling scaffold. v2.6.1 is a live Rust deployment with 328 tests and a signed APK. | Add a dated banner: "Historical snapshot as of 4 November 2026; current status is v2.6.1, see `docs/midterm2/README.md`." Keep the 4 November evidence, but correct the bottom line and the "Plan for the coming months" status. |
| `SDLC-15-Traceability-Matrix.md` | FR-24/25/26 still say handlers/tests pending; FR-13/14 point at Kotlin MapLibre; FR-30 says Kotlin + Rust dual apps; the deferred list says all movement trails are out of scope; test evidence is stale. | Reconcile FR-25/26 to `tests/contract.rs` + `tests/router_probe.rs::session_list_and_revoke_flow` + `tests/account_delete.rs`; mark FR-24 design/schema-only and FR-27 login-only (no web enrolment); FR-13/14 to `src/offline_install.rs`/`src/tilepacks.rs` + web OPFS + `pinpoint-maps`; FR-30 to one Rust APK + native map server. Add rows for the private trail (`/locations/history`, `tests/today.rs`), private notes (`pinpoint_place_notes`, `tests/today.rs`), account deletion (`tests/account_delete.rs`), diagnostics/admin gate (`tests/router_probe.rs`), offline web (7/7) and the signed APK. Replace "movement trails" in the deferred list with "friend-visible trails/corridors and background tracking"; state that the owner-only 24-hour trail is delivered. Add a "v2.6.1 snapshot; report section 8 mirrors this" note. |
| `How-To-Mvp-Demo-Script.md` | 15-minute FastAPI MVP with `demo_parent`/`demo_child`, `demo_security.sh`, 140 Python tests and Flutter next steps. It is not the current demo. | Add a top banner: "Historical MVP script; current script: `docs/midterm2/Demo-Script.md`." Keep the security section as the model for the missing Midterm 2 security beat. |
| `How-To-Mvp-Presentation-Speech.md` | 11 slides, 158 unit + 17 integration tests, JWT refresh tokens, Flutter/October roadmap. It is not the current talk. | Add a top banner: "Historical MVP speech; current deck and notes: `docs/midterm2/Presentation.md`." Do not copy the old numbers into the Midterm 2 deck. |

### P1 - scope and evidence drift

| Page | What is wrong | Exact update |
|---|---|---|
| `SDLC-03-Functional-Requirements.md` | Non-goals explicitly list "Trails"; FR-13/14 describe Kotlin MapLibre bbox packs, not country packs + OPFS; no FR for notes or the private trail. | Change "Trails" to "continuous/background trails, friend-visible trails, heatmaps"; add FR-35 (private 24-hour owner-only trail, 7-day retention) and FR-36 (private place notes, JPEG/PNG, size bound, owner-only); update FR-13/14 to the delivered web OPFS + native map server story. |
| `SDLC-01-Business-Requirements.md` | Out-of-scope lists movement trails; success criteria still say FR-01..FR-22 on web + Android and `docker compose up`. | Remove the private trail from out-of-scope (keep continuous/background and friend-visible trails); update success criteria to the current FR set (including the new trail/notes rows) and the live Render deployment. |
| `SDLC-02-Scope-And-Assumptions.md` | Out of scope includes live trails and check-ins; dependencies still name the Go API and Python alt API; offline scope says shell only. | Keep background location, friend-visible trails and push out of scope; move private trail + check-ins into scope; replace Go/Python dependencies with the Rust API and archived stacks; update offline to OPFS map/search + native packs. |
| `SDLC-04-Non-Functional-Requirements.md` | NFR-04 assumes Compose; NFR-08 says web is shell-only offline; NFR-10 says zero-dependency vanilla JS; NFR-13 says Go + Python backends; NFR-14 says latest-only. | Update NFR-04 to the Render service and local Compose path; NFR-08 to OPFS pack + offline search; NFR-10 to vendored MapLibre/PMTiles; NFR-13 to the Rust API; NFR-14 to latest location + 24 h visible/7 d retained owner-only trail. |
| `SDLC-10-Security-And-Privacy.md` | Says "no location history server-side" and "friend locations latest-only"; the data table has no trail/notes retention. | Update the controls and data table: owner-only trail (24 h visible, 7 d retained), private notes, and the same "no friend data to OSM services" rule. Clarify that the service worker still must not cache friend locations; offline packs are map data. |
| `SDLC-11-QA-And-Verification.md` | Unit layer is Go/Python; no Rust gate, live/browser/offline run or APK check. | Replace with the Rust gate (`fmt`, `clippy`, `build/test --locked`), 328 library tests + 24 binaries, live 14/14, UI 9/9, offline 7/7, APK signature check, macOS flake. |
| `SDLC-12-Deployment-And-Operations.md` | Environments are Go/Python/Compose with seeded demo credentials; no Render or APK. | Add the v2.6.1 live environment (Render + managed Postgres, `/health`, multi-replica, no seeds, admin promotion), the release/APK artifact and the current CI jobs. Mark the Go/Python sections historical. |
| `Evidence-Index.md`, `Evidence-Security-Test-Report.md`, `Evidence-Web-Client-E2e.md` | 140 Python tests/97% coverage and the old web client; not the v2.6.1 evidence. | Add a banner "historical September MVP evidence" and link report section 6, or regenerate the evidence for the Rust suites. |
| `How-To-Implementation-Plan.md` | Task board and evidence still reference `backend/app`, Go stores and the September MVP; Task 9 is "Prepared". | Add a v2.6.1 status column or a top banner pointing to `docs/midterm2/README.md`; mark the old task evidence historical. |
| `How-To-Development-Plan-October-December.md` | Promises Flutter, Celery/FCM, JWT/EdDSA and a January/February plan that no longer matches the Rust direction. | Mark historical and link report section 10; do not present it as the current plan. |

### P2 - consistency and navigation

| Page | What to do |
|---|---|
| `Architecture-Building-Block-View.md`, `Architecture-Runtime-View.md`, `Architecture-Deployment-View.md` | Either update the FastAPI/Go/Python diagrams to the Rust API + web/native/native map server, or label them "as built September MVP" and link report section 3. |
| `Architecture-Movement-Trails-And-Search-Corridors.md`, `Architecture-Privacy-And-Data-Handling.md` | Mark the owner-only 24 h trail as delivered; keep friend-visible trails, corridors and mosaics deferred. Update retention text. |
| `Database-Schema.md` | No `pinpoint_location_history` or `pinpoint_place_notes`; update to migration 16 or label the page a target design. |
| `Home.md`, `_Sidebar.md` | Add a "Midterm 2" link (wiki landing page or `docs/midterm2/README.md`) so the wiki does not present Task 1 as the current package. |
| `Release-Plan.md`, `Roadmap.md`, `Planned-Tasks.md` | Check for Flutter/background/push/JWT promises; align with report section 10 or label historical. |

## 4. Smallest complete set before the defence

1. Report: author, progress table, scope rows for freshness/check-ins, section 8
   keyed to FR IDs, 14/14 wording, deployment paragraph, risk/next-step tables.
2. Demo: architecture beat, optional security proof, sessions step, close on
   evidence.
3. Deck: control-to-test footnote, honest limits, Q&A rows.
4. README: list every file in the folder, distinguish the two PDFs, narrow the
   "no credentials in the repo" sentence.
5. Wiki: add the P0 banners/updates above; the rest can follow the thesis
   refresh.

## 5. Do not add

- Do not duplicate the whole SDLC-15 matrix in the report; one status table plus
  the SDLC link is enough.
- Do not add a new security script unless the slot has time. The evidence table
  plus one live CSRF check restores the prior structure without another
  artifact; port the two-account checks only if the assessment explicitly
  requires live attacks.
- Do not add a separate speech transcript unless the course requires one.
- Do not claim a main-web freshness badge or a main-web check-in tab: the main
  app has neither. Say "native badge" or "native Trips" instead.