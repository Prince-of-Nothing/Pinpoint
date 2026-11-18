# Admin demo runbook (Midterm 2)

**Status: blocked until the Render external `DATABASE_URL` is available.** There is
no seeded admin account and no HTTP bootstrap; the Admin tab stays hidden for
every account until an existing account is promoted directly in the deployment
database. Nothing in this runbook was executed while writing it.

Target: https://pinpoint-uj4y.onrender.com (v2.6.1).

## 0. Prerequisites

- Access to the Render Postgres instance that backs the live service.
- The **External Database URL**: Render Dashboard -> the Postgres instance ->
  **Connect** -> **External Database URL**. Copy the full `postgres://...`
  string. The *internal* URL only resolves inside Render and will not work from
  the laptop.
- The account must already be **registered and active on the live system**.
  Promotion updates one existing row; it never creates a user, so there is no
  way to "add an admin" first.
- Keep the URL out of the repo, slides, screenshots and shell recordings. Set it
  per shell only.

## 1. Register the account first (live)

1. Open https://pinpoint-uj4y.onrender.com in a clean browser profile.
2. Register the email you intend to promote and confirm you can sign in.
3. Do not use an anonymous account for the admin account.

If this step is skipped, the promote command exits with:

```text
Error: no active account found for <email>; register the account first, then retry
```

## 2. Promote the account (from the repo)

PowerShell, from the repository root:

```powershell
$env:DATABASE_URL = "postgres://..."   # Render External Database URL
cargo run --bin pinpoint-admin -- --promote-email owner@example.com
```

Expected output and exit code 0:

```text
promoted owner@example.com to admin
```

Notes:

- The binary connects straight to Postgres, updates
  `pinpoint_users.role = 'admin'` for that active email and exits. It does not
  need the Render service running and it does not touch sessions.
- `--promote-email` requires `DATABASE_URL`; otherwise it fails with
  `DATABASE_URL is required to promote an account`.
- Connection timeout or DNS failure usually means the internal URL was copied;
  use the External Database URL.
- Build this once before the demo window: `cargo build --bin pinpoint-admin`.
- Optional cleanup after the demo: `Remove-Item Env:DATABASE_URL`.

## 3. Pick up the role and open Admin

1. Wait about 10 seconds: each replica reloads its snapshot from Postgres every
   5 seconds, so the promotion is not instant.
2. On the live site, **sign out and sign in** as the promoted account. (A plain
   reload after the wait also refreshes `/api/v1/auth/me`.)
3. Confirm the **Admin** button appears in the top bar next to **Sign out**.
4. Open **Admin**. The console defaults to **Overview**.

If the tab is still missing, reload once more; if it remains hidden, the
promotion hit a different database or the account has `is_active = false`.

## 4. What to show

**Overview** only, then **Audit**.

- Overview is a read-only counter view: users, active users, sessions,
  friendships, shares, open reports and audit events, plus the app version.
  Explain that counts include all accounts (anonymous and never-signed-in)
  and contain no personal data.
- Audit is the append-only accountability trail: when, actor handle, action,
  target, detail. It stores handles and numeric ids, not emails or
  coordinates.
- Audit may be empty right after the bootstrap: the direct SQL promotion is not
  an API admin action, so it writes no audit row. That is expected. Do not
  create, triage or delete real data just to populate it.
- Read-only tour. Do not change roles, deactivate or delete anyone on the live
  system.

## 5. What not to project

- **Users** tab: it lists every account's real email, name and role. Do not
  open it, and do not show it in screenshots.
- The Admin header prints your own `email · role` in the corner. Keep it out of
  frame or accept that it is the presenter's own address.
- **Diagnostics** can contain user-submitted reports with personal text; leave
  it closed for this demo.
- Never show the terminal with `DATABASE_URL`, the Render dashboard, or the
  connection string.

## 6. Verify the role gate returns 403 for a resident

Use a second, separate browser profile (its own `session_id`) with a normal
account registered on the live site.

1. Confirm that profile has **no Admin** button.
2. Open DevTools -> Console and run:

   ```js
   await fetch("/api/v1/admin/overview", { credentials: "same-origin" })
     .then(async (r) => `${r.status} ${await r.text()}`);
   ```

3. Expected result:

   ```text
   403 {"error":{"code":"FORBIDDEN","message":"Administrator role required."}}
   ```

   `/api/v1/admin/users` and `/api/v1/admin/audit` return the same 403.

Interpretation:

- `403` -> role gate works; the signed-in account is a resident.
- `401` -> that profile is signed out; sign in and retry.
- `200` -> you are still signed in as the promoted account in that profile, so
  the test proves nothing; use the clean profile.

## Fallback if the DB URL is unavailable

Skip the Admin segment. Do not claim the console is demo-ready: the tab is
hidden, the promotion cannot run without the database URL, and there is no
seeded admin. Keep the limitation wording already used in
[`Midterm2-Report.md`](./Midterm2-Report.md) section 9 and
[`Demo-Script.md`](./Demo-Script.md) section 6.
