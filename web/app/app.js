// Pinpoint web UI — ES module, same-origin, no third-party code.
// Talks to /api/v1 with the session cookie + X-CSRF-Token double-submit.

import { createMap } from "./map.js";
import { initNavigation } from "./nav.js";
import { createAdminConsole } from "./admin.js";
import { freshnessText, friendName, initSharing, shareRowModel } from "./sharing.js";
import { initAppearance } from "./appearance.js";
import {
  cryptoAvailable,
  deleteIdentity,
  ensureIdentity,
  open,
  publicBundle,
  safetyNumber,
  seal,
} from "./message-crypto.js";

const state = {
  csrf: null,
  user: null,
  map: { lat: 47.0105, lon: 28.8638, zoom: 13 },
  locations: [],
  history: [],
  notes: [],
  trailVisible: false,
  messageConversations: [],
  messageUnread: 0,
  friendshipCount: 0,
  layers: { friends: true, notes: true },
};

let accountNameDirty = false;
let authBusy = false;
let notePhotoData = null;
let pendingNotePoint = null;
let messageIdentity = null;
let messageIdentityUserId = null;
let messagePeer = null;
let messageSafety = null;
let messageThread = [];
let messageBusy = false;
let messagePollBusy = false;

const $ = (id) => document.getElementById(id);

function show(el, visible) {
  el.classList.toggle("hidden", !visible);
}

function setStatus(text, isError = false) {
  const el = $("status");
  el.textContent = text || "";
  el.classList.toggle("error", isError);
}

// HTML minlength counts UTF-16 units while the server counts Unicode
// characters; a 4-emoji password is 8 units but only 4 characters.
function passwordTooShort(value) {
  return Array.from(value).length < 8;
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
    // An expired or revoked session must not leave a signed-in shell full of
    // failed actions; return to the login form on the first 401.
    if (res.status === 401) resetToLogin();
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
  state.history = [];
  state.notes = [];
  pendingNotePoint = null;
  state.trailVisible = false;
  state.messageConversations = [];
  state.messageUnread = 0;
  state.friendshipCount = 0;
  const chatsLabel = $("chats-label");
  if (chatsLabel) chatsLabel.textContent = "Chats";
  messagePeer = null;
  messageThread = [];
  messageSafety = null;
  messageIdentity = null;
  messageIdentityUserId = null;
  accountNameDirty = false;
  mapApi?.clearRoute();
  mapApi?.clearSelfLocation();
  mapApi?.clearTrail();
  mapApi?.setNoteMarkers([]);
  $("nav-banner").classList.add("hidden");
  $("place-sheet").classList.add("hidden");
  $("route-panel").classList.add("hidden");
  $("share-sheet").classList.add("hidden");
  $("message-sheet").classList.add("hidden");
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
  // The auth forms are hidden; clear the secrets so they do not linger in the DOM.
  for (const id of ["login-password", "login-totp", "register-password"]) {
    const input = $(id);
    if (input) input.value = "";
  }
  renderIdentity();
  updateAdminTab();
  // A temporary password blocks the API until it is rotated. Take the user
  // straight to the Account panel instead of letting every tab fail.
  if (state.user?.must_change_password) {
    setDrawer(true, false);
    setActiveTab("account");
    $("account-card")?.scrollIntoView({ block: "start" });
    setStatus("Change the temporary password before using the app.", true);
  }
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
  if (passwordTooShort(next)) {
    setStatus("Password must be at least 8 characters.", true);
    return;
  }
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
  const userId = state.user?.id;
  try {
    await api("/api/v1/users/me", {
      method: "DELETE",
      body: { confirm, current_password: password || null },
    });
    // The account is gone server-side; drop the local message identity too.
    try {
      await deleteIdentity(userId);
    } catch (_) {
      // Best effort: a missing key is already deleted.
    }
    try {
      localStorage.removeItem(verifiedStorageKey(userId));
      localStorage.removeItem(bundlesStorageKey(userId));
    } catch (_) {
      // Storage can be unavailable in private mode.
    }
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
  // A temporary password blocks every other API route; the Account panel
  // handles the rotation, so do not fire requests that will 403.
  if (state.user?.must_change_password) return;
  const [friendships, shares, locations, sessions, history, notes] = await Promise.all([
    api("/api/v1/friendships/").catch(() => []),
    api("/api/v1/locations/shares").catch(() => []),
    api("/api/v1/locations/friends").catch(() => []),
    api("/api/v1/auth/sessions").catch(() => ({ sessions: [] })),
    api("/api/v1/locations/history").catch(() => []),
    api("/api/v1/locations/notes").catch(() => []),
  ]);
  state.locations = Array.isArray(locations) ? locations : [];
  state.history = Array.isArray(history) ? history : [];
  state.notes = Array.isArray(notes) ? notes : [];
  const friends = Array.isArray(friendships) ? friendships : [];
  renderFriends(friends);
  renderShares(Array.isArray(shares) ? shares : [], friends);
  renderLocations(state.locations);
  renderToday();
  renderNotes();
  renderSessions(Array.isArray(sessions?.sessions) ? sessions.sessions : []);
  await refreshMessages();
  updateMap();
  nav?.refreshStorage();
  updateMapEmptyState();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------
function renderFriends(items) {
  // Only non-rejected rows can become an accepted friendship, and messages
  // need one; the poll below is pointless before that.
  state.friendshipCount = items.filter((f) => f.status !== "rejected").length;
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
      const chat = document.createElement("button");
      chat.textContent = "Message";
      chat.className = "secondary";
      const friendId =
        f.requester_id === state.user?.id ? f.receiver_id : f.requester_id;
      chat.onclick = () =>
        openMessageThread(friendId, friendName(f, state.user?.id));
      li.appendChild(chat);
      const manage = document.createElement("button");
      manage.textContent = "Sharing";
      manage.className = "secondary";
      manage.onclick = () => sharing?.open(f);
      li.appendChild(manage);
      const remove = document.createElement("button");
      remove.textContent = "Remove";
      remove.className = "secondary";
      remove.onclick = () =>
        removeFriendship(
          f.id,
          "Remove this friend? Any location shares with them stop too.",
          "Friend removed.",
        );
      li.appendChild(remove);
    } else if (f.requester_id === state.user?.id) {
      const cancel = document.createElement("button");
      cancel.textContent = "Cancel request";
      cancel.className = "secondary";
      cancel.onclick = () =>
        removeFriendship(
          f.id,
          "Cancel this friend request?",
          "Friend request cancelled.",
        );
      li.appendChild(cancel);
    }
    ul.appendChild(li);
  }
}

async function removeFriendship(id, confirmation, message) {
  if (!window.confirm(confirmation)) return;
  try {
    await api(`/api/v1/friendships/${id}`, { method: "DELETE" });
    setStatus(message);
    await refresh();
  } catch (error) {
    setStatus(error.message, true);
  }
}

function renderShares(items, friendships = []) {
  const ul = $("shares-list");
  ul.innerHTML = "";
  const rows = items
    .map((share) => ({ share, model: shareRowModel(share) }))
    .filter(({ model }) => model.isGrant);
  if (!rows.length) {
    ul.innerHTML = '<li class="muted">You are not sharing your location.</li>';
    return;
  }
  for (const { share: s, model } of rows) {
    const friendship =
      friendships.find((f) => f.id === s.friendship_id) || { id: s.friendship_id };
    const badge =
      model.status === "active" ? "accepted" : model.status === "expired" ? "rejected" : "pending";
    const li = document.createElement("li");
    const label = model.label ? `${escapeHtml(model.label)} · ` : "";
    li.innerHTML = `<span>${label}${escapeHtml(friendName(friendship, state.user?.id))} · ${escapeHtml(
      model.precision,
    )} · ${escapeHtml(model.expiry)}</span>
      <span class="badge ${badge}">${model.status}</span>`;
    const revoke = document.createElement("button");
    revoke.textContent = "Revoke";
    revoke.className = "secondary";
    revoke.onclick = () => revokeShare(s.id);
    li.appendChild(revoke);
    if (model.canShareAgain) {
      const again = document.createElement("button");
      again.textContent = "Share again";
      again.className = "secondary";
      again.onclick = () => sharing?.open(friendship);
      li.appendChild(again);
    } else if (model.canPauseResume) {
      const toggle = document.createElement("button");
      toggle.textContent = model.status === "active" ? "Pause" : "Resume";
      toggle.className = "secondary";
      toggle.onclick = () => toggleShare(s.id, model.status !== "active");
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
    li.textContent = `${loc.user_name} · ${Number(loc.latitude).toFixed(4)}, ${Number(loc.longitude).toFixed(4)} · ${loc.precision_level} · updated ${freshnessText(loc.updated_at)}`;
    ul.appendChild(li);
  }
}

function renderToday() {
  const points = Array.isArray(state.history) ? state.history : [];
  const count = $("today-count");
  if (count) count.textContent = `${points.length} ${points.length === 1 ? "point" : "points"}`;
  const list = $("trail-list");
  if (list) {
    list.innerHTML = "";
    for (const point of [...points].reverse().slice(0, 12)) {
      const li = document.createElement("li");
      const time = document.createElement("time");
      time.dateTime = point.recorded_at;
      time.textContent = new Date(point.recorded_at).toLocaleTimeString([], {
        hour: "2-digit",
        minute: "2-digit",
      });
      const coords = document.createElement("span");
      coords.className = "coords";
      coords.textContent = `${Number(point.latitude).toFixed(4)}, ${Number(point.longitude).toFixed(4)}`;
      li.append(time, coords);
      list.append(li);
    }
  }
  const toggle = $("trail-toggle");
  if (toggle) {
    toggle.disabled = points.length < 2;
    toggle.textContent = state.trailVisible ? "Hide trail" : "Show trail";
    toggle.setAttribute("aria-pressed", String(state.trailVisible));
  }
  const summary = $("today-summary");
  if (summary) {
    summary.textContent = points.length
      ? "Your private trail for the last 24 hours. It never leaves your account."
      : "No points yet. Publish your location or use GPS to add a point.";
  }
}

function renderNotes() {
  const ul = $("notes-list");
  if (!ul) return;
  ul.innerHTML = "";
  if (!state.notes.length) {
    ul.innerHTML = '<li class="muted">No notes yet. Pan the map and add one.</li>';
    return;
  }
  for (const note of [...state.notes].reverse()) {
    const li = document.createElement("li");
    const details = document.createElement("details");
    details.dataset.noteId = String(note.id);
    const summary = document.createElement("summary");
    summary.textContent = note.label || "Private note";
    const body = document.createElement("div");
    body.className = "note-body";
    const message = document.createElement("p");
    message.textContent = note.message;
    body.append(message);
    if (note.photo) {
      const img = document.createElement("img");
      img.src = note.photo;
      img.alt = note.label ? `Photo for ${note.label}` : "Private note photo";
      img.loading = "lazy";
      body.append(img);
    }
    const meta = document.createElement("p");
    meta.className = "muted small";
    meta.textContent = `${Number(note.latitude).toFixed(4)}, ${Number(note.longitude).toFixed(4)} · ${fmtTime(note.created_at)}`;
    body.append(meta);
    const actions = document.createElement("div");
    actions.className = "note-actions";
    const show = document.createElement("button");
    show.type = "button";
    show.className = "secondary";
    show.textContent = "Show on map";
    show.onclick = () => showNote(note);
    const del = document.createElement("button");
    del.type = "button";
    del.className = "secondary";
    del.textContent = "Delete";
    del.onclick = () => deleteNote(note.id);
    actions.append(show, del);
    body.append(actions);
    details.append(summary, body);
    li.append(details);
    ul.append(li);
  }
}

function showNote(note) {
  const lat = Number(note.latitude);
  const lon = Number(note.longitude);
  ensureMap()
    .then((map) => map.map.flyTo([lat, lon], Math.max(map.map.getZoom(), 15), { duration: 0.6 }))
    .catch(() => {});
  const details = document.querySelector(`#notes-list details[data-note-id="${note.id}"]`);
  if (details) {
    details.open = true;
    details.scrollIntoView({ block: "nearest" });
  }
}

async function deleteNote(id) {
  if (!window.confirm("Delete this private note?")) return;
  try {
    await api(`/api/v1/locations/notes/${id}`, { method: "DELETE" });
    setStatus("Note deleted.");
    await refresh();
  } catch (error) {
    setStatus(error.message, true);
  }
}

async function readNotePhoto(file) {
  if (!/^image\/(jpeg|png)$/.test(file.type)) {
    throw new Error("Choose a JPEG or PNG photo.");
  }
  if (file.size > 8 * 1024 * 1024) {
    throw new Error("Photo must be under 8 MB.");
  }
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 1024 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  canvas.getContext("2d").drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close?.();
  return canvas.toDataURL("image/jpeg", 0.82);
}

async function updateMapEmptyState() {
  const el = $("map-empty-state");
  if (!el) return;
  let pack = false;
  try {
    const { listPacks } = await import("./offline.js");
    const packs = await listPacks();
    pack = Array.isArray(packs) && packs.length > 0;
  } catch {
    // The offline module may not be ready; keep the prompt visible.
  }
  const online = Boolean($("online-map-toggle")?.checked);
  el.classList.toggle("hidden", online || pack);
  const download = $("offline-map-download");
  if (download) download.hidden = pack;
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

// ---------------------------------------------------------------------------
// End-to-end encrypted messages (ECIES; see web/app/message-crypto.js)
// ---------------------------------------------------------------------------

function verifiedStorageKey(userId) {
  return `pinpoint-message-verified-${userId}`;
}

const MAX_PEER_BUNDLES = 8;

function bundlesStorageKey(userId) {
  return `pinpoint-message-bundles-${userId}`;
}

function loadPeerBundles() {
  if (!state.user) return {};
  try {
    return JSON.parse(localStorage.getItem(bundlesStorageKey(state.user.id))) || {};
  } catch (_) {
    return {};
  }
}

function savePeerBundle(peerId, bundle) {
  if (!state.user || !bundle?.x25519 || !bundle?.ed25519) return;
  const all = loadPeerBundles();
  const key = String(peerId);
  const list = Array.isArray(all[key]) ? all[key] : [];
  if (
    list.some(
      (known) =>
        known.x25519 === bundle.x25519 && known.ed25519 === bundle.ed25519,
    )
  ) {
    return;
  }
  list.push({ x25519: bundle.x25519, ed25519: bundle.ed25519 });
  if (list.length > MAX_PEER_BUNDLES) {
    list.splice(0, list.length - MAX_PEER_BUNDLES);
  }
  all[key] = list;
  try {
    localStorage.setItem(bundlesStorageKey(state.user.id), JSON.stringify(all));
  } catch (_) {
    // Storage can be unavailable in private mode.
  }
}

function peerBundlesFor(peerId, current) {
  const all = loadPeerBundles();
  const list = Array.isArray(all[String(peerId)]) ? [...all[String(peerId)]] : [];
  if (
    current?.x25519 &&
    current?.ed25519 &&
    !list.some(
      (known) =>
        known.x25519 === current.x25519 && known.ed25519 === current.ed25519,
    )
  ) {
    list.push(current);
  }
  return list;
}

function loadVerifiedPeers() {
  if (!state.user) return {};
  try {
    return JSON.parse(localStorage.getItem(verifiedStorageKey(state.user.id))) || {};
  } catch (_) {
    return {};
  }
}

function saveVerifiedPeer(peerId, value) {
  if (!state.user || !value) return;
  const verified = loadVerifiedPeers();
  verified[String(peerId)] = value;
  try {
    localStorage.setItem(
      verifiedStorageKey(state.user.id),
      JSON.stringify(verified),
    );
  } catch (_) {
    // Storage can be unavailable in private mode.
  }
}

function renderMessageSafety() {
  const label = $("message-safety");
  const button = $("message-verify");
  if (!label || !button) return;
  if (!messagePeer?.key) {
    label.textContent = messagePeer
      ? "This friend has not enabled encrypted messages yet."
      : "";
    button.classList.add("hidden");
    return;
  }
  if (!messageSafety) {
    label.textContent = "";
    button.classList.add("hidden");
    return;
  }
  const verified = loadVerifiedPeers()[String(messagePeer.user_id)];
  if (verified === messageSafety) {
    label.textContent = `Safety number: ${messageSafety} \u00b7 Verified`;
    button.classList.add("hidden");
    return;
  }
  label.textContent = verified
    ? `Safety number changed. Compare again: ${messageSafety}`
    : `Safety number: ${messageSafety}`;
  button.textContent = verified ? "Mark as verified again" : "Mark as verified";
  button.classList.remove("hidden");
}

async function ensureMessageIdentity() {
  if (!state.user) return null;
  if (messageIdentity && messageIdentityUserId === state.user.id) {
    return messageIdentity;
  }
  const note = $("messages-crypto-note");
  if (!cryptoAvailable()) {
    if (note) {
      note.textContent =
        "This browser cannot use encrypted messages. Use a current Chrome/Edge or the Android app.";
    }
    return null;
  }
  try {
    messageIdentity = await ensureIdentity(state.user.id);
    messageIdentityUserId = state.user.id;
    await api("/api/v1/messages/key", {
      method: "PUT",
      body: publicBundle(messageIdentity),
    });
    if (note) note.textContent = "";
    return messageIdentity;
  } catch (error) {
    messageIdentity = null;
    messageIdentityUserId = null;
    if (note) note.textContent = error?.message || "Encrypted messages are unavailable.";
    return null;
  }
}

async function refreshMessages() {
  if (!state.user) return;
  const identity = await ensureMessageIdentity();
  if (!identity) {
    renderMessages();
    return;
  }
  try {
    const [inbox, unread] = await Promise.all([
      api("/api/v1/messages/conversations"),
      api("/api/v1/messages/unread"),
    ]);
    state.messageConversations = Array.isArray(inbox?.conversations)
      ? inbox.conversations
      : [];
    state.messageUnread = Number(unread?.unread) || 0;
    renderMessages();
  } catch (error) {
    console.info("[pinpoint] message inbox refresh failed", error?.message || error);
  }
}

function renderMessages() {
  const ul = $("conversations-list");
  if (!ul) return;
  ul.innerHTML = "";
  const unread = Number(state.messageUnread) || 0;
  $("messages-unread").textContent = `${unread} unread`;
  const tab = $("chats-label");
  if (tab) tab.textContent = unread > 0 ? `Chats (${unread})` : "Chats";
  if (!state.messageConversations.length) {
    ul.innerHTML =
      '<li class="muted">No conversations yet. Add a friend and send the first message.</li>';
    return;
  }
  for (const conversation of state.messageConversations) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "secondary";
    const name = conversation.full_name || `@${conversation.username || "friend"}`;
    button.textContent =
      conversation.unread > 0 ? `${name} (${conversation.unread})` : name;
    button.addEventListener("click", () =>
      openMessageThread(conversation.user_id, name),
    );
    li.appendChild(button);
    if (conversation.last_message_at) {
      const meta = document.createElement("span");
      meta.className = "muted small";
      meta.textContent = fmtTime(conversation.last_message_at);
      li.appendChild(meta);
    }
    ul.appendChild(li);
  }
}

async function openMessageThread(friendId, name) {
  if (!state.user) return;
  setDrawer(false);
  messagePeer = { user_id: friendId, full_name: name || "Chat", key: null };
  messageThread = [];
  messageSafety = null;
  $("message-peer").textContent = name || "Chat";
  renderMessageSafety();
  $("message-thread").textContent = "Loading…";
  $("message-status").textContent = "";
  show($("message-sheet"), true);
  await loadMessageThread(friendId);
}

async function loadMessageThread(friendId) {
  if (messageBusy) return;
  messageBusy = true;
  try {
    const identity = await ensureMessageIdentity();
    if (!identity) {
      $("message-thread").textContent =
        "Encrypted messages are unavailable in this browser.";
      return;
    }
    const thread = await api(`/api/v1/messages/${friendId}?limit=50`);
    messagePeer = thread.peer;
    const peerKey = thread.peer?.key || null;
    if (peerKey) savePeerBundle(friendId, peerKey);
    const peerBundles = peerKey ? peerBundlesFor(friendId, peerKey) : [];
    const myBundle = publicBundle(identity);
    const rows = [];
    for (const message of thread.messages || []) {
      const mine = message.sender_id === state.user.id;
      let text = null;
      if (mine) {
        text = await open(
          identity,
          myBundle,
          message.sender_id,
          message.recipient_id,
          message,
          false,
        );
      } else {
        for (const bundle of [...peerBundles].reverse()) {
          text = await open(
            identity,
            bundle,
            message.sender_id,
            message.recipient_id,
            message,
            true,
          );
          if (text !== null) break;
        }
      }
      rows.push({ ...message, mine, text });
    }
    messageThread = rows;
    $("message-peer").textContent = thread.peer?.full_name || "Chat";
    messageSafety = peerKey
      ? await safetyNumber(myBundle, state.user.id, peerKey, friendId)
      : null;
    renderMessageSafety();
    renderMessageThread();
    // Only write read state when the friend's copy actually changed.
    const hasUnread = (thread.messages || []).some(
      (message) => message.sender_id === friendId && !message.read_at,
    );
    if (hasUnread) {
      await api("/api/v1/messages/read", {
        method: "POST",
        body: { friend_id: friendId },
      });
    }
    await refreshMessages();
  } catch (error) {
    $("message-thread").textContent =
      error?.message || "Could not load this conversation.";
  } finally {
    messageBusy = false;
  }
}

function renderMessageThread() {
  const container = $("message-thread");
  container.innerHTML = "";
  if (!messageThread.length) {
    container.innerHTML = '<p class="muted">No messages yet. Say hello.</p>';
    return;
  }
  for (const message of messageThread) {
    const row = document.createElement("div");
    row.className = `message-row ${message.mine ? "mine" : "theirs"}`;
    const bubble = document.createElement("div");
    bubble.className = "message-bubble";
    bubble.textContent =
      message.text ?? "Unable to decrypt this message (the key changed).";
    const meta = document.createElement("span");
    meta.className = "message-meta";
    meta.textContent = `${fmtTime(message.created_at)}${
      message.mine && message.read_at ? " · read" : ""
    }`;
    bubble.appendChild(meta);
    row.appendChild(bubble);
    container.appendChild(row);
  }
  container.scrollTop = container.scrollHeight;
}

async function sendMessage(event) {
  event.preventDefault();
  if (!messagePeer || !state.user) return;
  const input = $("message-input");
  const text = input.value.trim();
  if (!text) return;
  try {
    const identity = await ensureMessageIdentity();
    if (!identity) throw new Error("Encrypted messages are unavailable.");
    if (!messagePeer.key) {
      throw new Error("This friend has not enabled encrypted messages yet.");
    }
    const sealed = await seal(
      identity,
      messagePeer.key,
      state.user.id,
      messagePeer.user_id,
      text,
    );
    if (!sealed) throw new Error("Message must be 1-2000 characters.");
    await api("/api/v1/messages", {
      method: "POST",
      body: { recipient_id: messagePeer.user_id, message: sealed },
    });
    input.value = "";
    $("message-status").textContent = "Sent.";
    await loadMessageThread(messagePeer.user_id);
  } catch (error) {
    $("message-status").textContent = error?.message || "Could not send.";
  }
}

$("message-close")?.addEventListener("click", () => show($("message-sheet"), false));
$("message-form")?.addEventListener("submit", sendMessage);
$("message-verify")?.addEventListener("click", () => {
  if (!messagePeer || !messageSafety) return;
  saveVerifiedPeer(messagePeer.user_id, messageSafety);
  renderMessageSafety();
  $("message-status").textContent = "Safety number verified on this device.";
});
async function pollMessages() {
  if (
    !state.user ||
    state.user.must_change_password ||
    state.friendshipCount === 0 ||
    document.hidden ||
    messagePollBusy
  )
    return;
  messagePollBusy = true;
  try {
    await refreshMessages();
    if (messagePeer && !$("message-sheet").classList.contains("hidden")) {
      await loadMessageThread(messagePeer.user_id);
    }
  } catch (_) {
    // A transient failure must not break the next tick.
  } finally {
    messagePollBusy = false;
  }
}

setInterval(pollMessages, 5000);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) pollMessages();
});

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
    $("recover-code").value = "";
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

let locationPrompted = false;

/**
 * Ask for geolocation once after sign-in when the browser has not decided.
 * Automatic failures stay quiet; the crosshair is the explicit retry.
 */
async function promptLocationOnce(map) {
  if (locationPrompted) return;
  locationPrompted = true;
  try {
    const status = await navigator.permissions?.query?.({ name: "geolocation" });
    if (status?.state === "denied") return;
  } catch (_) {
    // Permission API unavailable; let getCurrentPosition decide.
  }
  try {
    await map.locate();
  } catch (_) {
    // Denied or timed out: no nag on the automatic attempt.
  }
}

function pinNoteAt(point) {
  const lat = Number(point?.lat);
  const lon = Number(point?.lon);
  if (!Number.isFinite(lat) || !Number.isFinite(lon)) return;
  pendingNotePoint = { lat, lon };
  mapApi?.setSelectedPlace({ latitude: lat, longitude: lon, label: "New note" });
  setDrawer(true, false);
  setActiveTab("today");
  $("today-card")?.scrollIntoView({ block: "start" });
  setStatus(`Note pinned at ${lat.toFixed(5)}, ${lon.toFixed(5)}. Write the note below.`);
  $("note-message")?.focus();
}

async function createAppMap() {
  const pmtilesUrl = await installedPmtilesUrl();
  const theme = document.documentElement.dataset.theme === "dark" ? "dark" : "light";
  mapApi = createMap({
    element: $("map"),
    center: [state.map.lat, state.map.lon],
    zoom: state.map.zoom,
    theme,
    onClick: (point) => nav?.onMapClick(point),
    onLongPress: (point) => pinNoteAt(point),
    onLoad: (api) => {
      api.setNoteMarkers(state.notes, (note) => showNote(note));
      if (state.trailVisible) api.drawTrail(state.history);
    },
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
    .then((map) => {
      map.setFriendMarkers(state.layers.friends ? state.locations : []);
      map.setNoteMarkers(state.layers.notes ? state.notes : [], (note) => showNote(note));
      if (state.trailVisible) map.drawTrail(state.history);
      else map.clearTrail();
      renderBasemap();
      if (state.user) promptLocationOnce(map);
    })
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
  await updateMapEmptyState();
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
  const password = $("register-password").value;
  if (passwordTooShort(password)) {
    setStatus("Password must be at least 8 characters.", true);
    return;
  }
  try {
    await register(
      $("register-email").value.trim(),
      password,
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
  $("recover-code").value = "";
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
    e.preventDefault();
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
const drawerHead = drawer.querySelector(".drawer-head");
const tabs = [...document.querySelectorAll(".tab-bar [data-tab]")];
const TAB_META = {
  friends: { eyebrow: "People", title: "Friends" },
  chats: { eyebrow: "Messages", title: "Chats" },
  sharing: { eyebrow: "Consent", title: "Sharing" },
  today: { eyebrow: "Your trail", title: "Today" },
  account: { eyebrow: "Settings", title: "Account" },
};
const SHEET_IDS = ["place-sheet", "share-sheet", "route-panel", "message-sheet"];
let drawerCloseTimer = null;

/** Each tab is its own screen; only its cards are rendered. */
function setScreen(name) {
  const meta = TAB_META[name];
  if (!meta) return;
  drawer.dataset.screen = name;
  const eyebrow = $("drawer-eyebrow");
  if (eyebrow) eyebrow.textContent = meta.eyebrow;
  const title = $("drawer-title");
  if (title) title.textContent = meta.title;
  drawer.scrollTop = 0;
  setActiveTab(name);
}

function setActiveTab(name) {
  for (const button of tabs) {
    const active = button.dataset.tab === name;
    button.classList.toggle("active", active);
    if (active) button.setAttribute("aria-current", "page");
    else button.removeAttribute("aria-current");
  }
}

function setDrawer(open, animate = true) {
  clearTimeout(drawerCloseTimer);
  drawer.classList.remove("closing");
  if (open) {
    drawer.classList.remove("closed");
    drawer.classList.add("open");
  } else if (animate && drawer.classList.contains("open")) {
    drawer.classList.remove("open");
    drawer.classList.add("closing");
    drawerCloseTimer = setTimeout(() => {
      drawer.classList.add("closed");
      drawer.classList.remove("closing");
    }, 180);
  } else {
    drawer.classList.remove("open");
    drawer.classList.add("closed");
  }
  // Keep the closed drawer out of the accessibility tree, and never leave
  // focus on a control that is about to become hidden.
  if (!open && drawer.contains(document.activeElement)) drawerToggle.focus();
  drawer.setAttribute("aria-hidden", String(!open));
  drawerToggle.setAttribute("aria-expanded", String(open));
  if (!open) setActiveTab("map");
}
setDrawer(false);
drawerToggle.addEventListener("click", () => {
  setDrawer(drawerToggle.getAttribute("aria-expanded") !== "true");
});
document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  // The admin overlay owns Escape while it is open.
  if (!$("admin-panel").classList.contains("hidden")) return;
  if (drawerToggle.getAttribute("aria-expanded") === "true") {
    setDrawer(false);
    drawerToggle.focus();
    event.preventDefault();
  }
});
// A map sheet is the next surface under the drawer and admin overlay.
document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape" || event.defaultPrevented) return;
  if (!$("admin-panel").classList.contains("hidden")) return;
  if (drawerToggle.getAttribute("aria-expanded") === "true") return;
  const sheet = SHEET_IDS.map((id) => $(id)).find(
    (el) => el && !el.classList.contains("hidden"),
  );
  if (!sheet) return;
  const close = sheet.querySelector(".sheet-close");
  if (close) close.click();
  else sheet.classList.add("hidden");
  event.preventDefault();
});

for (const button of tabs) {
  button.addEventListener("click", () => {
    const name = button.dataset.tab;
    if (name === "map") {
      for (const id of SHEET_IDS) $(id).classList.add("hidden");
      setDrawer(false);
      return;
    }
    setScreen(name);
    setDrawer(true);
  });
}

// Drag the sheet by its header; the body keeps its normal scroll.
let dragStartY = 0;
let dragDistance = 0;
let drawerDragging = false;

drawerHead.addEventListener("pointerdown", (event) => {
  if (event.button !== 0 || event.target.closest("button")) return;
  drawerDragging = true;
  dragStartY = event.clientY;
  dragDistance = 0;
  drawer.classList.add("dragging");
  drawerHead.setPointerCapture(event.pointerId);
});
drawerHead.addEventListener("pointermove", (event) => {
  if (!drawerDragging) return;
  dragDistance = Math.max(0, event.clientY - dragStartY);
  drawer.style.transform = `translateY(${dragDistance}px)`;
});
function endDrawerDrag() {
  if (!drawerDragging) return;
  drawerDragging = false;
  drawer.classList.remove("dragging");
  drawer.style.transform = "";
  if (dragDistance > 90) setDrawer(false, false);
  dragDistance = 0;
}
drawerHead.addEventListener("pointerup", endDrawerDrag);
drawerHead.addEventListener("pointercancel", endDrawerDrag);

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
$("empty-download")?.addEventListener("click", downloadOfflineMap);
$("empty-online")?.addEventListener("click", () => {
  const toggle = $("online-map-toggle");
  if (toggle && !toggle.checked) {
    toggle.checked = true;
    toggle.dispatchEvent(new Event("change", { bubbles: true }));
  }
  updateMapEmptyState();
});
$("online-map-toggle")?.addEventListener("change", updateMapEmptyState);
$("drawer-close")?.addEventListener("click", () => setDrawer(false));

function setTrail(visible) {
  state.trailVisible = visible;
  const box = $("layer-trail");
  if (box) box.checked = visible;
  renderToday();
  updateMap();
  if (visible && state.history.length >= 2) {
    ensureMap()
      .then((map) =>
        map.fitRoute({
          points: state.history.map((point) => [point.latitude, point.longitude]),
        }),
      )
      .catch(() => {});
  }
}

$("trail-toggle")?.addEventListener("click", () => setTrail(!state.trailVisible));
$("layer-trail")?.addEventListener("change", (event) => setTrail(event.target.checked));

// ---------------------------------------------------------------------------
// Map options: basemap and layers
// ---------------------------------------------------------------------------
/** The account card's checkbox is the single source of truth for the basemap. */
function renderBasemap() {
  const online = Boolean($("online-map-toggle")?.checked);
  for (const [id, expected] of [
    ["basemap-offline", false],
    ["basemap-streets", true],
  ]) {
    const button = $(id);
    if (!button) continue;
    button.classList.toggle("active", online === expected);
    button.setAttribute("aria-pressed", String(online === expected));
  }
}

function chooseBasemap(online) {
  const toggle = $("online-map-toggle");
  if (!toggle) return;
  toggle.checked = online;
  // The map module switches the style on this event.
  toggle.dispatchEvent(new Event("change", { bubbles: true }));
  renderBasemap();
}

function setMapControls(open) {
  const panel = $("map-controls");
  const button = $("map-layers-toggle");
  if (!panel || !button) return;
  if (open) renderBasemap();
  panel.classList.toggle("hidden", !open);
  button.setAttribute("aria-expanded", String(open));
}

$("map-layers-toggle")?.addEventListener("click", () => {
  setMapControls($("map-controls").classList.contains("hidden"));
});
$("basemap-offline")?.addEventListener("click", () => chooseBasemap(false));
$("basemap-streets")?.addEventListener("click", () => chooseBasemap(true));
$("online-map-toggle")?.addEventListener("change", renderBasemap);
$("layer-friends")?.addEventListener("change", (event) => {
  state.layers.friends = event.target.checked;
  updateMap();
});
$("layer-notes")?.addEventListener("change", (event) => {
  state.layers.notes = event.target.checked;
  updateMap();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") setMapControls(false);
});
document.addEventListener("click", (event) => {
  const panel = $("map-controls");
  const button = $("map-layers-toggle");
  if (!panel || panel.classList.contains("hidden")) return;
  if (panel.contains(event.target) || button?.contains(event.target)) return;
  setMapControls(false);
});

// The theme toggle must repaint the map too, not only the page chrome.
document.addEventListener("pinpoint:theme", (event) => {
  mapApi?.setTheme(event.detail === "dark" ? "dark" : "light");
});

// ---------------------------------------------------------------------------
// OAuth sign-in
// ---------------------------------------------------------------------------
const OAUTH_ERRORS = {
  provider_denied: "The provider sign-in was cancelled.",
  state_mismatch: "The sign-in attempt expired. Try again.",
  exchange_failed: "The provider sign-in failed. Try again.",
  userinfo_failed: "The provider did not return your profile. Try again.",
  email_missing: "The provider did not share an email address.",
  email_unverified: "The provider email is not verified.",
  email_exists:
    "An account with this email already exists. Sign in with your password first; linking a provider is not automatic.",
  account_disabled: "This account is disabled.",
  account_failed: "The account could not be created. Try again.",
  provider_unknown: "This sign-in provider is not enabled.",
};

async function loadOauthProviders() {
  const box = $("oauth-buttons");
  if (!box) return;
  try {
    const data = await api("/api/v1/auth/providers");
    const providers = Array.isArray(data?.providers) ? data.providers : [];
    box.innerHTML = "";
    for (const provider of providers) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "secondary";
      button.textContent = `Continue with ${provider[0].toUpperCase()}${provider.slice(1)}`;
      button.addEventListener("click", () => {
        window.location.assign(
          `/api/v1/auth/oauth/${encodeURIComponent(provider)}/start`,
        );
      });
      box.appendChild(button);
    }
    show(box, providers.length > 0);
  } catch (_) {
    show(box, false);
  }
}

function showOauthError() {
  const params = new URLSearchParams(window.location.search);
  const code = params.get("oauth_error");
  if (!code) return;
  setStatus(OAUTH_ERRORS[code] || "Provider sign-in failed.", true);
  params.delete("oauth_error");
  const rest = params.toString();
  history.replaceState(null, "", `${location.pathname}${rest ? `?${rest}` : ""}`);
}

loadOauthProviders();
showOauthError();

$("note-photo")?.addEventListener("change", async (event) => {
  const file = event.target.files?.[0];
  const preview = $("note-preview");
  notePhotoData = null;
  show(preview, false);
  preview.removeAttribute("src");
  if (!file) return;
  try {
    notePhotoData = await readNotePhoto(file);
    preview.src = notePhotoData;
    show(preview, true);
  } catch (error) {
    event.target.value = "";
    setStatus(error.message, true);
  }
});

$("note-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const message = $("note-message").value.trim();
  if (!message) {
    setStatus("Write a message for the note.", true);
    return;
  }
  try {
    const map = await ensureMap();
    const center = map.map.getCenter();
    const latitude = pendingNotePoint?.lat ?? center.lat;
    const longitude = pendingNotePoint?.lon ?? center.lng;
    const note = await api("/api/v1/locations/notes", {
      method: "POST",
      body: {
        latitude,
        longitude,
        label: $("note-label").value.trim(),
        message,
        ...(notePhotoData ? { photo: notePhotoData } : {}),
      },
    });
    $("note-form").reset();
    notePhotoData = null;
    pendingNotePoint = null;
    show($("note-preview"), false);
    $("note-preview").removeAttribute("src");
    setStatus("Private note saved.");
    await refresh();
    showNote(note);
  } catch (error) {
    setStatus(error.message, true);
  }
});

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
