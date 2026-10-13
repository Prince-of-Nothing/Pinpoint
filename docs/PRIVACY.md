# Privacy Policy & Consent Model

*(Design document for an academic project — not a shipping legal policy. If this product
ever goes public, this becomes the basis of the real privacy statement.)*

## What we collect
| Data                        | Why                                   | Retention                  |
|-----------------------------|---------------------------------------|----------------------------|
| Email + bcrypt hash         | Account identification/authentication | Until account deletion     |
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
