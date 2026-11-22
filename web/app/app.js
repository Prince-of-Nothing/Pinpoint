// Pinpoint web UI — ES module, same-origin, no third-party code.
// Talks to /api/v1 with the session cookie + X-CSRF-Token double-submit.

import { createMap } from "./map.js";
import { initNavigation } from "./nav.js";
import { createAdminConsole } from "./admin.js";
import { friendName, initSharing, shareRowModel } from "./sharing.js";
import { initAppearance } from "./appearance.js";
import {
  cryptoAvailable,
  deleteIdentity,
  ensureIdentity,
  fingerprint,
  open,
  publicBundle,
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
};

let accountNameDirty = false;
let authBusy = false;
let notePhotoData = null;
let messageIdentity = null;
let messageIdentityUserId = null;
let messagePeer = null;
let messageThread = [];
let messageBusy = false;

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
  state.history = [];
  state.notes = [];
  state.trailVisible = false;
  state.messageConversations = [];
  state.messageUnread = 0;
  messagePeer = null;
  messageThread = [];
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
    }
    ul.appendChild(li);
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
    li.innerHTML = `<span>${escapeHtml(friendName(friendship, state.user?.id))} · ${escapeHtml(
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
    li.textContent = `${loc.user_name} · ${Number(loc.latitude).toFixed(4)}, ${Number(loc.longitude).toFixed(4)} · ${loc.precision_level}`;
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
  $("message-peer").textContent = name || "Chat";
  $("message-safety").textContent = "";
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
    const myBundle = publicBundle(identity);
    const rows = [];
    for (const message of thread.messages || []) {
      const mine = message.sender_id === state.user.id;
      const senderBundle = mine ? myBundle : peerKey;
      let text = null;
      if (senderBundle) {
        text = await open(
          identity,
          senderBundle,
          message.sender_id,
          message.recipient_id,
          message,
          !mine,
        );
      }
      rows.push({ ...message, mine, text });
    }
    messageThread = rows;
    $("message-peer").textContent = thread.peer?.full_name || "Chat";
    if (peerKey) {
      const safety = await fingerprint(peerKey);
      $("message-safety").textContent = safety ? `Safety number: ${safety}` : "";
    } else {
      $("message-safety").textContent =
        "This friend has not enabled encrypted messages yet.";
    }
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
setInterval(() => {
  if (!state.user) return;
  refreshMessages();
  if (messagePeer && !$("message-sheet").classList.contains("hidden")) {
    loadMessageThread(messagePeer.user_id);
  }
}, 5000);

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

async function createAppMap() {
  const pmtilesUrl = await installedPmtilesUrl();
  mapApi = createMap({
    element: $("map"),
    center: [state.map.lat, state.map.lon],
    zoom: state.map.zoom,
    onClick: (point) => nav?.onMapClick(point),
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
      map.setFriendMarkers(state.locations);
      map.setNoteMarkers(state.notes, (note) => showNote(note));
      if (state.trailVisible) map.drawTrail(state.history);
      else map.clearTrail();
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
const drawerHead = drawer.querySelector(".drawer-head");
const tabs = [...document.querySelectorAll(".tab-bar [data-tab]")];
const TAB_CARDS = {
  friends: "friends-card",
  chats: "messages-card",
  sharing: "shares-card",
  today: "today-card",
  account: "account-card",
};
const SHEET_IDS = ["place-sheet", "share-sheet", "route-panel", "message-sheet"];
let drawerCloseTimer = null;

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
  drawerToggle.setAttribute("aria-expanded", String(open));
  if (!open) setActiveTab("map");
}
setDrawer(false);
drawerToggle.addEventListener("click", () => {
  setDrawer(drawerToggle.getAttribute("aria-expanded") !== "true");
});

for (const button of tabs) {
  button.addEventListener("click", () => {
    const name = button.dataset.tab;
    if (name === "map") {
      for (const id of SHEET_IDS) $(id).classList.add("hidden");
      setDrawer(false);
      return;
    }
    setDrawer(true);
    setActiveTab(name);
    $(TAB_CARDS[name])?.scrollIntoView({
      block: "start",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth",
    });
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

$("trail-toggle")?.addEventListener("click", () => {
  state.trailVisible = !state.trailVisible;
  renderToday();
  updateMap();
  if (state.trailVisible && state.history.length >= 2) {
    ensureMap()
      .then((map) =>
        map.fitRoute({
          points: state.history.map((point) => [point.latitude, point.longitude]),
        }),
      )
      .catch(() => {});
  }
});

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
    const note = await api("/api/v1/locations/notes", {
      method: "POST",
      body: {
        latitude: center.lat,
        longitude: center.lng,
        label: $("note-label").value.trim(),
        message,
        ...(notePhotoData ? { photo: notePhotoData } : {}),
      },
    });
    $("note-form").reset();
    notePhotoData = null;
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
