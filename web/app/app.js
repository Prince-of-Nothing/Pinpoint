// Pinpoint web UI — ES module, same-origin, no third-party code.
// Talks to /api/v1 with the session cookie + X-CSRF-Token double-submit.

import { createMap } from "./map.js";
import { initNavigation } from "./nav.js";
import { createAdminConsole } from "./admin.js";
import {
  expiryText,
  friendName,
  initSharing,
  precisionLabel,
  shareStatus,
} from "./sharing.js";
import { initAppearance } from "./appearance.js";

const state = {
  csrf: null,
  user: null,
  map: { lat: 47.0105, lon: 28.8638, zoom: 13 },
  locations: [],
};

let accountNameDirty = false;
let authBusy = false;

const $ = (id) => document.getElementById(id);

function show(el, visible) {
  el.classList.toggle("hidden", !visible);
}

function setStatus(text, isError = false) {
  const el = $("status");
  el.textContent = text || "";
  el.classList.toggle("error", isError);
}

async function api(path, { method = "GET", body } = {}) {
  const headers = { Accept: "application/json" };
  if (body !== undefined) headers["Content-Type"] = "application/json";
  if (state.csrf && method !== "GET") headers["X-CSRF-Token"] = state.csrf;
  const res = await fetch(path, {
    method,
    headers,
    credentials: "include",
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  // Any authenticated response may re-arm the CSRF token (e.g. /auth/me
  // after a reload); the cookie itself is HttpOnly and unreadable to JS.
  const headerToken = res.headers.get("X-CSRF-Token");
  if (headerToken) state.csrf = headerToken;
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    const msg = err?.error?.message || `HTTP ${res.status}`;
    throw new Error(msg);
  }
  if (res.status === 204) return null;
  return res.json().catch(() => null);
}

function captureCsrf(res, body) {
  const header = res.headers.get("X-CSRF-Token");
  const token = body?.csrf_token || header;
  if (token) state.csrf = token;
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------
async function login(email, password, totp = "") {
  const payload = { email, password };
  if (totp) payload.totp = totp;
  const res = await fetch("/api/v1/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    credentials: "include",
    body: JSON.stringify(payload),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(body?.error?.message || `Login failed (HTTP ${res.status})`);
  captureCsrf(res, body);
  state.user = body.user;
  await enterApp();
}

async function register(email, password, fullName) {
  const res = await fetch("/api/v1/auth/register", {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    credentials: "include",
    body: JSON.stringify({ email, password, full_name: fullName }),
  });
  const body = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(body?.error?.message || `Registration failed (HTTP ${res.status})`);
  captureCsrf(res, body);
  state.user = body.user;
  await enterApp();
}

function resetToLogin() {
  state.user = null;
  state.csrf = null;
  state.locations = [];
  accountNameDirty = false;
  mapApi?.clearRoute();
  mapApi?.clearSelfLocation();
  $("nav-banner").classList.add("hidden");
  $("place-sheet").classList.add("hidden");
  $("route-panel").classList.add("hidden");
  $("share-sheet").classList.add("hidden");
  $("search-results").classList.add("hidden");
  $("saved-list").innerHTML = "";
  $("recent-list").innerHTML = "";
  show($("app-panel"), false);
  show($("session-bar"), false);
  show($("admin-tab"), false);
  $("user-handle").textContent = "";
  adminConsole?.close();
  show($("recovery-panel"), false);
  $("recovery-code").textContent = "";
  show($("login-panel"), true);
  clearPasswordFields();
  $("login-password").value = "";
  $("login-totp").value = "";
  $("delete-confirm").value = "";
  $("delete-password").value = "";
  renderAccount();
}

async function logout() {
  try {
    await api("/api/v1/auth/logout", { method: "POST" });
  } catch (_) {
    // best effort; local state is cleared regardless
  }
  resetToLogin();
  setStatus("Signed out.");
}

async function enterApp() {
  show($("login-panel"), false);
  show($("session-bar"), true);
  show($("app-panel"), true);
  renderIdentity();
  updateAdminTab();
  ensureMap()
    .then((map) => map.map.invalidateSize())
    .catch(() => {});
  if (!sharing) {
    sharing = initSharing({
      api,
      setStatus,
      getUser: () => state.user,
      onChanged: refresh,
    });
  }
  await refresh();
}

function renderIdentity() {
  $("user-email").textContent = state.user?.email || "";
  const handle = state.user?.username;
  $("user-handle").textContent = handle ? `@${handle}` : "";
  renderAccount();
}

function renderAccount() {
  const user = state.user;
  $("account-email").textContent = user?.email || "";
  const handle = user?.username;
  $("account-username").textContent = handle ? `@${handle}` : "No public username yet.";
  $("account-copy-username").disabled = !handle;
  const nameInput = $("account-name");
  if (!accountNameDirty && document.activeElement !== nameInput) {
    nameInput.value = user?.full_name || "";
  }
  const anonymous = user?.auth_method === "anonymous";
  show($("password-form"), !anonymous);
  show($("password-anonymous-note"), anonymous);
  const deleteLabel = $("delete-password-label");
  if (deleteLabel) show(deleteLabel, !anonymous);
}

function clearPasswordFields() {
  for (const id of ["password-current", "password-new", "password-confirm"]) {
    const input = $(id);
    if (input) input.value = "";
  }
}

async function saveDisplayName() {
  if (!state.user) return;
  const name = $("account-name").value.trim();
  if (!name) {
    setStatus("Display name cannot be empty.", true);
    return;
  }
  try {
    const updated = await api(`/api/v1/users/${state.user.id}`, {
      method: "PATCH",
      body: { full_name: name },
    });
    state.user = { ...state.user, ...updated };
    accountNameDirty = false;
    renderIdentity();
    setStatus("Display name saved.");
  } catch (err) {
    setStatus(err.message, true);
  }
}

async function changePassword() {
  if (!state.user) return;
  const current = $("password-current").value;
  const next = $("password-new").value;
  const confirm = $("password-confirm").value;
  if (next !== confirm) {
    setStatus("New passwords do not match.", true);
    return;
  }
  try {
    await api("/api/v1/auth/password/change", {
      method: "POST",
      body: {
        current_password: current,
        new_password: next,
        confirm_password: confirm,
      },
    });
    resetToLogin();
    setStatus("Password changed. Sign in again with the new password.");
  } catch (err) {
    clearPasswordFields();
    setStatus(err.message, true);
  }
}

async function deleteAccount() {
  if (!state.user) return;
  const confirm = $("delete-confirm").value.trim();
  const password = $("delete-password").value;
  if (confirm !== "DELETE") {
    setStatus("Type DELETE to confirm account deletion.", true);
    return;
  }
  if (!window.confirm("Delete your account permanently? This cannot be undone.")) return;
  try {
    await api("/api/v1/users/me", {
      method: "DELETE",
      body: { confirm, current_password: password || null },
    });
    resetToLogin();
    setStatus("Account deleted.");
  } catch (err) {
    setStatus(err.message, true);
  }
}

async function refresh() {
  try {
    const me = await api("/api/v1/auth/me");
    state.user = me;
    renderIdentity();
    updateAdminTab();
  } catch (_) {
    show($("app-panel"), false);
    show($("session-bar"), false);
    show($("login-panel"), true);
    return;
  }
  const [friendships, shares, locations, sessions] = await Promise.all([
    api("/api/v1/friendships/").catch(() => []),
    api("/api/v1/locations/shares").catch(() => []),
    api("/api/v1/locations/friends").catch(() => []),
    api("/api/v1/auth/sessions").catch(() => ({ sessions: [] })),
  ]);
  state.locations = Array.isArray(locations) ? locations : [];
  const friends = Array.isArray(friendships) ? friendships : [];
  renderFriends(friends);
  renderShares(Array.isArray(shares) ? shares : [], friends);
  renderLocations(state.locations);
  renderSessions(Array.isArray(sessions?.sessions) ? sessions.sessions : []);
  updateMap();
  nav?.refreshStorage();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------
function renderFriends(items) {
  const ul = $("friends-list");
  ul.innerHTML = "";
  if (!items.length) {
    ul.innerHTML = '<li class="muted">No friendships yet.</li>';
    return;
  }
  for (const f of items) {
    const li = document.createElement("li");
    const incoming = f.receiver_id === state.user?.id && f.status === "pending";
    li.innerHTML = `<span>${escapeHtml(f.requester_name)} → ${escapeHtml(f.receiver_name)}</span>
      <span class="badge ${f.status}">${escapeHtml(f.status)}</span>`;
    if (incoming) {
      const accept = document.createElement("button");
      accept.textContent = "Accept";
      accept.onclick = () => respond(f.id, "accept");
      const reject = document.createElement("button");
      reject.textContent = "Reject";
      reject.className = "secondary";
      reject.onclick = () => respond(f.id, "reject");
      li.append(accept, reject);
    } else if (f.status === "accepted") {
      const manage = document.createElement("button");
      manage.textContent = "Sharing";
      manage.className = "secondary";
      manage.onclick = () => sharing?.open(f);
      li.appendChild(manage);
    }
    ul.appendChild(li);
  }
}

function renderShares(items, friendships = []) {
  const ul = $("shares-list");
  ul.innerHTML = "";
  if (!items.length) {
    ul.innerHTML = '<li class="muted">You are not sharing your location.</li>';
    return;
  }
  for (const s of items) {
    const friendship =
      friendships.find((f) => f.id === s.friendship_id) || { id: s.friendship_id };
    const status = shareStatus(s);
    const badge = status === "active" ? "accepted" : status === "expired" ? "rejected" : "pending";
    const li = document.createElement("li");
    li.innerHTML = `<span>${escapeHtml(friendName(friendship, state.user?.id))} · ${escapeHtml(
      precisionLabel(s.precision_level),
    )} · ${escapeHtml(expiryText(s))}</span>
      <span class="badge ${badge}">${status}</span>`;
    const revoke = document.createElement("button");
    revoke.textContent = "Revoke";
    revoke.className = "secondary";
    revoke.onclick = () => revokeShare(s.id);
    li.appendChild(revoke);
    if (status === "expired") {
      const again = document.createElement("button");
      again.textContent = "Share again";
      again.className = "secondary";
      again.onclick = () => sharing?.open(friendship);
      li.appendChild(again);
    } else {
      const toggle = document.createElement("button");
      toggle.textContent = s.is_active ? "Pause" : "Resume";
      toggle.className = "secondary";
      toggle.onclick = () => toggleShare(s.id, !s.is_active);
      li.appendChild(toggle);
    }
    ul.appendChild(li);
  }
}

function renderLocations(items) {
  const ul = $("locations-list");
  ul.innerHTML = "";
  if (!items.length) {
    ul.innerHTML = '<li class="muted">No friend locations visible.</li>';
    return;
  }
  for (const loc of items) {
    const li = document.createElement("li");
    li.textContent = `${loc.user_name} · ${Number(loc.latitude).toFixed(4)}, ${Number(loc.longitude).toFixed(4)} · ${loc.precision_level}`;
    ul.appendChild(li);
  }
}

function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
  }[c]));
}

function fmtTime(iso) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "unknown" : d.toLocaleString();
}

function renderSessions(items) {
  const ul = $("sessions-list");
  ul.innerHTML = "";
  if (!items.length) {
    ul.innerHTML = '<li class="muted">No active sessions.</li>';
    return;
  }
  for (const s of items) {
    const li = document.createElement("li");
    li.innerHTML = `<span>${escapeHtml(s.auth_method)} · ${escapeHtml(s.id)}…</span>
      <span class="badge ${s.current ? "accepted" : "pending"}">${s.current ? "this device" : "active"}</span>
      <span class="muted">${escapeHtml(fmtTime(s.last_activity))}</span>`;
    const revoke = document.createElement("button");
    revoke.textContent = s.current ? "Sign out" : "Revoke";
    revoke.className = "secondary";
    revoke.onclick = () => revokeSession(s.id, s.current);
    li.appendChild(revoke);
    ul.appendChild(li);
  }
}

function showRecoveryCode(code) {
  $("recovery-code").textContent = code;
  show($("recovery-panel"), true);
  $("recovery-panel").scrollIntoView({ behavior: "smooth", block: "nearest" });
}

async function anonymousSignIn() {
  if (authBusy) return;
  authBusy = true;
  $("anonymous").disabled = true;
  try {
    const res = await fetch("/api/v1/auth/anonymous", {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      credentials: "include",
      body: JSON.stringify({ device_name: "Web browser" }),
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body?.error?.message || `Anonymous sign-in failed (HTTP ${res.status})`);
    captureCsrf(res, body);
    state.user = body.user;
    showRecoveryCode(body.recovery_code);
    await enterApp();
    setStatus("Anonymous account created. Save the recovery code.");
  } catch (err) {
    setStatus(err.message, true);
  } finally {
    authBusy = false;
    $("anonymous").disabled = false;
  }
}

async function recoverAccount(code) {
  if (authBusy) return;
  authBusy = true;
  try {
    const res = await fetch("/api/v1/auth/anonymous/recover", {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      credentials: "include",
      body: JSON.stringify({ recovery_code: code }),
    });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body?.error?.message || `Recovery failed (HTTP ${res.status})`);
    captureCsrf(res, body);
    state.user = body.user;
    showRecoveryCode(body.recovery_code);
    await enterApp();
    setStatus("Account recovered. The previous code is no longer valid.");
  } catch (err) {
    setStatus(err.message, true);
  } finally {
    authBusy = false;
  }
}

async function revokeSession(id, isCurrent) {
  try {
    await api(`/api/v1/auth/sessions/${encodeURIComponent(id)}`, { method: "DELETE" });
    if (isCurrent) {
      await logout();
      return;
    }
    setStatus("Session revoked.");
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function logoutAll() {
  try {
    await api("/api/v1/auth/logout-all", { method: "POST" });
  } catch (_) {
    // local state is cleared regardless
  }
  resetToLogin();
  setStatus("Signed out on every device.");
}

// ---------------------------------------------------------------------------
// Crash capture: latest uncaught error, sent only when the user asks.
// ---------------------------------------------------------------------------
let lastCrash = null;
let crashSentFor = null;
const crashShownFor = new Set();

function crashSignature(error) {
  return `${error.name || "Error"}: ${error.message || ""}`.slice(0, 240);
}

function sanitizeCrashText(value) {
  return String(value || "")
    .replace(/\b[a-z][a-z0-9+.-]*:\/\/\S+/gi, "[url]")
    .replace(
      /\b(token|password|secret|csrf|session|cookie|authorization)[=:]\s*\S+/gi,
      "$1=[redacted]",
    )
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 2000);
}

function showCrash(error) {
  const signature = crashSignature(error);
  if (crashShownFor.has(signature)) return;
  crashShownFor.add(signature);
  lastCrash = {
    signature,
    message: sanitizeCrashText(
      `${error.name || "Error"}: ${error.message || "Unknown error"}\n${(error.stack || "")
        .split("\n")
        .slice(0, 10)
        .join("\n")}`,
    ),
  };
  show($("crash-banner"), true);
}

async function sendCrash() {
  if (!lastCrash || crashSentFor === lastCrash.signature) return;
  crashSentFor = lastCrash.signature;
  const headers = { "Content-Type": "application/json", Accept: "application/json" };
  if (state.csrf) headers["X-CSRF-Token"] = state.csrf;
  try {
    const res = await fetch("/api/v1/diagnostics/report", {
      method: "POST",
      headers,
      credentials: "include",
      body: JSON.stringify({ category: "crash", message: lastCrash.message, platform: "web" }),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    setStatus("Crash report sent. Thank you.");
  } catch (err) {
    crashSentFor = null;
    setStatus(`Could not send the report: ${err.message}`, true);
  } finally {
    show($("crash-banner"), false);
    lastCrash = null;
  }
}

window.addEventListener("error", (event) => {
  if (event.error) showCrash(event.error);
});
window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason;
  showCrash(reason instanceof Error ? reason : new Error(String(reason)));
});

// ---------------------------------------------------------------------------
// MapLibre/PMTiles map adapter (same-origin assets; app/map.js owns the map)
// ---------------------------------------------------------------------------
let mapApi = null;
let mapPromise = null;
let nav = null;
let adminConsole = null;
let sharing = null;

function canAdmin() {
  return ["admin", "administrator"].includes(state.user?.role);
}

function updateAdminTab() {
  show($("admin-tab"), canAdmin());
  if (!canAdmin()) adminConsole?.close();
}

function ensureAdminConsole() {
  if (!adminConsole) {
    adminConsole = createAdminConsole({
      api,
      setStatus,
      getUser: () => state.user,
    });
  }
  return adminConsole;
}

function ensureMap() {
  if (mapApi) return Promise.resolve(mapApi);
  if (!mapPromise) {
    mapPromise = createAppMap().catch((error) => {
      mapPromise = null;
      throw error;
    });
  }
  return mapPromise;
}

async function createAppMap() {
  const pmtilesUrl = await installedPmtilesUrl();
  mapApi = createMap({
    element: $("map"),
    center: [state.map.lat, state.map.lon],
    zoom: state.map.zoom,
    onClick: (point) => nav?.onMapClick(point),
    ...(pmtilesUrl ? { pmtilesUrl } : {}),
  });
  nav = initNavigation({
    mapApi,
    setStatus,
    getUserId: () => state.user?.id ?? "anon",
  });
  return mapApi;
}

async function installedPmtilesUrl() {
  try {
    const { listPacks, openPack } = await import("./offline.js");
    const packs = await listPacks();
    const pack = packs.find((entry) => entry.id === "moldova") || packs[0];
    if (!pack) return null;
    const opened = await openPack(pack.id);
    return opened?.pmtilesUrl || null;
  } catch (error) {
    console.info(
      "[pinpoint] no installed offline map pack; using the fixture",
      error?.message || String(error),
    );
    return null;
  }
}

function updateMap() {
  ensureMap()
    .then((map) => map.setFriendMarkers(state.locations))
    .catch(() => {});
}

async function registerOfflineWorker() {
  if (!("serviceWorker" in navigator)) return false;
  try {
    await navigator.serviceWorker.register("/sw.js");
    await navigator.serviceWorker.ready;
    return true;
  } catch (error) {
    console.info(
      "[pinpoint] offline worker unavailable",
      error?.message || String(error),
    );
    return false;
  }
}

async function refreshOfflineBanner() {
  const banner = $("offline-map-banner");
  if (!banner) return;
  try {
    const { listPacks } = await import("./offline.js");
    const packs = await listPacks();
    const pack = packs.find((entry) => entry.id === "moldova") || packs[0];
    banner.classList.toggle("hidden", Boolean(pack));
    if (pack) {
      $("offline-map-status").textContent = `${pack.name || pack.id} is installed`;
      $("offline-map-download").hidden = true;
    }
  } catch (error) {
    // Keep the prompt visible: the download may still work even when listing
    // fails (for example before the service worker is active).
    console.info("[pinpoint] offline pack list unavailable", error?.message || error);
  }
}

async function downloadOfflineMap() {
  const button = $("offline-map-download");
  const status = $("offline-map-status");
  button.disabled = true;
  status.textContent = "Fetching the pack manifest…";
  try {
    await registerOfflineWorker();
    const { downloadPack } = await import("./offline.js");
    const installed = await downloadPack("/offline/packs/moldova/manifest.json", {
      onProgress: (progress) => {
        const percent = Number.isFinite(progress?.percent)
          ? ` ${Math.round(progress.percent)}%`
          : "";
        status.textContent = `${progress?.phase || "Downloading"}${percent}`;
      },
    });
    status.textContent = `${installed.name || installed.id} installed; reloading…`;
    location.reload();
  } catch (error) {
    status.textContent = `Download failed: ${error?.message || error}`;
    button.disabled = false;
  }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------
async function respond(id, action) {
  try {
    await api(`/api/v1/friendships/${id}/respond`, { method: "POST", body: { action } });
    setStatus(`Friendship ${action}ed.`);
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function toggleShare(id, isActive) {
  try {
    await api(`/api/v1/locations/shares/${id}`, { method: "PATCH", body: { is_active: isActive } });
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function revokeShare(id) {
  try {
    await api(`/api/v1/locations/shares/${id}`, { method: "DELETE" });
    setStatus("Sharing revoked.");
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function publish(lat, lon) {
  try {
    await api("/api/v1/locations/me", { method: "POST", body: { latitude: lat, longitude: lon, accuracy: 10 } });
    setStatus("Location published.");
    state.map.lat = lat;
    state.map.lon = lon;
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function sendFriendRequest(username) {
  const handle = String(username || "").trim().replace(/^@/, "");
  if (!handle) {
    setStatus("Enter a username.", true);
    return;
  }
  try {
    const found = await api(`/api/v1/users/lookup?username=${encodeURIComponent(handle)}`);
    if (found?.is_self) {
      setStatus("That is your own username.", true);
      return;
    }
    if (found?.friendship_status === "accepted") {
      setStatus(`@${handle} is already a friend.`);
      return;
    }
    if (found?.friendship_status === "pending") {
      setStatus(`A request with @${handle} is already pending.`);
      return;
    }
    await api("/api/v1/friendships/", { method: "POST", body: { receiver_username: handle } });
    setStatus(`Friend request sent to @${handle}.`);
    await refresh();
  } catch (e) {
    setStatus(e.message, true);
  }
}

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------
$("login-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    await login(
      $("login-email").value.trim(),
      $("login-password").value,
      $("login-totp").value.trim(),
    );
    setStatus("");
  } catch (err) {
    setStatus(err.message, true);
  }
});

$("register-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    await register(
      $("register-email").value.trim(),
      $("register-password").value,
      $("register-name").value.trim(),
    );
    setStatus("Account created.");
  } catch (err) {
    setStatus(err.message, true);
  }
});

$("register-toggle").addEventListener("click", () => {
  show($("login-form"), false);
  show($("register-form"), true);
});
$("login-toggle").addEventListener("click", () => {
  show($("register-form"), false);
  show($("login-form"), true);
});

$("anonymous").addEventListener("click", anonymousSignIn);
$("recover-toggle").addEventListener("click", () => {
  show($("login-form"), false);
  show($("register-form"), false);
  show($("recover-form"), true);
});
$("recover-cancel").addEventListener("click", () => {
  show($("recover-form"), false);
  show($("login-form"), true);
});
$("recover-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  await recoverAccount($("recover-code").value.trim());
});
$("recovery-copy").addEventListener("click", async () => {
  const code = $("recovery-code").textContent;
  try {
    await navigator.clipboard.writeText(code);
    setStatus("Recovery code copied.");
  } catch (_) {
    setStatus("Select the code and copy it manually.", true);
  }
});
$("recovery-dismiss").addEventListener("click", () => {
  show($("recovery-panel"), false);
  $("recovery-code").textContent = "";
});

$("logout").addEventListener("click", logout);
$("logout-all").addEventListener("click", logoutAll);

$("admin-tab").addEventListener("click", () => ensureAdminConsole().open("overview"));
$("admin-close").addEventListener("click", () => adminConsole?.close());
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !$("admin-panel").classList.contains("hidden")) {
    adminConsole?.close();
  }
});

$("publish-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const lat = Number($("publish-lat").value);
  const lon = Number($("publish-lon").value);
  if (!Number.isFinite(lat) || !Number.isFinite(lon)) {
    setStatus("Enter a valid latitude and longitude.", true);
    return;
  }
  await publish(lat, lon);
});

$("friend-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  await sendFriendRequest($("friend-username").value.trim());
});

$("diagnostic-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const message = $("diagnostic-message").value.trim();
  if (!message) return;
  try {
    const report = await api("/api/v1/diagnostics/report", {
      method: "POST",
      body: {
        category: $("diagnostic-category").value,
        message,
        platform: "web",
      },
    });
    $("diagnostic-message").value = "";
    setStatus(`Report #${report.id} sent. Thank you.`);
  } catch (err) {
    setStatus(err.message, true);
  }
});

$("locate-button").addEventListener("click", async () => {
  try {
    const map = await ensureMap();
    const fix = await map.locate();
    setStatus(`Located: ${fix.lat.toFixed(5)}, ${fix.lon.toFixed(5)}`);
  } catch (err) {
    setStatus(err.message, true);
  }
});

const drawer = $("social-drawer");
const drawerToggle = $("drawer-toggle");
function setDrawer(open) {
  drawer.classList.toggle("closed", !open);
  drawer.classList.toggle("open", open);
  drawerToggle.setAttribute("aria-expanded", String(open));
}
setDrawer(window.innerWidth >= 900);
drawerToggle.addEventListener("click", () => {
  setDrawer(drawerToggle.getAttribute("aria-expanded") !== "true");
});

$("account-name-form").addEventListener("submit", (e) => {
  e.preventDefault();
  saveDisplayName();
});
$("account-name").addEventListener("input", () => {
  accountNameDirty = true;
});
$("account-copy-username").addEventListener("click", async () => {
  const handle = state.user?.username;
  if (!handle) return;
  try {
    await navigator.clipboard.writeText(`@${handle}`);
    setStatus("Username copied.");
  } catch (_) {
    setStatus(`@${handle}`);
  }
});
$("password-form").addEventListener("submit", (e) => {
  e.preventDefault();
  changePassword();
});
$("delete-account").addEventListener("click", deleteAccount);
$("crash-send").addEventListener("click", sendCrash);
$("crash-dismiss").addEventListener("click", () => {
  show($("crash-banner"), false);
  lastCrash = null;
});

$("offline-map-download")?.addEventListener("click", downloadOfflineMap);
refreshOfflineBanner();
registerOfflineWorker().then(() => refreshOfflineBanner());

// Initial session check (cookie may already be valid).
initAppearance();

(async () => {
  try {
    const me = await api("/api/v1/auth/me");
    state.user = me;
    await enterApp();
  } catch (_) {
    show($("login-panel"), true);
    show($("app-panel"), false);
    show($("session-bar"), false);
  }
})();
