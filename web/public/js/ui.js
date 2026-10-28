// ---------------------------------------------------------------------------
// DOM helpers: escaping, toasts, view switching, offline banner.
// No innerHTML with untrusted data — every interpolation goes through esc().
// ---------------------------------------------------------------------------

export const $ = (sel) => document.querySelector(sel);

const ESC_MAP = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
export const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) => ESC_MAP[c]);

let toastTimer;
export function toast(msg, isErr = false) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.toggle("err", !!isErr);
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), 4000);
}

export function show(view) {
  $("#view-auth").classList.toggle("hidden", view !== "auth");
  $("#view-app").classList.toggle("hidden", view !== "app");
}

export function switchTab(name) {
  document.querySelectorAll(".tab").forEach((b) =>
    b.classList.toggle("active", b.dataset.tab === name));
  ["map", "friends", "shares", "admin"].forEach((t) =>
    $(`#tab-${t}`).classList.toggle("hidden", t !== name));
}

export function mkBtn(text, onClick, className = "btn") {
  const b = document.createElement("button");
  b.textContent = text;
  b.className = className;
  b.addEventListener("click", onClick);
  return b;
}

// ---- offline state ---------------------------------------------------------
export function watchOnlineStatus(onChange) {
  const update = () => {
    $("#offline-banner").classList.toggle("hidden", navigator.onLine);
    onChange(navigator.onLine);
  };
  window.addEventListener("online", update);
  window.addEventListener("offline", update);
  update();
}
