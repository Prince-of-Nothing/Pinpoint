# Privacy Policy & Consent Model

*(Design document for an academic project — not a shipping legal policy. If this product
ever goes public, this becomes the basis of the real privacy statement.)*

## What we collect
| Data                        | Why                                   | Retention                  |
|-----------------------------|---------------------------------------|----------------------------|
| Email + Argon2id hash        | Account identification/authentication | Until account deletion     |
| Display name (pseudonymous) | Shown to consenting recipients        | Until changed/deleted      |
| Role/flags (admin, locked…) | Access control                        | Until changed/deleted      |
| **Current location publish**| Only to serve explicitly granted shares | Overwritten by next publish; **no history kept in MVP** |
| Share permissions           | The consent records themselves        | Until revoked/expired      |
| Friendship relations        | Precondition for any share            | Until removed              |

## What we do NOT do
- No advertising, no profiling, no third-party analytics, no data sales. Ever.
- No background tracking: the server stores only what you actively publish.
- No implicit access: **friendship does not grant visibility** — only explicit, directional
  `SharePermission` records do, each with precision, purpose, and optional expiry.
- No covert features: everything a recipient can see, the sharer can see and revoke instantly.
- No OSM query leakage: only user-initiated searches/routes/POIs reach the OSM
  services, and production self-hosts them (ADR-0011); friend coordinates never
  leave Pinpoint.
- No hidden phone-home: the only automatic external request is a once-a-day
  GitHub releases feed check (ADR-0015), which can be turned off in Profile &
  settings. It sends only `Pinpoint/<version>` as the User-Agent — no account,
  location, friend or device data. GitHub sees the request IP, as with any HTTPS
  request. Manual checks from About always remain available.
- **No IP-based or covert geolocation.** We never derive location from IP
  addresses, WebRTC candidates, call metadata or similar side channels. We do not
  store IP addresses for tracking or analytics. If a short-lived security hash is
  ever needed for rate limiting, it is a **keyed HMAC** (not a plain hash) with a
  short retention; it is pseudonymous, used only for abuse prevention, and never
  for location or profiling.

## Diagnostics you choose to send
- The web client keeps the latest uncaught error in memory only. Nothing is sent
  until you press **Send report**.
- A crash report contains the error name, message and a short stack, with URLs
  and query strings removed and token/password-like fields redacted. It never
  contains location, IP address, account email, session cookie, CSRF token or a
  device identifier.
- The manual **Help & diagnostics** form sends only the category and the message
  you type.
- Reports are used for debugging and abuse handling, not analytics or profiling.

## The consent lifecycle
1. **Grant** — sharer creates a permission naming one recipient, one precision level,
   one purpose, optional expiry.
2. **Use** — each recipient read re-validates the permission server-side (state, pause, expiry).
3. **Pause / Resume** — temporary freeze without destroying the grant's audit trail.
4. **Revoke** — immediate, complete; next request already fails. No grace period, no cache.
5. **Expire** — automatic equivalent of revoke at the timestamp the sharer chose.

## Transparency controls for users
- View every active grant you've given (who, precision, purpose, expiry).
- See who has *asked* nothing — recipients gain access only when you grant it.
- Freshness honesty: stale/dead indicators prevent others from over-trusting old data.
- Account deletion removes identity, relations, permissions, and current location.

## Data subject rights (implemented intent)
- **Access**: export profile, friendships, permissions, current publish.
- **Rectification**: profile update endpoints.
- **Erasure**: delete account cascades to relations/permissions/locations.
- **Objection/withdrawal**: revoke = withdrawal, effective immediately.

## Developer & deployment hygiene
- Demo/seed identities are fictional; never test with real user data.
- Logs must not include location coordinates or password material.
- Any future feature that would extend collection (trails, regions, messaging) requires
  a written threat/privacy analysis in `docs/` **before** implementation (see ROADMAP gates).

## Competitive context (why this model)

See the [wiki competitive analysis](https://github.com/Prince-of-Nothing/Pinpoint/wiki/Components-07-Competitive-Analysis) for the full analysis. The
short version:- **Life360-style harvesting** collects precise geolocation, driving events,
  motion activity and advertising identifiers, and discloses location/movement
  data to business partners for monetisation (with opt-outs). Pinpoint collects
  none of that: no trackers, no ads, no partner sharing, no profiling.
- **Apple's Advanced Data Protection** is the reference model for Pinpoint's
  E2E location encryption: keys live on the device (OS keychain), the server
  stores ciphertext only.
- **Organic Maps / OsmAnd** are the reference for privacy-first offline maps:
  no account, no trackers, OSM data, open source.
- **Pinpoint's differentiator** is directional, per-recipient consent
  (direction + precision + expiry) on top of E2E encryption — no mainstream
  location-sharing app combines both.
