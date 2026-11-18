# Midterm 2 package

Everything needed for the Midterm 2 documentation, demo and presentation, built
from the live **v2.6.1** system.

| File | What it is | Use |
|---|---|---|
| [`Midterm2-Report.md`](./Midterm2-Report.md) | Full report: problem, scope, architecture, consent/security, data, verification, limitations, next steps, traceability | Submit as the written documentation |
| [`Midterm2-Report.pdf`](./Midterm2-Report.pdf) | 9-page A4 PDF of the report | Submit or print |
| [`Demo-Script.md`](./Demo-Script.md) | Pre-flight, exact run of show, expected values, fallbacks, 3-minute cut, phone steps | Rehearse and run the live demo |
| [`Presentation.md`](./Presentation.md) | 13-slide outline with speaker notes, timing and Q&A preparation | Rehearse the talk |
| [`slides.html`](./slides.html) | Self-contained visual deck (dark Pinpoint theme, keyboard navigation, print to PDF) | Present or export to PDF |
| [`Pinpoint-Midterm2.pdf`](./Pinpoint-Midterm2.pdf) | 13-slide A4 landscape deck PDF | Present or keep as backup |
| [`Handout.md`](./Handout.md) | Two-page printable summary of scope, security, evidence and limits | Give to the examiner |
| [`Checklist.md`](./Checklist.md) | Pre-flight, accounts, phone, offline and evidence checklist | Tick before the defence |
| [`Q-and-A.md`](./Q-and-A.md) | 20 likely questions with verified/reported/not-built answers | Prepare for questions |
| [`Admin-Demo.md`](./Admin-Demo.md) | Admin promotion and verification runbook | Use only with the deployment DB |
| [`reviews/`](./reviews) | Deck QA, artifact, live security/a11y and fact-check evidence | Supporting evidence, not submitted prose |
| [`assets/`](./assets) | Screenshots: login, map, Today, notes, offline pack, native offline route; `05-mobile.png`/`06-native.png` are archive shots from before the Today tab | Slides and report references |

## Quick start

Open the deck in a browser:

```powershell
start docs\midterm2\slides.html
```

- `→` / `←` / `PageDown` / `PageUp`: navigate
- `Home` / `End`: first / last slide
- `P` or **Print**: print to PDF (A4 landscape)
- The deck uses only local assets; no network is required.

## What is live

- App: https://pinpoint-uj4y.onrender.com (v2.6.1)
- Health: https://pinpoint-uj4y.onrender.com/health
- Release / APK: https://github.com/Prince-of-Nothing/Pinpoint/releases/tag/v2.6.1
- Source: https://github.com/Prince-of-Nothing/Pinpoint

## Before presenting

1. Warm the Render service and confirm `"version":"2.6.1"`.
2. Register two email accounts; pre-accept the friendship.
3. Choose a map and publish two points + one photo note in account A.
4. Run the route once (Chișinău → Bălți; expect 136.4 km / 2 h 04).
5. Install the 2.6.1 APK on the phone and open the Today tab once.
6. Read the **Known limits** slide and the demo fallbacks.

The demo uses real accounts registered on the live system. There are no seeded
users, demo pins or shared passwords in the repository.

## Rebuild the screenshots (optional)

The screenshots in `assets/` were captured from the live web app and the native
map harness on 18 November 2026. The UI checks live in
`C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\today-ui-check.mjs` and
`today-ui-live.mjs` on the author's machine; they are not committed because
they use a temporary Chrome profile and test accounts. The 9/9 and 7/7 numbers
are therefore reported manual runs, not reproducible from this repository.

## Admin note

The Admin tab is role-gated and there is no seeded admin. To demo it, promote an
already-registered account with the Render `DATABASE_URL`:

```powershell
$env:DATABASE_URL = "postgres://..."
cargo run --bin pinpoint-admin -- --promote-email owner@example.com
```

Do not project the Users table: it contains real email addresses.
