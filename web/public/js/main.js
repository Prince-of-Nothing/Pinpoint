// ---------------------------------------------------------------------------
// Always Together — web client entry point.
// Modular vanilla ES modules: no build step, no npm, zero third-party JS
// (0-tracker stance; see THIRD_PARTY_NOTICES.md).
// ---------------------------------------------------------------------------
import { api, state } from "./api.js";
import { login, logout, restoreSession } from "./session.js";
import { renderMap } from "./map.js";
import { $, esc, toast, show, switchTab, mkBtn, watchOnlineStatus } from "./ui.js";
import { SW_VERSION } from "./config.js";

// ---- map tab ---------------------------------------------------------------
async function loadFriendLocations() {
  try {
    const locs = await api("/locations/friends");
    renderMap(locs);
    $("#map-status").textContent = locs.length
      ? `${locs.length} friend location(s) visible to you (consent-based).`
      : "No friends are sharing their location with you yet.";
  } catch (e) {
    $("#map-status").textContent = e.message;
  }
}

async function publishLocation() {
  let lat = parseFloat($("#loc-lat").value), lon = parseFloat($("#loc-lon").value);
  if (Number.isNaN(lat) || Number.isNaN(lon)) {
    if (!navigator.geolocation) { toast("Enter coordinates manually.", true); return; }
    try {
      const pos = await new Promise((res, rej) =>
        navigator.geolocation.getCurrentPosition(res, rej, { timeout: 8000 }));
      lat = pos.coords.latitude; lon = pos.coords.longitude;
    } catch (_) { toast("Enter valid lat/lon manually.", true); return; }
  }
  try {
    const r = await api("/locations/me", {
      method: "POST",
      body: { latitude: lat, longitude: lon, accuracy: 10 },
    });
    toast(`Location shared (${r.shares_updated} active share(s)).`);
    loadFriendLocations();
  } catch (e) { toast(e.message, true); }
}

// ---- friends tab -----------------------------------------------------------
async function loadFriendships() {
  try {
    state.friendships = await api("/friendships/");
    renderFriendships();
  } catch (e) { $("#friends-msg").textContent = e.message; }
}

function renderFriendships() {
  const ul = $("#friendship-list");
  ul.innerHTML = "";
  if (!state.friendships.length) {
    ul.innerHTML = '<li class="muted">No friendships yet.</li>';
    return;
  }
  state.friendships.forEach((f) => {
    const other = f.requester_id === state.user.id ? `with ${f.receiver_name}` : `from ${f.requester_name}`;
    const li = document.createElement("li");
    li.innerHTML = `<strong>${esc(other)}</strong><span class="badge">${esc(f.status)}</span>`;
    if (f.status === "pending" && f.receiver_id === state.user.id) {
      const accept = mkBtn("Accept", () => respond(f.id, "accept"), "btn primary");
      const reject = mkBtn("Reject", () => respond(f.id, "reject"), "btn danger");
      accept.style.marginLeft = reject.style.marginLeft = ".5rem";
      li.appendChild(accept);
      li.appendChild(reject);
    }
    ul.appendChild(li);
  });
}

async function respond(id, action) {
  try {
    await api(`/friendships/${id}/respond`, { method: "POST", body: { action } });
    loadFriendships();
    loadShares();
  } catch (e) { toast(e.message, true); }
}

async function sendFriendRequest(email) {
  try {
    await api("/friendships/", { method: "POST", body: { receiver_email: email } });
    toast("Friend request sent.");
    loadFriendships();
  } catch (e) { toast(e.message, true); }
}

// ---- sharing tab -----------------------------------------------------------
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
    o.value = f.id;
    o.textContent = `#${f.id} — ${other}`;
    sel.appendChild(o);
  });
  if (!sel.options.length) {
    const o = document.createElement("option");
    o.value = "";
    o.textContent = "no accepted friendships";
    sel.appendChild(o);
  }
}

function renderShares() {
  const ul = $("#share-list");
  ul.innerHTML = "";
  if (!state.shares.length) {
    ul.innerHTML = '<li class="muted">You have not granted any location shares yet.</li>';
    return;
  }
  state.shares.forEach((s) => {
    const li = document.createElement("li");
    li.innerHTML = `<strong>Share #${s.id}</strong> · friendship #${s.friendship_id}
      <span class="badge">${esc(s.direction)}</span>
      <span class="badge ${esc(s.precision_level)}">${esc(s.precision_level)}</span>
      <span class="badge">${s.is_active ? "ACTIVE" : "PAUSED"}</span>
      <div class="muted">last update: ${s.last_location_update_at ? new Date(s.last_location_update_at).toLocaleString() : "never"}</div>`;
    li.appendChild(
      mkBtn(s.is_active ? "Pause" : "Resume", async () => {
        try {
          await api(`/locations/shares/${s.id}`, { method: "PATCH", body: { is_active: !s.is_active } });
          loadShares();
        } catch (e) { toast(e.message, true); }
      }),
    );
    ul.appendChild(li);
  });
}

async function createShare(body) {
  try {
    await api("/locations/shares", { method: "POST", body });
    toast("Share created.");
    loadShares();
  } catch (e) { toast(e.message, true); }
}

// ---- admin tab ---------------------------------------------------------------
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
      await api("/auth/password/force-change", {
        method: "POST",
        body: { new_password: newPassword, confirm_password: confirmPassword },
      });
    } else {
      await api("/auth/password/change", {
        method: "POST",
        body: { current_password: currentPassword, new_password: newPassword, confirm_password: confirmPassword },
      });
    }
    $("#pw-dialog").close();
    toast("Password changed. All sessions revoked — signing you back in…");
    await reloginAfterPasswordChange();
  } catch (e) {
    errEl.textContent = e.detail && e.detail.fields
      ? `${e.message} (${JSON.stringify(e.detail.fields)})`
      : e.message;
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
    enterApp();
  } catch (_) {
    show("auth");
  }
}

// ---- app shell -----------------------------------------------------------------
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

// ---- wire-up ---------------------------------------------------------------------
function wireUp() {
  $("#login-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const btn = $("#login-btn");
    btn.disabled = true;
    $("#login-error").classList.add("hidden");
    try {
      await login($("#login-email").value.trim(), $("#login-password").value);
      enterApp();
    } catch (err) {
      const el = $("#login-error");
      el.textContent = err.message;
      el.classList.remove("hidden");
    } finally { btn.disabled = false; }
  });

  $("#logout-btn").addEventListener("click", async () => {
    await logout();
    show("auth");
  });
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
    createShare({
      friendship_id: fid,
      direction: $("#share-direction").value,
      precision_level: $("#share-precision").value,
    });
  });
  $("#create-user-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    try {
      const r = await api("/users/", {
        method: "POST",
        body: {
          email: $("#new-email").value.trim(),
          full_name: $("#new-name").value.trim(),
          temporary_password: $("#new-pass").value,
          role: $("#new-role").value,
        },
      });
      toast(r.temporary_password ? `User created (temp pw: ${r.temporary_password})` : "User created.");
      loadUsers();
    } catch (err) { toast(err.message, true); }
  });
  document.querySelectorAll(".tab").forEach((b) =>
    b.addEventListener("click", () => switchTab(b.dataset.tab)));
  $("#pw-form").addEventListener("submit", submitPasswordChange);
}

// ---- boot -------------------------------------------------------------------------
async function boot() {
  wireUp();
  watchOnlineStatus(() => {});

  // Offline-capable shell: register the service worker (app cache only —
  // API responses and location data are NEVER cached, see sw.js).
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register(`sw.js?v=${SW_VERSION}`).catch(() => {
      /* private mode / disabled — app still works online */
    });
  }

  // Session restore: the HttpOnly cookie survives reloads; validate it
  // server-side instead of forcing a login screen.
  const user = await restoreSession();
  if (user) {
    enterApp();
  } else {
    show("auth");
  }
}

boot();
