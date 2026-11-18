# Pinpoint - Midterm 2 Presentation

**Deck:** 13 slides, ~8 minutes + Q&A.  
**Visual deck:** [`slides.html`](./slides.html) (open in a browser, press `P`
to print to PDF, or use the pre-printed `Pinpoint-Midterm2.pdf` if present).  
**Demo:** [`Demo-Script.md`](./Demo-Script.md).

The speaker notes below are written to be spoken, not read. Timings are
suggested; the live demo can replace slides 10-11 if the projector is reliable.

---

## Slide 1 - Title (20 s)

**Visual:** Pinpoint wordmark on the dark green/gold palette; subtitle
"Privacy-first location sharing for trusted friends"; v2.6.1; live URL.

**Say:**
> Good morning. I'm [name], and this is Pinpoint: location sharing for trusted
> friends, built around one rule - friendship is not consent.

## Slide 2 - The problem (45 s)

**Visual:** Three short columns or icons:
- "Friend = full access"
- "A stale dot looks live"
- "Control is buried"

**Say:**
> Location sharing is genuinely useful for safety: did they get home, is the
> kid on the way. The problem is how most products model it. Once you are
> friends, the other person can often see you all the time, until you remember
> to turn it off. The second problem is honesty: when a phone dies or loses
> signal, many apps keep showing the last dot as if it were live. In an
> emergency, an old dot that looks live is worse than no dot. And third,
> stopping someone from seeing you, or narrowing what they see, is often buried
> or incomplete.

## Slide 3 - The rule: consent is a share, not a friendship (60 s)

**Visual:** One-way arrow A -> B with labels "exact", "1 hour", "pause",
"revoke"; a crossed-out arrow B -> A labelled "not implied".

**Say:**
> Pinpoint makes sharing a decision, not a side effect. I choose a friend, a
> precision - exact, approximate or city only - and a duration. The share is
> one-directional: me sharing with you never means you share with me. I can
> pause, resume or revoke it. And the server checks all of that again on every
> read: is this share mine, is the friendship accepted, is it active, has it
> expired, and what precision did I choose?

## Slide 4 - What is delivered (60 s)

**Visual:** Six compact cards:
- Identity & sessions
- Friends & consent sharing
- Map / search / route
- Offline Moldova pack
- Today trail + private notes
- Web / desktop / Android

**Say:**
> Version 2.6.1 is live. It has email registration and opaque sessions; friend
> requests by handle; the consent share with exact, approximate and city-only
> precision, expiry, pause and revoke; map, search and driving directions;
> offline Moldova map and search; a private 24-hour trail; private text and
> photo notes; and three clients from one Rust codebase - web, desktop and
> Android. No demo accounts and no seeded pins: every account is registered
> through the public flow.

## Slide 5 - Architecture (60 s)

**Visual:** Mermaid-style boxes: Web / Native / native map server -> Rust Axum
API -> PostgreSQL; OpenFreeMap / Nominatim / OSRM and the pack catalog at the
edges.

**Say:**
> The API is Rust with Axum, and PostgreSQL is authoritative. The web client is
> same-origin vanilla ES modules with MapLibre and PMTiles. The desktop and
> Android clients are the same Rust/egui code; the native map server serves the
> offline packs. Every request goes through the same server-side checks:
> security headers, session, CSRF on unsafe methods, validation, authorization,
> then the response. The client is never trusted to enforce privacy.

## Slide 6 - Security and privacy (60 s)

**Visual:** A short checklist:
- Argon2id + opaque sessions
- HttpOnly cookie + CSRF double-submit
- Server-side authorization every read
- Precision applied on read
- Owner-only check-ins / emergencies
- No trackers, no third-party scripts

**Say:**
> Passwords are Argon2id. Sessions are opaque, server-side, and sent in an
> HttpOnly cookie; every unsafe request also carries a CSRF token bound to the
> session. Sharing is checked on the server: the owner, the accepted
> friendship, the active flag, the expiry and the precision. Check-ins and
> emergency alerts are owner-scoped. The web client has a strict CSP, no
> third-party scripts, and no trackers. Friend coordinates are never sent to
> the search or routing services.

## Slide 7 - Offline maps (45 s)

**Visual:** Screenshot `assets/07-offline.png` and `assets/08-route.png`.

**Say:**
> The offline pack is a real artifact, not a mock. Moldova is about 139
> megabytes to download and 204 megabytes installed; the web client stores it
> in OPFS and the service worker serves it with range requests. The map and
> search work with the network off. The native map server routes on the
> installed graph; online routing in the web client uses OSRM.

## Slide 8 - Today and private notes (45 s)

**Visual:** `assets/03-today.png` and `assets/04-notes.png`.

**Say:**
> Today is the owner's private trail: the points I publish in the last 24
> hours, newest first, drawn on the map. It is owner-only and retained for
> seven days on the server. Private notes attach a label, a message and an
> optional photo to a place. The photo is downscaled in the browser, bounded in
> size, validated on the server and visible only to me. This is the personal,
> on-device-feeling side of the map - not a social feed.

## Slide 9 - Native clients (40 s)

**Visual:** `assets/06-native.png` (desktop theme) and `assets/05-mobile.png`.

**Say:**
> The same Rust core ships a desktop client and an Android APK. The 2.6.1 APK
> is signed for arm64 and armeabi-v7a. The native client has the map-first
> theme, the Today tab with the trail and text notes, and the existing sharing
> flow. Photos are web-only in this build; the note shows that a photo is
> attached and points to the web client.

## Slide 10 - Verification (45 s)

**Visual:** A compact table:
- 328 library tests + 24 test binaries
- 10 consent tests, 2 Today tests, 11 contract, 11 assets, 17 route probes
- live 14/14 on v2.6.1 (24/24 earlier), 9/9 UI, 7/7 offline
- signed 2.6.1 APK

**Say:**
> I verify at four levels. The Rust gate is fmt, clippy, build and test: 328
> library tests and 24 test binaries, including the consent, Today, contract
> and asset suites. On the live deployment, a two-account API run covers
> registration, friendship, exact sharing, pause/revoke, the trail, notes,
> check-in privacy and deletion: 14 of 14 on v2.6.1, and 24 of 24 on the
> earlier demo-hardening run. The browser run is 9 of 9, and the offline run is
> 7 of 7. The 2.6.1 APK is signed and verified.

## Slide 11 - Live demo (90-120 s)

**Visual:** the running app (switch to the demo windows). Fallback: the
screenshots in `assets/`.

**Say:**
> Let me show the core in two minutes. This is A, this is B. They are friends.
> B shares exact location for one hour, confirms the summary, and A sees the
> exact point. B pauses; A refreshes and sees nothing. B resumes; A sees it
> again. B revokes; it is gone. On A's map I can show the Today trail and a
> private photo note. Then I take A offline: the map and search still work
> because the Moldova pack is installed. Finally, the same account on the
> phone.

Follow the exact steps and fallbacks in [`Demo-Script.md`](./Demo-Script.md).

## Slide 12 - Limitations and next steps (45 s)

**Visual:** Two columns: "Known limits" and "Next".

**Known limits:**
- Admin demo needs DB promotion; no seeded admin.
- MFA login works; web enrolment is not built.
- Offline web routing is not built.
- Native photos are not built.
- The trail records explicit publishes, not background GPS.
- macOS CI has an unrelated installer test flake.

**Next:**
- Admin bootstrap without SQL.
- Web MFA and account recovery.
- Friend-visible notes with per-note consent.
- Offline web routing.
- Mobile photo picker.
- Load-test and replace the per-request full refresh.

**Say:**
> I am explicit about the edges. Admin is implemented but needs a promoted
> account and database access, so I do not demo it without that. MFA works at
> login, but enrolment is not in the web UI. Offline web routing and native
> photos are not built. The trail is explicit publishes, not 24/7 tracking.
> The next steps are admin bootstrap, web MFA, consent-based shared notes,
> offline routing and a mobile photo picker.

## Slide 13 - Close (20 s)

**Visual:** Title/wordmark; QR or URL to the live app and release.

**Say:**
> Pinpoint shows that privacy can be the product, not a settings page:
> sharing is explicit, directional, precise by choice and revocable, and the
> server enforces it. Thank you - I'm happy to take questions.

---

## Q&A preparation

| Likely question | Short answer |
|---|---|
| Why Rust? | One memory-safe codebase for desktop, Android and the API; shared models and wire contract; no runtime. |
| How is consent enforced, not just UI? | Every read loads the accepted friendship, the owner's active share, the expiry and the precision on the server; a modified client cannot widen it. |
| What happens if a share expires mid-session? | The next read fails closed; the UI shows expired and offers "Share again". |
| Can a friend see my location because we are friends? | No. Friendship is a prerequisite, not consent. A share is required. |
| How do you handle multiple replicas? | Security-sensitive writes are direct SQL (users, sessions, friendships, shares, latest location, trail, notes); a stale snapshot cannot resurrect a revoked share. The per-request refresh is the current convergence model, sized for the demo. |
| Is location stored forever? | The latest location is replaced on publish; the trail retains 7 days (24 hours visible); notes stay until deleted; account deletion purges both. |
| What about offline? | Moldova pack in OPFS; map + search work offline; online web routes use OSRM; the native map server routes on the installed graph. |
| What about photos? | Browser downscales to max 1024 px, JPEG ~0.82; server validates JPEG/PNG data URL and a 675 KB bound; owner-only. Native picker is next. |
| Is it production-ready? | No. It is a verified demonstration/small-group build. Admin bootstrap, MFA enrolment, recovery, offline routing, native photos, load testing and CI determinism remain. |
| What did you personally build? | The Rust API and clients, PostgreSQL schema/migrations, consent and precision enforcement, offline pack flow, Today/notes, tests and the live deployment. |
