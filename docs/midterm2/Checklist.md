# Pinpoint - Midterm 2 Submission and Defence Checklist

**System:** v2.6.1 - https://pinpoint-uj4y.onrender.com  
**Print:** A4 portrait, 1-2 pages. Tick each box; every line names the file or URL
that holds the evidence or the step.

---

## 1. Documentation to submit

- [ ] Report: `docs/midterm2/Midterm2-Report.md` (problem, scope, architecture,
      consent/security, data, verification, limitations, traceability).
- [ ] Report PDF if the course wants one:
      `docs/midterm2/Midterm2-Report.pdf` (regenerate from the report).
- [ ] Deck PDF backup: `docs/midterm2/Pinpoint-Midterm2.pdf`
      (regenerate by opening `docs/midterm2/slides.html` and pressing `P`).
- [ ] Demo script: `docs/midterm2/Demo-Script.md`.
- [ ] Presentation outline with speaker notes:
      `docs/midterm2/Presentation.md`.
- [ ] Printable handout: `docs/midterm2/Handout.md`.
- [ ] Q&A preparation: `docs/midterm2/Q-and-A.md`.
- [ ] Admin runbook (only if DB access is available):
      `docs/midterm2/Admin-Demo.md`.
- [ ] Package index and live links: `docs/midterm2/README.md`.
- [ ] Changelog entry under `[Unreleased]`: `CHANGELOG.md`.
- [ ] Version consistent everywhere: `Cargo.toml` line 3, report header,
      `/health`, release tag all say `2.6.1`.

## 2. Presentation

- [ ] Deck opens offline: `docs/midterm2/slides.html` (arrows/PageDown
      navigate, `P` prints A4 landscape).
- [ ] Printed or PDF backup of the deck, in case the projector or laptop fails.
- [ ] Timing rehearsed: 13 slides, ~8 min
      (`docs/midterm2/Presentation.md`, per-slide seconds).
- [ ] 3-minute cut rehearsed if the slot is short:
      `docs/midterm2/Demo-Script.md` section 5.
- [ ] Q&A answers read once: `docs/midterm2/Presentation.md`, "Q&A preparation"
      table at the end.
- [ ] Known limits match the report: slide 12 vs
      `docs/midterm2/Midterm2-Report.md` section 9.

## 3. Live system

- [ ] Service warm and correct version:
      https://pinpoint-uj4y.onrender.com/health shows
      `"version":"2.6.1"` and `"database":"connected"`.
- [ ] App loads and shows the login page:
      https://pinpoint-uj4y.onrender.com (`assets/01-login.png` is the expected
      look).
- [ ] Release page has the APK:
      https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1.
- [ ] Local gate green if asked to prove it:
      `docs/midterm2/Midterm2-Report.md` Appendix B
      (`cargo fmt`, `clippy`, `build --locked`, `test --locked`).

## 4. Demo run of show

- [ ] Two isolated browser sessions prepared:
      `docs/midterm2/Demo-Script.md` section 1, step 2 (Chrome profile 1 and
      Edge or Chrome guest).
- [ ] Map, search, route warm: section 2, `0:30`; expect **136.4 km / 2 h 04**.
- [ ] Consent story: section 2, `2:00` - B shares exact/1 hour, A sees exact,
      B pauses, A reloads and sees nothing, B resumes, B revokes.
- [ ] Today trail and private photo note: section 2, `4:00`.
- [ ] Offline map and local search, no reload: section 2, `5:30`.
- [ ] Phone and close: section 2, `6:30`.
- [ ] Expected values memorised: section 3 (route, `47.0245, 28.8322`,
      `47.02, 28.83`, `47.0, 28.8`, 2-point trail, Moldova pack installed in the web OPFS).

## 5. Accounts

- [ ] Account A (presenter) and account B (friend) registered with your own
      real emails through the public flow: `Demo-Script.md` section 1, step 3.
- [ ] Friendship accepted before the talk: A sends by handle, B reloads and
      accepts, A reloads (section 1, step 4). The app does not poll.
- [ ] Handles written down where you can read them (for example
      `calm-otter-42`).
- [ ] No seeded users, demo pins or shared passwords exist in the repo:
      `docs/midterm2/README.md`, "Before presenting".
- [ ] Never show the Admin Users table; it holds real emails
      (`docs/midterm2/README.md`, "Admin note").

## 6. Phone

- [ ] APK 2.6.1 installed from the release page above; signed ARM build
      (`Midterm2-Report.md` section 6.4).
- [ ] Signed in as B and the Today tab opened once before the talk
      (`Demo-Script.md` section 1, step 8).
- [ ] One tap tested on the actual device; if the phone misbehaves, fall back
      to the web demo (`Demo-Script.md` section 4, "Phone widgets").

## 7. Offline fallback

- [ ] Moldova pack installed and "Moldova is installed" seen
      (`Demo-Script.md` section 1, step 5; `assets/07-offline.png`).
- [ ] App loaded twice online first, so the service worker and OPFS are warm
      (`Demo-Script.md` section 4).
- [ ] DevTools > Network > Offline works on this machine; the loaded map stays
      and search returns **Bălți** locally (do not reload).
- [ ] Do **not** press Directions while offline; web routing is not offline
      (`Demo-Script.md` section 2, `5:30`).
- [ ] Fallback list skimmed: blank map prompt, stale UI (Ctrl+Shift+R,
      cache `pinpoint-offline-v3`), route failure keeps the warm panel
      (`Demo-Script.md` section 4).
- [ ] If live fails twice, switch to `docs/midterm2/assets/` and keep talking.

## 8. Admin caveat

- [ ] Admin tab is hidden by default and there is no seeded admin
      (`Midterm2-Report.md` section 9, first bullet).
- [ ] Only demo Admin if the account was promoted with `DATABASE_URL` and
      `pinpoint-admin --promote-email <email>` (`Demo-Script.md` section 6).
- [ ] If promoted: sign out/in, show **Overview** and **Audit** only.
- [ ] Say the limit out loud: no HTTP bootstrap, needs database access.
- [ ] Do not claim what is not built: `Demo-Script.md` section 7 (no background
      tracking, no friend-visible notes, no native photo picker, no offline web
      directions).

## 9. Evidence map (answer any "prove it")

| Claim | Where it lives |
|---|---|
| 328 unit tests, 24 test binaries; consent/today/contract/assets/router suites | `Midterm2-Report.md` section 6.1 |
| Live API run 14/14 on v2.6.1 (24/24 on v2.5.2), manual and unarchived | `Midterm2-Report.md` section 6.2 |
| Browser 9/9, offline 7/7, sharing UI 17/17 (first two manual/unarchived) | `Midterm2-Report.md` section 6.3 |
| APK size, versionCode, signing SHA-256, `/health` | `Midterm2-Report.md` section 6.4 |
| Screenshots of every screen | `docs/midterm2/assets/` |
| Requirement -> test traceability | `Midterm2-Report.md` section 8 |
| CI status | https://github.com/Prince-of-Nothing/Pinpoint/actions |

## 10. Pre-flight T-15 minutes

| T | Do | Where |
|---|---|---|
| -15 | Open `/health`, confirm `2.6.1` + postgres | https://pinpoint-uj4y.onrender.com/health |
| -13 | Open Window A (profile 1) and Window B (Edge/guest); sign in A, sign in B | `Demo-Script.md` section 1, step 2 |
| -10 | Friend request + accept; both reload | `Demo-Script.md` section 1, step 4 |
| -8 | Choose map in both; install Moldova if needed | `Demo-Script.md` section 1, step 5 |
| -6 | Publish two points, open Today, add one photo note in A | `Demo-Script.md` section 1, step 6 |
| -4 | Run the route once (136.4 km / 2 h 04) | `Demo-Script.md` section 1, step 7 |
| -3 | Phone on charger, APK open on Today as B | `Demo-Script.md` section 1, step 8 |
| -2 | Deck open at slide 1; PDF backup reachable; `slides.html` zoom reset | `docs/midterm2/slides.html` |
| -1 | Close personal tabs, silence notifications, clear projector mirror; keep `assets/` open in a folder | `docs/midterm2/assets/` |
| 0 | Sign in as A, start with the consent sentence | `Demo-Script.md` section 2, `0:00` |

---

**If something breaks:** say what should happen, show the matching screenshot
from `docs/midterm2/assets/`, and continue. Do not debug live for more than
30 seconds.
