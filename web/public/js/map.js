// ---------------------------------------------------------------------------
// Map rendering: raster tiles from the configured provider + friend pins.
//
// Compliance (see THIRD_PARTY_NOTICES.md):
//  - Tiles come from config.js `tiles.url` (default: Wikimedia Foundation,
//    which welcomes app usage; NOT tile.openstreetmap.org — OSMF's Tile
//    Usage Policy prohibits serving their infrastructure to apps).
//  - ODbL attribution is rendered permanently below the map; removing it
//    would violate the OpenStreetMap license.
// ---------------------------------------------------------------------------
import { tiles, freshness as freshnessConfig } from "./config.js";
import { $, esc } from "./ui.js";

export function freshness(updatedAt) {
  const mins = (Date.now() - new Date(updatedAt).getTime()) / 60000;
  if (!Number.isFinite(mins) || mins > freshnessConfig.staleMinutes) {
    return { label: "STALE — do not rely on this position", cls: "fresh-stale" };
  }
  if (mins > freshnessConfig.warnMinutes) {
    return { label: `updated ${Math.round(mins)} min ago`, cls: "fresh-warn" };
  }
  return { label: `updated ${Math.max(0, Math.round(mins))} min ago`, cls: "fresh-ok" };
}

function project(lat, lon, bounds, w, h, pad = 40) {
  const x = pad + ((lon - bounds.minLon) / (bounds.maxLon - bounds.minLon || 1)) * (w - 2 * pad);
  const y = pad + ((bounds.maxLat - lat) / (bounds.maxLat - bounds.minLat || 1)) * (h - 2 * pad);
  return { x, y };
}

/** Renders friend locations as pins over raster tiles; updates status text. */
export function renderMap(locs) {
  const map = $("#map");
  map.innerHTML = "";
  map.style.position = "relative";
  map.style.overflow = "hidden";
  const W = map.clientWidth || 800, H = map.clientHeight || 420;

  // Background tiles around centroid of all points.
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
      img.src = tiles.url
        .replace("{z}", z)
        .replace("{x}", tx0 - 1 + i)
        .replace("{y}", ty0 - 1 + j);
      img.alt = "";
      img.loading = "lazy";
      img.style.cssText = `position:absolute;width:256px;height:256px;left:${i * 256 - ox}px;top:${j * 256 - oy}px;filter:brightness(.75) saturate(.8);`;
      map.appendChild(img);
    }
  }

  // Permanent ODbL attribution (legal requirement, not optional).
  $("#map-attribution").innerHTML = tiles.attribution;

  const lats = locs.map((l) => l.latitude), lons = locs.map((l) => l.longitude);
  const bounds = locs.length
    ? { minLat: Math.min(...lats), maxLat: Math.max(...lats), minLon: Math.min(...lons), maxLon: Math.max(...lons) }
    : { minLat: cLat - 0.05, maxLat: cLat + 0.05, minLon: cLon - 0.05, maxLon: cLon + 0.05 };
  if (locs.length === 1) {
    bounds.minLat -= 0.02; bounds.maxLat += 0.02;
    bounds.minLon -= 0.02; bounds.maxLon += 0.02;
  }

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
        <a class="btn" target="_blank" rel="noopener" href="geo:${loc.latitude},${loc.longitude}?q=${loc.latitude}%2C${loc.longitude}">Navigate here</a>
        <a class="btn" target="_blank" rel="noopener" href="https://www.openstreetmap.org/directions?to=${loc.latitude}%2C${loc.longitude}">Open OSM ↗</a>
      </div>`;
    list.appendChild(li);
  });
}
