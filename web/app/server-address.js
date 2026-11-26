// Choosing which Pinpoint server the web app talks to.
//
// The web app always talks to the server that served it (same origin), which
// keeps session cookies, CSRF and the CSP simple and strict. Switching server
// therefore means opening the app on the other server and signing in there:
// sessions and keys never cross servers. Same address rule as the native
// client (ADR-0006): https, or plain http only for this computer (loopback).

const LOOPBACK = new Set(["localhost", "127.0.0.1", "[::1]"]);
export const MAX_RECENT_SERVERS = 5;

export function isLoopbackHost(hostname) {
  const host = String(hostname || "").toLowerCase();
  return LOOPBACK.has(host) || host.endsWith(".localhost");
}

/**
 * Validate what someone typed and reduce it to a server origin.
 *
 * @param {string} raw e.g. "pinpoint.example.org", "https://x.org/app", "http://localhost:8080"
 * @param {string} [current] the origin the app runs on now
 * @returns {{ok: true, origin: string, host: string, same: boolean} | {ok: false, error: string}}
 */
export function normalizeServerUrl(raw, current = "") {
  const text = String(raw ?? "").trim();
  if (!text) return { ok: false, error: "Enter a server address." };
  if (text.length > 300) return { ok: false, error: "That address is too long." };
  let url;
  try {
    url = new URL(/^[a-z][a-z0-9+.-]*:\/\//i.test(text) ? text : `https://${text}`);
  } catch {
    return { ok: false, error: "That isn't a valid address." };
  }
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    return { ok: false, error: "Use an https:// address." };
  }
  if (url.username || url.password) {
    return { ok: false, error: "Remove the user name or password from the address." };
  }
  if (!url.hostname) return { ok: false, error: "That isn't a valid address." };
  if (url.protocol === "http:" && !isLoopbackHost(url.hostname)) {
    return { ok: false, error: "Use https://. Plain http only works for a server on this computer." };
  }
  return { ok: true, origin: url.origin, host: url.host, same: url.origin === current };
}

/** Most recent first, no duplicates, at most MAX_RECENT_SERVERS. */
export function rememberServer(list, origin) {
  const previous = Array.isArray(list) ? list.filter((item) => typeof item === "string" && item !== origin) : [];
  return [origin, ...previous].slice(0, MAX_RECENT_SERVERS);
}
