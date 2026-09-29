/*
 * Always Together web client.
 *
 * Security notes:
 * - The access token lives only in memory. The refresh token is an HttpOnly
 *   cookie the page cannot read; refreshing requires the CSRF token, which
 *   the server bound to this session.
 * - All user-controlled text is rendered with textContent / DOM nodes, never with
 *   HTML-string APIs, so names, bios, notes and purposes cannot inject markup.
 * - Only same-origin scripts run (Content-Security-Policy: script-src 'self').
 */
"use strict";

const API = "/api/v1";
const POLL_MS = 30000;

const state = {
  accessToken: null,
  user: null,
  map: null,
  markers: new Map(),
  pollTimer: null,
  activeTab: "map",
  report: { mode: "off", watchId: null, lastSent: 0, lastPosition: null, battery: null, manualMarker: null },
  friends: [],
};

// ------------------------------------------------------------------ helpers

const $ = (selector) => document.querySelector(selector);

function el(tag, options = {}, ...children) {
  const node = document.createElement(tag);
  if (options.className) node.className = options.className;
  if (options.text !== undefined) node.textContent = options.text;
  if (options.href) node.href = options.href;
  if (options.attrs) for (const [k, v] of Object.entries(options.attrs)) node.setAttribute(k, v);
  if (options.onClick) node.addEventListener("click", options.onClick);
  for (const child of children) if (child) node.append(child);
  return node;
}

function button(label, onClick, className = "small") {
  return el("button", { className, text: label, onClick, attrs: { type: "button" } });
}

function showBanner(message, kind = "error") {
  const banner = $("#banner");
  banner.textContent = message;
  banner.className = `banner ${kind}`;
  clearTimeout(showBanner.timer);
  showBanner.timer = setTimeout(() => banner.classList.add("hidden"), 6000);
}

function readCookie(name) {
  const match = document.cookie.split("; ").find((part) => part.startsWith(`${name}=`));
  return match ? decodeURIComponent(match.split("=")[1]) : null;
}

function formData(form) {
  const data = {};
  for (const [key, value] of new FormData(form).entries()) {
    const trimmed = typeof value === "string" ? value.trim() : value;
    if (trimmed !== "") data[key] = trimmed;
  }
  return data;
}

function ago(isoString) {
  if (!isoString) return "never";
  const seconds = Math.max(0, Math.round((Date.now() - new Date(isoString).getTime()) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.round(seconds / 60)} min ago`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)} h ago`;
  return `${Math.round(seconds / 86400)} d ago`;
}

function when(isoString) {
  return isoString ? new Date(isoString).toLocaleString() : "no expiry";
}

function displayName(user) {
  return user.display_name ? `${user.display_name} (@${user.username})` : `@${user.username}`;
}

function emptyItem(text) {
  return el("li", { className: "empty", text });
}

// ------------------------------------------------------------------- API

class ApiError extends Error {
  constructor(status, body) {
    super((body && body.detail) || `Request failed (${status})`);
    this.status = status;
    this.body = body;
  }
}

async function refreshAccessToken() {
  const csrf = readCookie("at_csrf");
  if (!csrf) return false;
  const response = await fetch(`${API}/auth/refresh`, {
    method: "POST",
    headers: { "X-CSRF-Token": csrf },
    credentials: "same-origin",
  });
  if (!response.ok) return false;
  const body = await response.json();
  state.accessToken = body.access_token;
  state.user = body.user;
  return true;
}

async function api(path, { method = "GET", body, retry = true } = {}) {
  const headers = {};
  if (state.accessToken) headers.Authorization = `Bearer ${state.accessToken}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const response = await fetch(`${API}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    credentials: "same-origin",
  });
  if (response.status === 401 && retry && path !== "/auth/login") {
    if (await refreshAccessToken()) return api(path, { method, body, retry: false });
    showLoggedOut();
    throw new ApiError(401, { detail: "Your session has ended. Please log in again." });
  }
  if (response.status === 204) return null;
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new ApiError(response.status, data);
  return data;
}

function explain(error) {
  if (error instanceof ApiError && error.body && error.body.errors) {
    const messages = Object.values(error.body.errors).flat();
    return `${error.message} ${messages.join(" ")}`;
  }
  return error.message || "Something went wrong.";
}

async function run(action, successMessage) {
  try {
    const result = await action();
    if (successMessage) showBanner(successMessage, "ok");
    return result;
  } catch (error) {
    if (!(error instanceof ApiError && error.status === 401)) showBanner(explain(error));
    return undefined;
  }
}

// ------------------------------------------------------------ auth views

function showLoggedOut() {
  state.accessToken = null;
  state.user = null;
  stopReporting();
  clearInterval(state.pollTimer);
  $("#app-view").classList.add("hidden");
  $("#whoami").classList.add("hidden");
  $("#auth-view").classList.remove("hidden");
}

function showLoggedIn() {
  $("#auth-view").classList.add("hidden");
  $("#app-view").classList.remove("hidden");
  $("#whoami").classList.remove("hidden");
  $("#whoami-name").textContent = displayName(state.user);
  initMap();
  refreshAll();
  clearInterval(state.pollTimer);
  state.pollTimer = setInterval(refreshActiveTab, POLL_MS);
}

async function onLogin(event) {
  event.preventDefault();
  const form = event.target;
  const data = formData(form);
  await run(async () => {
    const body = await api("/auth/login", {
      method: "POST",
      body: { login: data.login, password: data.password, token_delivery: "cookie" },
      retry: false,
    });
    state.accessToken = body.access_token;
    state.user = body.user;
    form.reset();
    showLoggedIn();
  });
}

async function onRegister(event) {
  event.preventDefault();
  const form = event.target;
  const data = formData(form);
  const created = await run(
    () => api("/auth/register", { method: "POST", body: data, retry: false }),
    "Account created. You can log in now."
  );
  if (created) {
    form.reset();
    $("#login-form").elements.login.value = created.username;
  }
}

async function onLogout() {
  await run(() => api("/auth/logout", { method: "POST", retry: false }));
  showLoggedOut();
}

// -------------------------------------------------------------------- tabs

function switchTab(name) {
  state.activeTab = name;
  document.querySelectorAll(".tabs button").forEach((b) => b.classList.toggle("active", b.dataset.tab === name));
  document.querySelectorAll(".tab").forEach((t) => t.classList.toggle("hidden", t.id !== `tab-${name}`));
  if (name === "map" && state.map) setTimeout(() => state.map.invalidateSize(), 50);
  refreshAll();
}

const TAB_REFRESH = {
  map: () => refreshMap(),
  friends: () => refreshFriends(),
  sharing: () => { refreshShares(); refreshFriends(); },
  checkins: () => refreshCheckIns(),
  account: () => refreshAccount(),
};

function refreshActiveTab() {
  if (state.activeTab !== "map") refreshMap();
  TAB_REFRESH[state.activeTab]();
}

function refreshAll() {
  refreshMap();
  refreshFriends();
  refreshShares();
  refreshCheckIns();
  refreshAccount();
}

// --------------------------------------------------------------------- map

function initMap() {
  if (state.map || typeof L === "undefined") return;
  state.map = L.map("map").setView([44.4268, 26.1025], 12);
  L.tileLayer("https://tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
  }).addTo(state.map);
  state.map.on("click", (event) => {
    if (state.report.mode !== "manual") return;
    sendLocation({ latitude: event.latlng.lat, longitude: event.latlng.lng, accuracy_m: 10 }, true);
  });
}

const FRESHNESS_COLOURS = { live: "#1a9d55", stale: "#d98b00", dead: "#8a93a0" };
const FRESHNESS_BADGE = { live: "live", stale: "stale", dead: "no signal", unknown: "no data" };
const FRESHNESS_TEXT = {
  live: "Live",
  stale: "Stale - no recent update",
  dead: "No signal - last known position",
  unknown: "No location received yet",
};

function popupFor(item) {
  const box = el("div");
  box.append(el("strong", { text: displayName(item.user) }));
  box.append(el("div", { text: `${FRESHNESS_TEXT[item.freshness]} (${ago(item.recorded_at)})` }));
  const battery = item.last_battery_pct === null ? "unknown" : `${item.last_battery_pct}%`;
  box.append(el("div", { text: `Last known battery: ${battery}${item.low_battery ? " (low)" : ""}` }));
  box.append(el("div", { text: `Precision: ${item.precision}, accuracy ±${Math.round(item.accuracy_m || 0)} m` }));
  if (item.navigation) {
    const links = el("div", { className: "popup-links" });
    links.append(el("a", { text: "OpenStreetMap", href: item.navigation.openstreetmap, attrs: { target: "_blank", rel: "noopener noreferrer" } }));
    links.append(el("a", { text: "Google Maps", href: item.navigation.google_maps, attrs: { target: "_blank", rel: "noopener noreferrer" } }));
    links.append(el("a", { text: "Maps app", href: item.navigation.geo_uri }));
    box.append(links);
  }
  return box;
}

async function refreshMap() {
  if (!state.accessToken) return;
  const items = await run(() => api("/locations/visible"));
  if (!items) return;
  const list = $("#visible-list");
  list.replaceChildren();
  if (items.length === 0) list.append(emptyItem("Nobody is sharing with you right now."));

  const seen = new Set();
  for (const item of items) {
    const row = el("li", {},
      el("span", { text: displayName(item.user) }),
      el("span", { className: `badge ${item.freshness}`, text: FRESHNESS_BADGE[item.freshness] }),
      el("span", { className: "meta", text: `Updated ${ago(item.recorded_at)} · ${item.precision} · share ${item.share_expires_at ? "until " + when(item.share_expires_at) : "until revoked"}` })
    );
    list.append(row);
    if (!state.map || item.latitude === null) continue;
    seen.add(item.share_id);
    const colour = FRESHNESS_COLOURS[item.freshness] || "#8a93a0";
    let entry = state.markers.get(item.share_id);
    if (!entry) {
      entry = {
        dot: L.circleMarker([item.latitude, item.longitude], { radius: 8, weight: 2 }).addTo(state.map),
        area: L.circle([item.latitude, item.longitude], { radius: 1, weight: 1, fillOpacity: 0.08 }).addTo(state.map),
      };
      state.markers.set(item.share_id, entry);
      row.addEventListener("click", () => state.map.setView([item.latitude, item.longitude], 15));
    }
    entry.dot.setLatLng([item.latitude, item.longitude]).setStyle({ color: colour, fillColor: colour, fillOpacity: item.freshness === "live" ? 0.9 : 0.4, dashArray: item.freshness === "live" ? null : "4 4" });
    entry.area.setLatLng([item.latitude, item.longitude]).setRadius(item.accuracy_m || 1).setStyle({ color: colour });
    entry.dot.bindPopup(popupFor(item));
  }
  for (const [shareId, entry] of state.markers) {
    if (!seen.has(shareId)) {
      entry.dot.remove();
      entry.area.remove();
      state.markers.delete(shareId);
    }
  }
}

// --------------------------------------------------- my location reporting

async function readBattery() {
  if (!navigator.getBattery) return null;
  try {
    const battery = await navigator.getBattery();
    return { level: Math.round(battery.level * 100), charging: battery.charging };
  } catch {
    return null;
  }
}

function effectiveIntervalSeconds() {
  const chosen = Number($("#report-preset").value);
  const battery = state.report.battery;
  if (battery && battery.level <= 20 && !battery.charging) return Math.max(chosen, 600);
  return chosen;
}

async function sendLocation(coords, force = false) {
  const now = Date.now();
  if (!force && now - state.report.lastSent < effectiveIntervalSeconds() * 1000) return;
  state.report.battery = await readBattery();
  const body = {
    latitude: Number(coords.latitude.toFixed(6)),
    longitude: Number(coords.longitude.toFixed(6)),
    accuracy_m: coords.accuracy_m ?? coords.accuracy ?? null,
  };
  if (state.report.battery) {
    body.battery_pct = state.report.battery.level;
    body.is_charging = state.report.battery.charging;
  }
  const result = await run(() => api("/locations/me", { method: "PUT", body }));
  if (!result) return;
  state.report.lastSent = now;
  const saver = effectiveIntervalSeconds() > Number($("#report-preset").value);
  $("#my-status").textContent = `Last sent ${new Date().toLocaleTimeString()}${saver ? " · battery saver active" : ""}`;
  if (state.map) {
    const point = [body.latitude, body.longitude];
    if (!state.report.manualMarker) {
      state.report.manualMarker = L.circleMarker(point, { radius: 6, color: "#2463eb", fillOpacity: 1 }).addTo(state.map).bindTooltip("You");
    } else {
      state.report.manualMarker.setLatLng(point);
    }
  }
}

function stopReporting() {
  if (state.report.watchId !== null) navigator.geolocation.clearWatch(state.report.watchId);
  state.report.watchId = null;
  if (state.report.manualMarker) state.report.manualMarker.remove();
  state.report.manualMarker = null;
}

function onReportModeChange() {
  stopReporting();
  const mode = $("#report-mode").value;
  state.report.mode = mode;
  state.report.lastSent = 0;
  if (mode === "off") {
    $("#my-status").textContent = "Not sending.";
  } else if (mode === "manual") {
    $("#my-status").textContent = "Click the map to set your position.";
  } else if (!navigator.geolocation) {
    showBanner("This browser does not provide device location.");
    $("#report-mode").value = "off";
  } else {
    $("#my-status").textContent = "Waiting for device location…";
    state.report.watchId = navigator.geolocation.watchPosition(
      (position) => sendLocation(position.coords),
      (error) => showBanner(`Location unavailable: ${error.message}`),
      { enableHighAccuracy: Number($("#report-preset").value) <= 30, maximumAge: 30000, timeout: 60000 }
    );
  }
}

async function onEraseLocation() {
  await run(() => api("/locations/me", { method: "DELETE" }), "Your stored location was erased.");
  $("#report-mode").value = "off";
  onReportModeChange();
  refreshMap();
}

// ----------------------------------------------------------------- friends

async function refreshFriends() {
  const [data, blocks] = await Promise.all([run(() => api("/friendships")), run(() => api("/blocks"))]);
  if (!data) return;
  state.friends = data.friends;

  const incoming = $("#incoming-list");
  incoming.replaceChildren(...(data.incoming.length ? data.incoming.map((f) =>
    el("li", {}, el("span", { text: displayName(f.user) }),
      el("span", { className: "actions" },
        button("Accept", () => friendAction(`/friendships/${f.id}/accept`, "POST", "Friend request accepted.")),
        button("Decline", () => friendAction(`/friendships/${f.id}/decline`, "POST"), "small ghost")))
  ) : [emptyItem("No incoming requests.")]));

  const outgoing = $("#outgoing-list");
  outgoing.replaceChildren(...(data.outgoing.length ? data.outgoing.map((f) =>
    el("li", {}, el("span", { text: displayName(f.user) }),
      button("Cancel", () => friendAction(`/friendships/${f.id}`, "DELETE"), "small ghost"))
  ) : [emptyItem("No pending requests.")]));

  const friends = $("#friends-list");
  friends.replaceChildren(...(data.friends.length ? data.friends.map((f) =>
    el("li", {}, el("span", { text: displayName(f.user) }),
      el("span", { className: "actions" },
        button("Remove", () => confirm(`Remove ${f.user.username} as a friend? Location sharing between you ends too.`) && friendAction(`/friendships/${f.id}`, "DELETE", "Friend removed.")),
        button("Block", () => confirm(`Block ${f.user.username}? They will not be able to find or contact you.`) && blockUser(f.user.username), "small danger")))
  ) : [emptyItem("No friends yet.")]));

  const select = $("#share-friend");
  select.replaceChildren(...data.friends.map((f) => el("option", { text: displayName(f.user), attrs: { value: f.user.username } })));

  const blocked = $("#blocked-list");
  if (blocks) {
    blocked.replaceChildren(...(blocks.length ? blocks.map((b) =>
      el("li", {}, el("span", { text: `@${b.user.username}` }),
        button("Unblock", () => run(() => api(`/blocks/${encodeURIComponent(b.user.username)}`, { method: "DELETE" }), "User unblocked.").then(refreshFriends), "small ghost"))
    ) : [emptyItem("Nobody is blocked.")]));
  }
}

async function friendAction(path, method, message) {
  await run(() => api(path, { method }), message);
  refreshFriends();
  refreshShares();
}

async function blockUser(username) {
  await run(() => api("/blocks", { method: "POST", body: { username } }), "User blocked.");
  refreshFriends();
  refreshShares();
}

async function onFriendRequest(event) {
  event.preventDefault();
  const form = event.target;
  const ok = await run(() => api("/friendships", { method: "POST", body: formData(form) }), "Friend request sent.");
  if (ok) form.reset();
  refreshFriends();
}

// ----------------------------------------------------------------- sharing

async function refreshShares() {
  const data = await run(() => api("/location-shares"));
  if (!data) return;
  const outgoing = $("#outgoing-shares");
  outgoing.replaceChildren(...(data.outgoing.length ? data.outgoing.map((s) =>
    el("li", {},
      el("span", { text: displayName(s.recipient) }),
      el("span", { className: "actions" },
        button(s.precision === "exact" ? "Make approximate" : "Make exact", () => updateShare(s.id, { precision: s.precision === "exact" ? "approximate" : "exact" }), "small ghost"),
        button("Stop sharing", () => endShare(s.id, "Sharing stopped."), "small danger")),
      el("span", { className: "meta", text: `${s.precision} · ${s.expires_at ? "until " + when(s.expires_at) : "until you stop"}${s.purpose ? " · " + s.purpose : ""}` }))
  ) : [emptyItem("You are not sharing your location.")]));

  const incoming = $("#incoming-shares");
  incoming.replaceChildren(...(data.incoming.length ? data.incoming.map((s) =>
    el("li", {},
      el("span", { text: displayName(s.owner) }),
      button("Stop receiving", () => endShare(s.id, "You no longer receive this location."), "small ghost"),
      el("span", { className: "meta", text: `${s.precision} · ${s.expires_at ? "until " + when(s.expires_at) : "until revoked"}${s.purpose ? " · " + s.purpose : ""}` }))
  ) : [emptyItem("Nobody shares with you.")]));
}

async function onShare(event) {
  event.preventDefault();
  const data = formData(event.target);
  if (!data.recipient_username) return showBanner("Add a friend first, then share with them.");
  if (data.expires_in_minutes) data.expires_in_minutes = Number(data.expires_in_minutes);
  const ok = await run(() => api("/location-shares", { method: "POST", body: data }), "You are now sharing your location.");
  if (ok) event.target.elements.purpose.value = "";
  refreshShares();
}

async function updateShare(id, body) {
  await run(() => api(`/location-shares/${encodeURIComponent(id)}`, { method: "PATCH", body }), "Share updated.");
  refreshShares();
}

async function endShare(id, message) {
  await run(() => api(`/location-shares/${encodeURIComponent(id)}`, { method: "DELETE" }), message);
  refreshShares();
  refreshMap();
}

// --------------------------------------------------------------- check-ins

const CHECKIN_TEXT = {
  pending: "on track",
  due_soon: "due soon",
  overdue: "overdue (grace period)",
  missed: "MISSED",
  completed: "completed",
  cancelled: "cancelled",
};

async function refreshCheckIns() {
  const [mine, shared] = await Promise.all([run(() => api("/check-ins")), run(() => api("/check-ins/shared"))]);
  if (mine) {
    $("#my-checkins").replaceChildren(...(mine.length ? mine.map((c) => {
      const active = ["pending", "due_soon", "overdue", "missed"].includes(c.state);
      return el("li", {},
        el("span", { className: `badge ${c.state}`, text: CHECKIN_TEXT[c.state] }),
        active ? el("span", { className: "actions" },
          button("I'm OK", () => checkInAction(c.id, "complete", "Checked in. Stay safe!")),
          button("Cancel", () => checkInAction(c.id, "cancel"), "small ghost")) : null,
        el("span", { className: "meta", text: `Due ${when(c.due_at)}${c.note ? " · " + c.note : ""}` }));
    }) : [emptyItem("No check-ins.")]));
    const urgent = mine.find((c) => ["due_soon", "overdue", "missed"].includes(c.state));
    if (urgent) showBanner(`Reminder: your check-in is ${CHECKIN_TEXT[urgent.state]}. Open Check-ins and tap "I'm OK".`, "warning");
  }
  if (shared) {
    $("#shared-checkins").replaceChildren(...(shared.length ? shared.map((c) =>
      el("li", {},
        el("span", { text: displayName(c.user) }),
        el("span", { className: `badge ${c.state}`, text: CHECKIN_TEXT[c.state] }),
        el("span", { className: "meta", text: `Due ${when(c.due_at)}${c.note ? " · " + c.note : ""}` }))
    ) : [emptyItem("No check-ins from friends.")]));
  }
}

async function checkInAction(id, action, message) {
  await run(() => api(`/check-ins/${encodeURIComponent(id)}/${action}`, { method: "POST" }), message);
  refreshCheckIns();
}

async function onCheckIn(event) {
  event.preventDefault();
  const data = formData(event.target);
  data.due_in_minutes = Number(data.due_in_minutes);
  data.grace_minutes = Number(data.grace_minutes);
  const ok = await run(() => api("/check-ins", { method: "POST", body: data }), "Check-in started.");
  if (ok) event.target.elements.note.value = "";
  refreshCheckIns();
}

// ----------------------------------------------------------------- account

async function refreshAccount() {
  const [me, sessions] = await Promise.all([run(() => api("/users/me")), run(() => api("/auth/sessions"))]);
  const form = $("#profile-form");
  if (me && form.dataset.dirty !== "true") {
    // Never overwrite what the user is typing with values that arrived later
    form.elements.display_name.value = me.display_name || "";
    form.elements.bio.value = me.bio || "";
  }
  if (sessions) {
    $("#sessions-list").replaceChildren(...sessions.map((s) =>
      el("li", {},
        el("span", { text: s.current ? "This browser" : (s.user_agent || "Unknown device").slice(0, 60) }),
        s.current ? el("span", { className: "badge live", text: "current" }) : button("Revoke", async () => {
          await run(() => api(`/auth/sessions/${encodeURIComponent(s.id)}`, { method: "DELETE" }), "Session revoked.");
          refreshAccount();
        }, "small ghost"),
        el("span", { className: "meta", text: `Signed in ${when(s.created_at)} · last active ${ago(s.last_seen_at)} · ${s.ip_address || ""}` }))
    ));
  }
}

async function onProfile(event) {
  event.preventDefault();
  const form = event.target;
  const body = {
    display_name: form.elements.display_name.value.trim() || null,
    bio: form.elements.bio.value.trim() || null,
  };
  const me = await run(() => api("/users/me", { method: "PATCH", body }), "Profile saved.");
  if (me) {
    form.dataset.dirty = "false";
    state.user = me;
    $("#whoami-name").textContent = displayName(me);
  }
}

async function onPasswordChange(event) {
  event.preventDefault();
  const ok = await run(() => api("/auth/password", { method: "POST", body: formData(event.target) }));
  if (ok === null) {
    event.target.reset();
    showLoggedOut();
    showBanner("Password changed. All sessions were ended; please log in again.", "ok");
  }
}

async function onLogoutAll() {
  if (!confirm("Log out on every device?")) return;
  await run(() => api("/auth/logout-all", { method: "POST", retry: false }));
  showLoggedOut();
}

// -------------------------------------------------------------------- boot

document.addEventListener("DOMContentLoaded", async () => {
  $("#login-form").addEventListener("submit", onLogin);
  $("#register-form").addEventListener("submit", onRegister);
  $("#logout-btn").addEventListener("click", onLogout);
  document.querySelectorAll(".tabs button").forEach((b) => b.addEventListener("click", () => switchTab(b.dataset.tab)));
  $("#report-mode").addEventListener("change", onReportModeChange);
  $("#report-preset").addEventListener("change", () => { state.report.lastSent = 0; });
  $("#erase-location").addEventListener("click", onEraseLocation);
  $("#friend-request-form").addEventListener("submit", onFriendRequest);
  $("#share-form").addEventListener("submit", onShare);
  $("#checkin-form").addEventListener("submit", onCheckIn);
  $("#profile-form").addEventListener("submit", onProfile);
  $("#profile-form").addEventListener("input", (event) => { event.currentTarget.dataset.dirty = "true"; });
  $("#password-form").addEventListener("submit", onPasswordChange);
  $("#logout-all-btn").addEventListener("click", onLogoutAll);

  // Resume an existing session using the HttpOnly refresh cookie
  if (await refreshAccessToken()) showLoggedIn();
  else showLoggedOut();
});
