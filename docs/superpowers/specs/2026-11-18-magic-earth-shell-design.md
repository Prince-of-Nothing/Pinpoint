# Magic Earth shell - short design

**Date:** 2026-11-18
**Status:** approved direction; web first
**Scope:** `web/app` shell and visual language

## Intent

Pinpoint should feel like a map app, not a settings dashboard. The map is the
product; chrome is light, sparse and thumb-reachable; one sheet at a time
carries search, place, route, friends, sharing, Today and notes.

## Goals

- Light-first palette, with the current dark palette available as a theme.
- Full-bleed map with floating search and controls; no 60 px header band.
- One bottom sheet for every secondary surface, on desktop and mobile.
- Keep every DOM id, JS hook and account/consent behaviour.
- Match the Magic Earth feel: soft neutral surfaces, one warm accent, large
  touch targets, quiet motion.

## Non-goals

- No rewrite of map, search, route, sharing or auth logic.
- No new screens or features.
- No native theme change in this phase (port after the web shell passes).
- No branding copy of Magic Earth; only the interaction and visual grammar.

## Approach

Chosen: restyle the existing shell in place.

1. Replace the dark token block with light defaults and a dark override.
   Tokenise the hard-coded surface colours that would otherwise stay dark.
2. Turn the 60 px header into a floating glass bar over the map: wordmark left,
   search centred, session chip right.
3. Promote the mobile drawer rules to all widths: a bottom sheet with a drag
   handle, max-height 70vh, rounded top corners, slide transition.
4. Re-anchor search results, chips, nav banner and attribution to the floating
   bar and the sheet.
5. Add a light/dark toggle in Account; default to light.
6. Delete the duplicate colour override layer and the dead legacy map rules.

Rejected: a new component framework or a second stylesheet. The existing CSS
already has the pieces; a second layer would double the cascade.

## Constraints

- Demo flows from `docs/midterm2/Demo-Script.md` must still work unchanged:
  publish, trail, note, share, pause/resume/revoke, offline map and local search.
- `tests/assets.rs` pins same-origin assets and CSP; no new hosts or inline
  handlers.
- AA contrast for text and controls; 44 px touch targets; reduced-motion
  respected.
- No layout shift when the sheet opens; map keeps its centre and zoom.

## Risks

- The cascade has a late override layer (app.css:1321-1366) that can mask new
  rules; delete it in the same change.
- The global `header` selector also styles admin/report headers; scope the new
  rules to `body > header`.
- The sheet covers the first search result on small screens; keep the close
  button and the existing flow.

## Phases

1. Tokens + light theme + floating header + sheet geometry (CSS, small markup).
2. Behaviour pass: sheet open/close, theme toggle, focus/escape, reduced motion.
3. Visual QA at 1600x900 and 390x844; contrast and touch targets.
4. Native theme port in a separate change.
