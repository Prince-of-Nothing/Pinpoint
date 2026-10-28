// ---------------------------------------------------------------------------
// API layer: the ONLY module that talks to the backend.
// Auth: session cookie (same-origin) + double-submit CSRF header on writes.
// ---------------------------------------------------------------------------
import { API_BASE } from "./config.js";

export const state = {
  user: null,
  csrf: null,
  friendships: [],
  shares: [],
};

export async function api(path, opts = {}) {
  const headers = { ...opts.headers };
  if (opts.body !== undefined) headers["Content-Type"] = "application/json";
  const method = (opts.method || "GET").toUpperCase();
  if (method !== "GET" && state.csrf) headers["X-CSRF-Token"] = state.csrf;

  let res;
  try {
    res = await fetch(API_BASE + path, {
      method,
      headers,
      credentials: "same-origin", // session cookie stays first-party
      body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
    });
  } catch (_) {
    const err = new Error("Network unreachable (offline?)");
    err.offline = true;
    throw err;
  }

  // The server re-mirrors the non-HttpOnly `csrf_token` cookie on every
  // authed request (survives reloads by design), and sets the X-CSRF-Token
  // response header on login/session-bootstrap. Prefer the header when
  // present, else fall back to the cookie.
  const rotated = res.headers.get("X-CSRF-Token");
  if (rotated) {
    state.csrf = rotated;
  } else if (!state.csrf) {
    state.csrf = readCsrfCookie();
  }

  let data = null;
  try { data = await res.json(); } catch (_) { /* empty body */ }
  if (!res.ok) {
    const env = (data && data.error) || data || {};
    const err = new Error(env.message || `HTTP ${res.status}`);
    err.status = res.status;
    err.detail = env;
    throw err;
  }
  return data;
}

/** Reads the non-HttpOnly csrf_token companion cookie (reload survival). */
export function readCsrfCookie() {
  const match = document.cookie.match(/(?:^|;\s*)csrf_token=([^;]*)/);
  return match ? decodeURIComponent(match[1]) : null;
}
