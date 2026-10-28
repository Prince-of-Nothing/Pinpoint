# Web Client End-to-End Evidence

**Controls:** SC-02 (directional consent, revocation), SC-05 (session
handling, CSRF-protected refresh, CSP, XSS), REQ-03/05/06/07.
**Script:** [`e2e/web-client-flow.js`](../../e2e/web-client-flow.js) (Playwright, headless Chromium).
**Recorded:** 2026-09-29 against the API on `http://localhost:8000` with PostgreSQL and Redis.

## Scenario

1. Alice and Bob register and log in through the web forms (cookie mode).
2. Alice sends a friend request; Bob accepts it.
3. Alice sets her display name to `<img src=x onerror=alert(1)>`.
4. Alice shares her **exact** location with Bob for one hour ("Walking home").
5. Alice switches reporting to manual and clicks the map; the location is sent.
6. Alice starts a 30-minute check-in ("Evening run").
7. Bob **reloads** the page: the session resumes via the HttpOnly refresh
   cookie plus the session-bound CSRF token; the access token was only in memory.
8. Bob sees Alice live on the map and in the list, and sees her check-in.
9. Alice stops sharing; Bob's list becomes empty on the next refresh.
10. Bob logs out; all auth cookies are gone; a reload shows the login form.

## Observed result

```json
{
 "cookiesAfterLogout": [],
 "nameShown": "<img src=x onerror=alert(1)> (@alice…)",
 "row": "<img src=x onerror=alert(1)> (@alice…)liveUpdated 0s ago · exact · share until …",
 "markers": 2,
 "sharedCheckin": "<img src=x onerror=alert(1)> (@alice…)on trackDue … · Evening run",
 "afterRevoke": "Nobody is sharing with you right now.",
 "problems": []
}
E2E PASSED
```

- The XSS payload is displayed as literal text everywhere; no `alert()` fired.
- No Content-Security-Policy violations or other console errors (map tile
  requests were blocked by the recording environment's network policy and
  are filtered out; tiles load normally with internet access).

## Bug found and fixed by this test

The Account tab loaded the profile asynchronously and could overwrite
what the user had just typed, saving an empty display name. The form now
tracks edits and never overwrites user input with late-arriving data.
