// Per-friend sharing controls: one sheet over /api/v1/locations/shares.
// Directional: this edits only what I share with the selected friend.

const $ = (id) => document.getElementById(id);

const PRECISION_LABELS = {
  exact: "Exact",
  approximate: "Approximate",
  city_only: "City only",
};

const PRECISION_RANK = { city_only: 0, approximate: 1, exact: 2 };

export function friendName(friendship, me) {
  if (!friendship) return "this friend";
  const name =
    friendship.requester_id === me ? friendship.receiver_name : friendship.requester_name;
  return name || `friendship #${friendship.id}`;
}

export function precisionLabel(level) {
  return PRECISION_LABELS[level] || String(level || "Unknown");
}

function expiryDate(share) {
  if (!share?.expires_at) return null;
  const date = new Date(share.expires_at);
  return Number.isNaN(date.getTime()) ? null : date;
}

function requestedExpiry(current, form, now = Date.now()) {
  if (form?.duration === "keep") return expiryDate(current);
  const hours = Number(form?.duration);
  if (!Number.isFinite(hours)) return undefined;
  if (hours <= 0) return null;
  const at = now instanceof Date ? now.getTime() : Number(now);
  return new Date(at + hours * 3_600_000);
}

export function shareStatus(share, now = Date.now()) {
  if (!share) return "paused";
  const expiry = expiryDate(share);
  const at = now instanceof Date ? now.getTime() : Number(now);
  if (expiry && expiry.getTime() <= at) return "expired";
  return share.is_active ? "active" : "paused";
}

export function expiryText(share, now = Date.now()) {
  if (!share?.expires_at) return "no expiry";
  const expiry = expiryDate(share);
  if (!expiry) return "expiry unknown";
  const at = now instanceof Date ? now.getTime() : Number(now);
  return `${expiry.getTime() <= at ? "expired" : "expires"} ${expiry.toLocaleString()}`;
}

/** True when the requested form grants more than the stored share. */
export function shareWidens(current, form, now = Date.now()) {
  if (!current) return false;
  const currentRank = PRECISION_RANK[current.precision_level] ?? PRECISION_RANK.exact;
  const requestedRank = PRECISION_RANK[form?.precision] ?? currentRank;
  if (requestedRank > currentRank) return true;
  const stored = expiryDate(current);
  if (!stored) return false;
  const requested = requestedExpiry(current, form, now);
  if (requested === undefined) return false;
  if (requested === null) return true;
  return requested.getTime() > stored.getTime();
}

/**
 * The PATCH/POST body for the sheet form. Pausing sends only `is_active`, so
 * precision and expiry are never rewritten by a pause; a null result means
 * there is nothing to send.
 */
export function shareRequestBody(current, form) {
  if (!form?.enabled) {
    if (!current || shareStatus(current) === "expired") return null;
    return { is_active: false };
  }
  const body = {
    friendship_id: form.friendshipId,
    direction: current?.direction || "to_friend",
    precision_level: form.precision,
    is_active: true,
  };
  if (form.duration === "keep") {
    if (current?.expires_at) body.expires_at = current.expires_at;
    else body.duration_hours = 0;
  } else {
    body.duration_hours = Number(form.duration);
  }
  return body;
}

export function shareRowModel(share, now = Date.now()) {
  const status = shareStatus(share, now);
  return {
    isGrant: Boolean(share) && share.direction !== "from_friend",
    status,
    precision: precisionLabel(share?.precision_level),
    expiry: expiryText(share, now),
    canPauseResume: status !== "expired",
    canShareAgain: status === "expired",
  };
}

export function initSharing({ api, setStatus, getUser, onChanged }) {
  const sheet = $("share-sheet");
  let friendship = null;
  let current = null;

  function selectedExpiry() {
    const value = $("share-duration").value;
    if (value === "keep") return expiryDate(current);
    const hours = Number(value);
    return hours > 0 ? new Date(Date.now() + hours * 3_600_000) : null;
  }

  function summary() {
    if (!friendship) return "";
    const until = selectedExpiry();
    return `${precisionLabel($("share-precision").value)} location · ${friendName(
      friendship,
      getUser?.()?.id,
    )} · ${until ? `until ${until.toLocaleString()}` : "no expiry"}`;
  }

  function confirmText() {
    const until = selectedExpiry();
    const precision = precisionLabel($("share-precision").value).toLowerCase();
    const friend = friendName(friendship, getUser?.()?.id);
    return until
      ? `Share ${precision} location with ${friend} until ${until.toLocaleString()}?`
      : `Share ${precision} location with ${friend} until you turn it off?`;
  }

  function refreshSummary() {
    $("share-summary").textContent = summary();
  }

  function syncDuration() {
    const select = $("share-duration");
    select.querySelector('option[value="keep"]')?.remove();
    const expiry = expiryDate(current);
    if (expiry && shareStatus(current) !== "expired") {
      const keep = document.createElement("option");
      keep.value = "keep";
      keep.textContent = `Keep current (until ${expiry.toLocaleString()})`;
      select.insertBefore(keep, select.firstChild);
      select.value = "keep";
    } else {
      select.value = !current || shareStatus(current) === "expired" ? "24" : "0";
    }
  }

  async function open(f) {
    friendship = f;
    current = null;
    const me = getUser?.()?.id;
    const isOutgoing = f.requester_id === me;
    $("share-title").textContent = `Share with ${friendName(f, me)}`;
    $("share-subtitle").textContent = isOutgoing
      ? "You decide exactly what this friend can see."
      : "They control what they share with you. You control what you share back.";
    $("share-incoming").textContent =
      "What they share with you is controlled by them and appears on the map only while active.";
    try {
      const shares = await api("/api/v1/locations/shares");
      current =
        (Array.isArray(shares) ? shares : []).find(
          (s) =>
            s.friendship_id === f.id &&
            (s.direction === "to_friend" || s.direction === "mutual"),
        ) || null;
      $("share-enabled").checked = shareStatus(current) === "active";
      $("share-precision").value = current?.precision_level || "exact";
      syncDuration();
      $("share-revoke").classList.toggle("hidden", !current);
      sheet.classList.remove("hidden");
      $("place-sheet").classList.add("hidden");
      $("route-panel").classList.add("hidden");
      refreshSummary();
    } catch (err) {
      setStatus(err.message, true);
    }
  }

  async function save() {
    if (!friendship) return;
    const form = {
      friendshipId: friendship.id,
      enabled: $("share-enabled").checked,
      precision: $("share-precision").value,
      duration: $("share-duration").value,
    };
    const body = shareRequestBody(current, form);
    if (!body) {
      setStatus("Nothing was shared.");
      close();
      return;
    }
    const granting = body.is_active === true && (!current || shareStatus(current) !== "active");
    const widening = body.is_active === true && shareWidens(current, form);
    if ((granting || widening) && !window.confirm(confirmText())) return;

    try {
      if (current) {
        await api(`/api/v1/locations/shares/${current.id}`, { method: "PATCH", body });
        setStatus(body.is_active ? "Sharing updated." : "Sharing paused.");
      } else {
        await api("/api/v1/locations/shares", { method: "POST", body });
        setStatus("Sharing started.");
      }
      await onChanged?.();
      close();
    } catch (err) {
      setStatus(err.message, true);
    }
  }

  async function revoke() {
    if (!current) {
      close();
      return;
    }
    try {
      await api(`/api/v1/locations/shares/${current.id}`, { method: "DELETE" });
      setStatus("Sharing revoked.");
      await onChanged?.();
      close();
    } catch (err) {
      setStatus(err.message, true);
    }
  }

  function close() {
    sheet.classList.add("hidden");
    $("share-summary").textContent = "";
    friendship = null;
    current = null;
  }

  $("share-save").addEventListener("click", save);
  $("share-revoke").addEventListener("click", revoke);
  $("share-close").addEventListener("click", close);
  for (const id of ["share-enabled", "share-precision", "share-duration"]) {
    $(id).addEventListener("change", refreshSummary);
  }

  return { open, close };
}
