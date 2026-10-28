"use strict";

// ---------------------------------------------------------------------------
// Always Together — web client (vanilla JS SPA)
// Talks to the Go API through the nginx same-origin proxy (/api/v1/...).
// Auth: session cookie + double-submit CSRF token (X-CSRF-Token header).
// ---------------------------------------------------------------------------

const API = "/api/v1";
const state = { user: null, csrf: null, accessToken: null, friendships: [], shares: [] };

// ---- helpers ---------------------------------------------------------------
const $ = (sel) => document.querySelector(sel);
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));

async function api(path, opts = {}) {
  const headers = Object.assign({}, opts.headers);
  if (opts.body !== undefined) headers["Content-Type"] = "application/json";
  const method = (opts.method || "GET").toUpperCase();
  if (method !== "GET" && state.csrf) headers["X-CSRF-Token"] = state.csrf;

  const res = await fetch(API + path, {
    method,
    headers,
    credentials: "same-origin",
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  });
  let data = null;
  try { data = await res.json(); } catch (_) { /* empty body */ }
  if (!res.ok) {
    const err = new Error((data && data.error && data.error.message) || `HTTP ${res.status}`);
    err.status = res.status;
    err.detail = data && data.error;
    throw err;
  }
  return data;
}

let toastTimer;
function toast(msg, isErr = false) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.toggle("err", !!isErr);
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), 4000);
}

function show(view) {
  $("#view-auth").classList.toggle("hidden", view !== "auth");
  $("#view-app").classList.toggle("hidden", view !== "app");
}

// ---- auth ------------------------------------------------------------------
async function login(email, password) {
  const data = await api("/auth/login", { method: "POST", body: { email, password } });
  state.csrf = data.csrf_token;
  state.accessToken = data.access_token;
  state.user = data.user;
  enterApp();
}

async function logout() {
  try { await api("/auth/logout", { method: "POST" }); } catch (_) {}
  state.user = state.csrf = state.accessToken = null;
  show("auth");
}

function enterApp() {
  $("#whoami").textContent = `${state.user.full_name} (${state.user.email})`;
  document.querySelectorAll(".admin-only").forEach((el) =>
    el.classList.toggle("hidden", state.user.role !== "administrator"));
  show("app");
  if (state.user.must_change_password) {
    toast("Temporary password detected — please change it now.", true);
    openPasswordDialog(true);
  }
  switchTab("map");
  loadFriendLocations();
  loadFriendships();
  loadShares();
  if (state.user.role === "administrator") loadUsers();
}

// ---- tabs ------------------------------------------------------------------
function switchTab(name) {
  document.querySelectorAll(".tab").forEach((b) => b.classList.toggle("active", b.dataset.tab === name));
  ["map", "friends", "shares", "admin"].forEach((t) => $(`#tab-${t}`).classList.toggle("hidden", t !== name));
}

// ---- map tab ---------------------------------------------------------------
function freshness(updatedAt) {
  const mins = (Date.now() - new Date(updatedAt).getTime()) / 60000;
  if (!Number.isFinite(mins) || mins > 30) return { label: "STALE — do not rely on this position", cls: "fresh-stale" };
  if (mins > 10) return { label: `updated ${Math.round(mins)} min ago`, cls: "fresh-warn" };
  return { label: `updated ${Math.max(0, Math.round(mins))} min ago`, cls: "fresh-ok" };
}

async function loadFriendLocations() {
  try {
    const locs = await api("/locations/friends");
    renderMap(locs);
    $("#map-status").textContent = locs.length
      ? `${locs.length} friend location(s) visible to you (consent-based).`
      : "No friends are sharing their location with you yet.";
  } catch (e) { $("#map-status").textContent = e.message; }
}

function project(lat, lon, bounds, w, h, pad = 40) {
  const x = pad + ((lon - bounds.minLon) / (bounds.maxLon - bounds.minLon || 1)) * (w - 2 * pad);
  const y = pad + ((bounds.maxLat - lat) / (bounds.maxLat - bounds.minLat || 1)) * (h - 2 * pad);
  return { x, y };
}

function renderMap(locs) {
  const map = $("#map");
  map.innerHTML = "";
  map.style.position = "relative";
  map.style.overflow = "hidden";
  const W = map.clientWidth || 800, H = map.clientHeight || 420;

  // Background tiles around centroid of all points (OpenStreetMap raster).
  const pts = locs.length ? locs : [{ latitude: 48.8566, longitude: 2.3522 }];
  const cLat = pts.reduce((a, p) => a + p.latitude, 0) / pts.length;
  const cLon = pts.reduce((a, p) => a + p.longitude, 0) / pts.length;
  const z = 11;
  const n = 2 ** z;
  const latRad = (lat) => Math.log(Math.tan(Math.PI / 4 + (lat * Math.PI) / 360));
  const px = ((cLon + 180) / 360) * n * 256;
  const py = ((1 - latRad(cLat) / Math.PI) / 2) * n * 256;
  const tx0 = Math.floor(px / 256), ty0 = Math.floor(py / 256);
  const cols = Math.ceil(W / 256) + 1, rows = Math.ceil(H / 256) + 1;
  const ox = (px / 256 - tx0) * 256, oy = (py / 256 - ty0) * 256;
  for (let i = 0; i < cols; i++) {
    for (let j = 0; j < rows; j++) {
      const img = document.createElement("img");
      img.src = `https://maps.wikimedia.org/osm-intl/${z}/${tx0 - 1 + i}/${ty0 - 1 + j}.png`;
      img.alt = "";
      img.loading = "lazy";
      img.style.cssText = `position:absolute;width:256px;height:256px;left:${i * 256 - ox}px;top:${j * 256 - oy}px;filter:brightness(.75) saturate(.8);`;
      map.appendChild(img);
    }
  }

  const lats = locs.map((l) => l.latitude), lons = locs.map((l) => l.longitude);
  const bounds = locs.length
    ? { minLat: Math.min(...lats), maxLat: Math.max(...lats), minLon: Math.min(...lons), maxLon: Math.max(...lons) }
    : { minLat: cLat - 0.05, maxLat: cLat + 0.05, minLon: cLon - 0.05, maxLon: cLon + 0.05 };
  if (locs.length === 1) { bounds.minLat -= 0.02; bounds.maxLat += 0.02; bounds.minLon -= 0.02; bounds.maxLon += 0.02; }

  const list = $("#friend-list");
  list.innerHTML = "";
  locs.forEach((loc) => {
    const digits = loc.precision_level === "exact" ? 4 : 2;
    const { x, y } = project(loc.latitude, loc.longitude, bounds, W, H);

    const pin = document.createElement("div");
    pin.title = `${loc.user_name}: ${loc.latitude.toFixed(digits)}, ${loc.longitude.toFixed(digits)}`;
    pin.textContent = "📍";
    pin.style.cssText = `position:absolute;left:${x - 12}px;top:${y - 24}px;font-size:26px;z-index:5;text-shadow:0 0 6px #000;`;
    map.appendChild(pin);

    const nameTag = document.createElement("span");
    nameTag.textContent = loc.user_name.split(" ")[0];
    nameTag.style.cssText = `position:absolute;left:${x - 20}px;top:${y + 6}px;font-size:.75rem;background:#1e293bd0;padding:1px 6px;border-radius:8px;z-index:5;`;
    map.appendChild(nameTag);

    const fr = freshness(loc.updated_at);
    const li = document.createElement("li");
    li.innerHTML = `<strong>${esc(loc.user_name)}</strong>
      <span class="badge ${esc(loc.precision_level)}">${esc(loc.precision_level)}</span>
      <div class="${fr.cls}">${esc(fr.label)}</div>
      <div class="muted">${loc.latitude.toFixed(digits)}, ${loc.longitude.toFixed(digits)}${loc.accuracy != null ? ` ±${Math.round(loc.accuracy)} m` : ""}</div>
      <div class="row" style="margin-top:.4rem">
        <a class="btn" target="_blank" rel="noopener" href="https://www.openstreetmap.org/directions?to=${loc.latitude}%2C${loc.longitude}">Navigate ↗</a>
      </div>`;
    list.appendChild(li);
  });
}

async function publishLocation() {
  let lat = parseFloat($("#loc-lat").value), lon = parseFloat($("#loc-lon").value);
  if (Number.isNaN(lat) || Number.isNaN(lon)) {
    if (!navigator.geolocation) { toast("Enter coordinates manually.", true); return; }
    try {
      const pos = await new Promise((res, rej) => navigator.geolocation.getCurrentPosition(res, rej, { timeout: 8000 }));
      lat = pos.coords.latitude; lon = pos.coords.longitude;
    } catch (_) { toast("Enter valid lat/lon manually.", true); return; }
  }
  try {
    const r = await api("/locations/me", { method: "POST", body: { latitude: lat, longitude: lon, accuracy: 10 } });
    toast(`Location shared (${r.shares_updated} active share(s)).`);
    loadFriendLocations();
  } catch (e) { toast(e.message, true); }
}

// ---- friends tab -------------------------------------------------------------
async function loadFriendships() {
  try {
    state.friendships = await api("/friendships/");
    renderFriendships();
  } catch (e) { $("#friends-msg").textContent = e.message; }
}

const mkBtn = (text, fn) => { const b = document.createElement("button"); b.textContent = text; b.onclick = fn; return b; };

function renderFriendships() {
  const ul = $("#friendship-list");
  ul.innerHTML = "";
  if (!state.friendships.length) { ul.innerHTML = '<li class="muted">No friendships yet.</li>'; return; }
  state.friendships.forEach((f) => {
    const other = f.requester_id === state.user.id ? `with ${f.receiver_name}` : `from ${f.requester_name}`;
    const li = document.createElement("li");
    li.innerHTML = `<strong>${esc(other)}</strong><span class="badge">${esc(f.status)}</span>`;
    if (f.status === "pending" && f.receiver_id === state.user.id) {
      const accept = mkBtn("Accept", () => respond(f.id, "accept"));
      const reject = mkBtn("Reject", () => respond(f.id, "reject"));
      accept.className = "btn primary"; reject.className = "btn danger";
      accept.style.marginLeft = reject.style.marginLeft = ".5rem";
      li.appendChild(accept); li.appendChild(reject);
    }
    ul.appendChild(li);
  });
}

async function respond(id, action) {
  try { await api(`/friendships/${id}/respond`, { method: "POST", body: { action } }); loadFriendships(); loadShares(); }
  catch (e) { toast(e.message, true); }
}

async function sendFriendRequest(email) {
  try { await api("/friendships/", { method: "POST", body: { receiver_email: email } }); toast("Friend request sent."); loadFriendships(); }
  catch (e) { toast(e.message, true); }
}

// ---- sharing tab ---------------------------------------------------------------
async function loadShares() {
  try {
    state.shares = await api("/locations/shares");
    renderShares();
    renderShareSelect();
  } catch (e) { $("#shares-msg").textContent = e.message; }
}

function renderShareSelect() {
  const sel = $("#share-friendship");
  sel.innerHTML = "";
  state.friendships.filter((f) => f.status === "accepted").forEach((f) => {
    const other = f.requester_id === state.user.id ? f.receiver_name : f.requester_name;
    const o = document.createElement("option");
    o.value = f.id; o.textContent = `#${f.id} — ${other}`;
    sel.appendChild(o);
  });
  if (!sel.options.length) {
    const o = document.createElement("option");
    o.value = ""; o.textContent = "no accepted friendships";
    sel.appendChild(o);
  }
}

function renderShares() {
  const ul = $("#share-list");
  ul.innerHTML = "";
  if (!state.shares.length) { ul.innerHTML = '<li class="muted">You have not granted any location shares yet.</li>'; return; }
  state.shares.forEach((s) => {
    const li = document.createElement("li");
    li.innerHTML = `<strong>Share #${s.id}</strong> · friendship #${s.friendship_id}
      <span class="badge">${esc(s.direction)}</span>
      <span class="badge ${esc(s.precision_level)}">${esc(s.precision_level)}</span>
      <span class="badge">${s.is_active ? "ACTIVE" : "PAUSED"}</span>
      <div class="muted">last update: ${s.last_location_update_at ? new Date(s.last_location_update_at).toLocaleString() : "never"}</div>`;
    const toggle = mkBtn(s.is_active ? "Pause" : "Resume", async () => {
      try { await api(`/locations/shares/${s.id}`, { method: "PATCH", body: { is_active: !s.is_active } }); loadShares(); }
      catch (e) { toast(e.message, true); }
    });
    toggle.className = "btn"; toggle.style.marginTop = ".5rem";
    li.appendChild(toggle);
    ul.appendChild(li);
  });
}

async function createShare(body) {
  try { await api("/locations/shares", { method: "POST", body }); toast("Share created."); loadShares(); }
  catch (e) { toast(e.message, true); }
}

// ---- admin tab -----------------------------------------------------------------
async function loadUsers() {
  try {
    const users = await api("/users/");
    const tbody = $("#users-table tbody");
    tbody.innerHTML = "";
    users.forEach((u) => {
      const tr = document.createElement("tr");
      tr.innerHTML = `<td>${u.id}</td><td>${esc(u.email)}</td><td>${esc(u.full_name)}</td><td>${esc(u.role)}</td><td>${u.is_active ? "✔" : "✖"}</td>`;
      tbody.appendChild(tr);
    });
  } catch (e) { toast(e.message, true); }
}

// ---- password dialog -------------------------------------------------------------
function openPasswordDialog(forced) {
  $("#pw-form").dataset.forced = forced ? "1" : "0";
  $("#pw-current").closest("label").style.display = forced ? "none" : "";
  $("#pw-error").classList.add("hidden");
  $("#pw-dialog").showModal();
}

async function submitPasswordChange(evt) {
  evt.preventDefault();
  const forced = $("#pw-form").dataset.forced === "1";
  const newPassword = $("#pw-new").value, confirmPassword = $("#pw-confirm").value;
  const currentPassword = $("#pw-current").value;
  const errEl = $("#pw-error");
  errEl.classList.add("hidden");
  try {
    if (forced) {
      await api("/auth/password/force-change", { method: "POST", body: { new_password: newPassword, confirm_password: confirmPassword } });
    } else {
      await api("/auth/password/change", { method: "POST", body: { current_password: currentPassword, new_password: newPassword, confirm_password: confirmPassword } });
    }
    $("#pw-dialog").close();
    toast("Password changed. All sessions revoked — signing you back in…");
    await reloginAfterPasswordChange();
  } catch (e) {
    errEl.textContent = e.detail && e.detail.fields ? `${e.message} (${JSON.stringify(e.detail.fields)})` : e.message;
    errEl.classList.remove("hidden");
  }
}

// Changing the password revokes every server-side session, so the browser
// must log in again with the new credentials.
async function reloginAfterPasswordChange() {
  const email = state.user ? state.user.email : $("#login-email").value;
  const pwEl = $("#pw-new");
  try {
    await login(email, pwEl.value);
    pwEl.value = ""; $("#pw-confirm").value = ""; $("#pw-current").value = "";
  } catch (_) {
    show("auth");
  }
}

// ---- wire-up ---------------------------------------------------------------------
$("#login-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const btn = $("#login-btn");
  btn.disabled = true;
  $("#login-error").classList.add("hidden");
  try {
    await login($("#login-email").value.trim(), $("#login-password").value);
  } catch (err) {
    const el = $("#login-error");
    el.textContent = err.message; el.classList.remove("hidden");
  } finally { btn.disabled = false; }
});

$("#logout-btn").addEventListener("click", logout);
$("#change-pw-btn").addEventListener("click", () => openPasswordDialog(false));
$("#refresh-map").addEventListener("click", loadFriendLocations);
$("#publish-loc").addEventListener("click", publishLocation);
$("#friend-request-form").addEventListener("submit", (e) => {
  e.preventDefault();
  sendFriendRequest($("#friend-email").value.trim());
  $("#friend-email").value = "";
});
$("#share-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const fid = parseInt($("#share-friendship").value, 10);
  if (!fid) { toast("Create an accepted friendship first.", true); return; }
  createShare({ friendship_id: fid, direction: $("#share-direction").value, precision_level: $("#share-precision").value });
});
$("#create-user-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  try {
    const r = await api("/users/", { method: "POST", body: {
      email: $("#new-email").value.trim(), full_name: $("#new-name").value.trim(),
      temporary_password: $("#new-pass").value, role: $("#new-role").value } });
    toast(r.temporary_password ? `User created (temp pw: ${r.temporary_password})` : "User created.");
    loadUsers();
  } catch (err) { toast(err.message, true); }
});
document.querySelectorAll(".tab").forEach((b) => b.addEventListener("click", () => switchTab(b.dataset.tab)));
$("#pw-form").addEventListener("submit", submitPasswordChange);

// ---- boot -------------------------------------------------------------------------
(async function boot() {
  // A page reload loses the in-memory CSRF token, so cookies alone cannot
  // authorize writes. Start at the login screen; the backend rate limiter
  // and lockout still protect it.
  show("auth");
})();
