# Presentation assets

Images used in the MVP presentation (Canva deck and demo script).

| File | What it shows | How it was produced |
|---|---|---|
| `app-*.png` | Real web-client panels with the demo data from `backend/scripts/seed_demo.py` | Playwright screenshots of the running app (2x) |
| `app-map.png` | Map markers, accuracy circle and popup | Same; map tiles were unavailable in the capture environment, so the map background is blank |
| `diagram-*.png` | Architecture, consent model, token lifecycle, security controls, demo terminal output | HTML rendered in the deck's style (DM Serif Display, Inter, indigo accent), transparent background |

The terminal image reproduces real output of `backend/scripts/demo_security.sh`.
The numbers in `diagram-controls.png` come from `docs/evidence/security-test-report.md`.
