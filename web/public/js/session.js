// ---------------------------------------------------------------------------
// Session lifecycle: login, logout, and restore-after-reload via the API's
// cookie-bootstrap endpoint (POST /auth/session).
// ---------------------------------------------------------------------------
import { api, state, readCsrfCookie } from "./api.js";

export async function login(email, password) {
  const data = await api("/auth/login", { method: "POST", body: { email, password } });
  state.csrf = data.csrf_token;
  state.user = data.user;
  return data.user;
}

export async function logout() {
  try { await api("/auth/logout", { method: "POST" }); } catch (_) { /* session may be gone */ }
  state.user = null;
  state.csrf = null;
}

/**
 * Attempts to restore a session after page reload. The session cookie is
 * HttpOnly (JS never sees it); POST /auth/session validates it server-side,
 * refreshes the idle timer and returns the user + a fresh CSRF header.
 * Returns the user or null when there is no valid session.
 */
export async function restoreSession() {
  // The CSRF cookie is readable immediately; the token from the login
  // response header died with the previous page.
  state.csrf = readCsrfCookie();
  try {
    const data = await api("/auth/session", { method: "POST" });
    state.user = data.user;
    return data.user;
  } catch (_) {
    state.user = null;
    return null;
  }
}
