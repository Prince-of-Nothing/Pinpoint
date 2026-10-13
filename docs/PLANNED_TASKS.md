# Planned Tasks (Backlog beyond the 8-week board)

This is the grooming list: everything we *plan* to do but that is **not** yet committed to a weekly slot in [TASKS.md](TASKS.md). During each Friday sync we pull items into the board (assigning member + week + dependencies) or explicitly defer them with a reason.

Priority: 🔥 critical · ⭐ high · ➕ medium · 💤 low/nice-to-have

## Backend / Infrastructure
| ID | Priority | Title | Notes / likely dependencies |
|----|----------|-------|------------------------------|
| P-B01 | ⭐ | Multi-node horizontal scaling validation | After T34; sticky WS sessions or pub/sub cross-node |
| P-B02 | ➕ | Pub/Sub fan-out (Redis/NATS) for WS across instances | Depends on P-B01 findings |
| P-B03 | ⭐ | Rate-limit tiers per plan (free/family/business) | Needs billing decision |
| P-B04 | ➕ | gRPC internal API between services if split occurs | Only if monolith pain proven |
| P-B05 | 🔥 | Automated vulnerability patching (Dependabot/Renovate policies) | Extend T26/T39 tooling |
| P-B06 | ➕ | Observability upgrade: Prometheus metrics + alerting | Build on T15 structured logs |
| P-B07 | 💤 | Config-as-code hardening (no env typos: schema validation) | Small, good first issue |
| P-B08 | ⭐ | Key management: move JWT secret & SMTP creds to vault/secrets manager | Pre-prod requirement |

## Clients (web / weba / webk)
| ID | Priority | Title | Notes |
|----|----------|-------|-------|
| P-W01 | ⭐ | Progressive Web App for `web` (manifest, offline shell, push) | Reuses T17 push infra |
| P-W02 | ➕ | Accessibility certification pass (WCAG 2.2 AA audit external) | After T30/T31 |
| P-W03 | ➕ | Dark mode for `web` map tiles (CARTO dark basemap option) | Tile licensing check first |
| P-W04 | ⭐ | Shared component library for web/weba/webk (extract after T33 typed client lands) | Avoid duplication |
| P-W05 | 💤 | `webk` offline kiosk mode (cached last known position) | Edge case for family tablets |

## Mobile (Kotlin/Android)
| ID | Priority | Title | Notes |
|----|----------|-------|-------|
| P-M01 | ⭐ | Wear OS companion (glance at friend positions) | After T37 stable API |
| P-M02 | ➕ | Android Auto / CarPlay-style head-unit share status | Research battery impact |
| P-M03 | 🔥 | iOS port decision: KMP shared core vs Swift rewrite | Requires Apple hardware access; revisit quarterly |
| P-M04 | ➕ | Battery Saver integration (auto-downgrade tier T32 hook) | OEM-specific testing list |
| P-M05 | ⭐ | Widget: "who's nearby" home-screen glanceable | Room cache from T22 makes this cheap |
| P-M06 | 💤 | Localization expansion (ES, TR, AR + RTL) | Catalog exists from T30 |

## Product / Compliance / Ops
| ID | Priority | Title | Notes |
|----|----------|-------|-------|
| P-O01 | 🔥 | Formal DPIA sign-off with legal review | Extends T39 memo |
| P-O02 | ⭐ | Bug-bounty program setup (HackerOne or public safe-harbor policy) | Post-launch |
| P-O03 | ➕ | SOC2 Type I readiness assessment | If enterprise interest proves real |
| P-O04 | ⭐ | Pricing/monetization experiment plan | Blocks P-B03 |
| P-O05 | ➕ | Status page (self-hosted) + incident comms templates | On top of T34 monitoring |
| P-O06 | 💤 | Community translation platform (Weblate) | After P-M06 demand |

## Research spikes (timeboxed 1–2 days, feed backlog)
| ID | Title | Question to answer |
|----|-------|--------------------|
| R-01 | Tile hosting cost model | Self-host OSM tiles vs third-party at our DAU estimate |
| R-02 | Background location accuracy on OEM ROMs | Xiaomi/Samsung throttling behavior with WorkManager |
| R-03 | Consent UX experiments | Which wording maximizes correct understanding of directional shares |
| R-04 | CRDT sync feasibility | Could T22 evolve to peer-sync without server round-trips |

## Grooming rules
1. New ideas always land here first — never directly in TASKS.md.
2. To promote: copy the row, assign `Txx` ID, member slot, week, and `Requires` edges in TASKS.md.
3. Items deferred twice get a written reason and move to ROADMAP "Parking lot".
4. This file is reviewed every Friday; TASKS.md stamps are the source of truth for status.
