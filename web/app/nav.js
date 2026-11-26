// Search, place sheet and route panel wiring.
// All requests are same-origin; no user identity or friend data is sent to OSM.

import { getOfflineSearch, toSearchPlace } from "./offline-search.js";
import {
  combinedProvider,
  mapFeaturesProvider,
  offlinePackProvider,
  savedPlacesProvider,
  searchPlaces,
  serverSearchProvider,
} from "./services.js";

const $ = (id) => document.getElementById(id);
const MIN_QUERY = 3;

export function initNavigation({ mapApi, setStatus, getUserId }) {
  const form = $("search-form");
  const input = $("search-input");
  const results = $("search-results");
  const chips = $("category-chips");
  const placeSheet = $("place-sheet");
  const routePanel = $("route-panel");
  const savedList = $("saved-list");
  const recentList = $("recent-list");

  const capabilities = { autocomplete: false, modes: ["driving"], avoid: false };
  let searchController = null;
  let debounceTimer = null;
  let selected = null;
  let currentResults = [];
  let route = {
    origin: null,
    destination: null,
    mode: "driving",
    routes: [],
    index: 0,
    controller: null,
    watchId: null,
    via: [],
    avoid: [],
    addingStop: false,
    stopIndex: 0,
  };

  let savedFilter = "all";

  // --- per-account on-device storage (never leaves the browser) ------------
  function accountKey(kind) {
    const id = typeof getUserId === "function" ? getUserId() : "anon";
    return `pinpoint.v1.${id}.${kind}`;
  }

  function loadList(kind) {
    try {
      const raw = localStorage.getItem(accountKey(kind));
      const parsed = raw ? JSON.parse(raw) : [];
      return Array.isArray(parsed) ? parsed.filter(isPlaceRecord) : [];
    } catch (_) {
      return [];
    }
  }

  function saveList(kind, list) {
    try {
      localStorage.setItem(accountKey(kind), JSON.stringify(list.slice(0, 50)));
    } catch (_) {
      // quota/private mode: keep the app usable, just don't persist
    }
  }

  function isPlaceRecord(value) {
    return (
      value &&
      typeof value === "object" &&
      typeof value.display_name === "string" &&
      Number.isFinite(Number(value.latitude)) &&
      Number.isFinite(Number(value.longitude))
    );
  }

  function placeRecord(place) {
    return {
      display_name: String(place.display_name || ""),
      short_name: String(place.short_name || place.display_name || "Place"),
      latitude: Number(place.latitude),
      longitude: Number(place.longitude),
      category: String(place.category || ""),
    };
  }

  function samePlace(a, b) {
    return (
      a.display_name === b.display_name &&
      Math.abs(Number(a.latitude) - Number(b.latitude)) < 1e-6 &&
      Math.abs(Number(a.longitude) - Number(b.longitude)) < 1e-6
    );
  }

  // Place search asks these in order and falls through (services.js): the
  // offline pack, the Pinpoint server, then data already on this device.
  const placeProviders = [
    offlinePackProvider(getOfflineSearch, toSearchPlace),
    serverSearchProvider(),
    combinedProvider("device", "On this device", [
      savedPlacesProvider(() => ({ saved: loadList("savedPlaces"), recents: loadList("recents") })),
      mapFeaturesProvider(() => mapApi.getMap?.()),
    ]),
  ];

  function addRecent(place) {
    const record = placeRecord(place);
    if (!isPlaceRecord(record)) return;
    const recents = loadList("recents").filter((item) => !samePlace(item, record));
    recents.unshift(record);
    saveList("recents", recents);
  }

  function updateSaveButton() {
    const btn = $("place-save");
    if (!selected) return;
    const record = placeRecord(selected);
    const saved = loadList("savedPlaces").some((item) => samePlace(item, record));
    btn.textContent = saved ? "Saved ✓" : "Save";
    btn.setAttribute("aria-pressed", String(saved));
  }

  function renderStorage() {
    const saved = loadList("savedPlaces");
    const recents = loadList("recents");
    const categories = [...new Set(saved.map((place) => place.category || "place"))].sort();
    if (savedFilter !== "all" && !categories.includes(savedFilter)) savedFilter = "all";

    const filters = $("saved-filters");
    filters.innerHTML = "";
    for (const category of ["all", ...categories]) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = `saved-filter${category === savedFilter ? " active" : ""}`;
      btn.textContent = category === "all" ? "All" : category;
      btn.addEventListener("click", () => {
        savedFilter = category;
        renderStorage();
      });
      filters.appendChild(btn);
    }

    const visible = saved.filter(
      (place) => savedFilter === "all" || (place.category || "place") === savedFilter,
    );
    savedList.innerHTML = visible.length ? "" : '<li class="muted">No saved places.</li>';
    recentList.innerHTML = recents.length ? "" : '<li class="muted">No recent places.</li>';

    for (const place of visible) {
      savedList.appendChild(storageRow(place, "savedPlaces"));
    }
    for (const place of recents) {
      recentList.appendChild(storageRow(place, "recents"));
    }
    updateSaveButton();
    renderRouteHistory();
  }

  function storageRow(place, kind) {
    const li = document.createElement("li");
    const show = document.createElement("button");
    show.type = "button";
    show.className = "link";
    show.textContent = place.short_name || place.display_name || "Place";
    show.addEventListener("click", () => selectPlace(place));
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "mini";
    remove.setAttribute("aria-label", `Remove ${show.textContent}`);
    remove.textContent = "×";
    remove.addEventListener("click", () => {
      saveList(
        kind,
        loadList(kind).filter((item) => !samePlace(item, place)),
      );
      renderStorage();
    });
    const badge = document.createElement("span");
    badge.className = "badge";
    badge.textContent = place.category || "place";
    li.append(show, badge, remove);
    return li;
  }

  // --- route history (per account, on-device) ----------------------------
  function loadRoutes() {
    try {
      const raw = JSON.parse(localStorage.getItem(accountKey("routes")) || "[]");
      return Array.isArray(raw)
        ? raw.filter(
            (entry) =>
              entry?.from?.lat != null &&
              entry?.to?.lat != null &&
              Number.isFinite(Number(entry.from.lat)) &&
              Number.isFinite(Number(entry.to.lat)),
          )
        : [];
    } catch (_) {
      return [];
    }
  }

  function saveRoutes(routes) {
    try {
      localStorage.setItem(accountKey("routes"), JSON.stringify(routes.slice(0, 20)));
    } catch (_) {
      // quota/private mode
    }
  }

  function rememberRoute(chosen) {
    if (!route.origin || !route.destination) return;
    const record = {
      from: { ...route.origin },
      to: { ...route.destination },
      via: route.via.map((stop) => ({ ...stop })),
      mode: route.mode,
      distance_m: chosen.distance_m,
      duration_s: chosen.duration_s,
      at: Date.now(),
    };
    const same = (entry) =>
      entry.from.label === record.from.label &&
      entry.to.label === record.to.label &&
      entry.mode === record.mode &&
      (entry.via || []).length === record.via.length;
    saveRoutes([record, ...loadRoutes().filter((entry) => !same(entry))]);
    renderRouteHistory();
  }

  function renderRouteHistory() {
    const list = $("route-history-list");
    if (!list) return;
    const routes = loadRoutes();
    list.innerHTML = routes.length ? "" : '<li class="muted">No recent routes.</li>';
    for (const entry of routes) {
      const li = document.createElement("li");
      const open = document.createElement("button");
      open.type = "button";
      open.className = "link";
      open.textContent = `${entry.from.label || "A"} → ${entry.to.label || "B"} · ${formatDistance(entry.distance_m)} · ${formatDuration(entry.duration_s)}`;
      open.addEventListener("click", () => openRouteHistory(entry));
      const mode = document.createElement("span");
      mode.className = "badge";
      mode.textContent = entry.mode;
      li.append(open, mode);
      list.appendChild(li);
    }
  }

  function openRouteHistory(entry) {
    route.origin = { ...entry.from };
    route.destination = { ...entry.to };
    route.via = (entry.via || []).map((stop) => ({ ...stop }));
    route.mode = entry.mode || "driving";
    route.stopIndex = 0;
    renderRouteFields();
    document.querySelectorAll(".mode").forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.mode === route.mode);
    });
    showRoutePanel();
    planRoute();
  }

  fetch("/map/capabilities")
    .then((r) => (r.ok ? r.json() : null))
    .then((caps) => {
      if (!caps) return;
      capabilities.autocomplete = Boolean(caps.autocomplete);
      capabilities.modes = Array.isArray(caps.modes) ? caps.modes : ["driving"];
      capabilities.avoid = Boolean(caps.avoid);
      applyModeAvailability();
    })
    .catch(() => {});

  function applyModeAvailability() {
    document.querySelectorAll(".mode").forEach((btn) => {
      const mode = btn.dataset.mode;
      const enabled = capabilities.modes.includes(mode);
      btn.disabled = !enabled;
      btn.title = enabled
        ? ""
        : "Self-host foot/bike routing to enable this mode.";
    });
    document.querySelectorAll(".route-avoid input").forEach((input) => {
      input.disabled = !capabilities.avoid;
      input.title = capabilities.avoid
        ? ""
        : "Avoid options require a self-hosted OSRM instance.";
    });
  }

  function showResults(open) {
    results.classList.toggle("hidden", !open);
    chips.classList.toggle("hidden", !open || input.value.trim().length > 0);
  }

  function focusMap() {
    document.querySelector("#map .maplibregl-canvas")?.focus({ preventScroll: true });
  }

  function hidePlace() {
    const wasOpen = !placeSheet.classList.contains("hidden");
    placeSheet.classList.add("hidden");
    mapApi.setSelectedPlace(null);
    selected = null;
    if (wasOpen) focusMap();
  }

  function showPlace(place) {
    selected = place;
    $("place-name").textContent = place.short_name || place.display_name || "Place";
    $("place-address").textContent = place.display_name || "";
    $("place-distance").textContent = Number.isFinite(place.distance_m)
      ? `${formatDistance(place.distance_m)} from map centre`
      : "";
    placeSheet.classList.remove("hidden");
    mapApi.setSelectedPlace(place);
    $("place-close")?.focus({ preventScroll: true });
  }

  async function runSearch(query) {
    const q = query.trim();
    if (q.length < MIN_QUERY) {
      setStatus(`Type at least ${MIN_QUERY} characters.`, true);
      return;
    }
    if (searchController) searchController.abort();
    const controller = new AbortController();
    searchController = controller;
    const live = mapApi.map.getCenter();
    const center = { lat: live.lat, lon: live.lng };
    renderSearchNote("Searching…");
    try {
      const found = await searchPlaces(placeProviders, q, { center, limit: 8, signal: controller.signal });
      if (controller !== searchController) return;
      if (found.results.length) {
        renderResults(found.results, "", searchSourceNote(found));
        return;
      }
      if (found.error) {
        renderSearchError(`${searchErrorText(found.error)} Nothing saved or on the map matches “${q}” either.`, q);
        return;
      }
      renderResults([], `No places match “${q}”.`);
    } catch (err) {
      if (err.name === "AbortError" || controller !== searchController) return;
      renderSearchError(searchErrorText(err), q);
    }
  }

  // Say where results came from when it was not the usual source.
  function searchSourceNote({ source, error }) {
    if (source === "offline") return "From the offline map on this device.";
    if (source !== "device") return "";
    return error
      ? "Can't reach the Pinpoint server. Showing matches from your saved places and the map on screen."
      : "Matches from your saved places and the map on screen.";
  }

  // A failed request is not an empty result: say why, and offer a retry.
  function searchErrorText(err) {
    if (navigator.onLine === false) {
      return "You're offline. Search needs a connection unless the offline map is installed.";
    }
    if (err instanceof TypeError) {
      return "Can't reach the Pinpoint server. Check your connection and try again.";
    }
    return err?.message || "Search failed.";
  }

  function renderSearchNote(text, className = "result muted") {
    currentResults = [];
    results.innerHTML = "";
    const note = document.createElement("div");
    note.className = className;
    note.textContent = text;
    results.appendChild(note);
    showResults(true);
    return note;
  }

  function renderSearchError(text, query) {
    const note = renderSearchNote(text, "result search-error");
    note.setAttribute("role", "alert");
    const retry = document.createElement("button");
    retry.type = "button";
    retry.className = "secondary search-retry";
    retry.textContent = "Retry";
    retry.addEventListener("click", () => runSearch(query));
    note.appendChild(retry);
  }

  function renderResults(items, emptyText = "No places found.", note = "") {
    if (!items.length) {
      renderSearchNote(emptyText);
      return;
    }
    currentResults = items;
    results.innerHTML = "";
    if (note) {
      const row = document.createElement("div");
      row.className = "result muted search-source";
      row.textContent = note;
      results.appendChild(row);
    }
    for (const place of items) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "result";
      btn.dataset.index = String(currentResults.indexOf(place));
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = place.short_name || place.display_name || "Place";
      const meta = document.createElement("span");
      meta.className = "meta";
      const dist = Number.isFinite(place.distance_m) ? ` · ${formatDistance(place.distance_m)}` : "";
      meta.textContent = `${place.display_name || ""}${dist}`;
      btn.append(name, meta);
      results.appendChild(btn);
    }
    showResults(true);
  }

  function selectPlace(place, fly = true) {
    showPlace(place);
    showResults(false);
    addRecent(place);
    renderStorage();
    const lat = Number(place.latitude);
    const lon = Number(place.longitude);
    if (fly && Number.isFinite(lat) && Number.isFinite(lon)) {
      mapApi.map.flyTo([lat, lon], Math.max(mapApi.map.getZoom(), 15), { duration: 0.5 });
    }
  }

  async function reverseAt(lat, lon) {
    setStatus("Looking up this place…");
    try {
      const res = await fetch(
        `/map/reverse?lat=${lat.toFixed(5)}&lon=${lon.toFixed(5)}`,
        { credentials: "include" },
      );
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error?.message || "Could not look up this place.");
      selectPlace(body, false);
      setStatus("");
    } catch (err) {
      setStatus(err.message, true);
    }
  }

  function showRoutePanel() {
    routePanel.classList.remove("hidden");
    placeSheet.classList.add("hidden");
    $("route-close")?.focus({ preventScroll: true });
  }

  function openRouteFor(place) {
    route.origin = route.origin || centerOrigin();
    route.destination = {
      lat: Number(place.latitude),
      lon: Number(place.longitude),
      label: place.short_name || place.display_name || "Destination",
    };
    route.via = [];
    route.addingStop = false;
    route.stopIndex = 0;
    renderRouteFields();
    showRoutePanel();
    planRoute();
  }

  /** Set the route origin from a map point and open the route panel. */
  function routeFrom(point) {
    const lat = Number(point?.lat);
    const lon = Number(point?.lon);
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) return;
    route.origin = {
      lat,
      lon,
      label: `Dropped pin (${lat.toFixed(4)}, ${lon.toFixed(4)})`,
    };
    route.via = [];
    route.addingStop = false;
    route.stopIndex = 0;
    renderRouteFields();
    showRoutePanel();
    if (route.destination) planRoute();
    else setStatus("Origin set. Choose a destination.");
  }

  function centerOrigin() {
    const c = mapApi.map.getCenter();
    return { lat: c.lat, lon: c.lng, label: `Map centre (${c.lat.toFixed(4)}, ${c.lng.toFixed(4)})` };
  }

  function renderRouteFields() {
    $("route-from").value = route.origin?.label || "Choose origin";
    $("route-to").value = route.destination?.label || "Choose destination";
    const stops = $("route-stops");
    stops.innerHTML = "";
    route.via.forEach((stop, index) => {
      const row = document.createElement("div");
      row.className = "route-stop";
      const label = document.createElement("span");
      label.textContent = `Stop ${index + 1}: ${stop.lat.toFixed(4)}, ${stop.lon.toFixed(4)}`;
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "mini";
      remove.textContent = "×";
      remove.setAttribute("aria-label", `Remove stop ${index + 1}`);
      remove.addEventListener("click", () => {
        route.via.splice(index, 1);
        renderRouteFields();
        if (route.origin && route.destination) planRoute();
      });
      row.append(label, remove);
      stops.appendChild(row);
    });
  }

  async function planRoute() {
    if (!route.origin || !route.destination) return;
    if (route.controller) route.controller.abort();
    route.controller = new AbortController();
    $("route-summary").textContent = "Routing…";
    $("route-steps").innerHTML = "";
    $("route-alternatives").innerHTML = "";
    try {
      const params = new URLSearchParams({
        from_lat: route.origin.lat.toFixed(6),
        from_lon: route.origin.lon.toFixed(6),
        to_lat: route.destination.lat.toFixed(6),
        to_lon: route.destination.lon.toFixed(6),
        mode: route.mode,
        alternatives: "true",
      });
      if (route.via.length) {
        params.set(
          "via",
          route.via.map((stop) => `${stop.lat.toFixed(6)},${stop.lon.toFixed(6)}`).join(";"),
        );
      }
      if (route.avoid.length) params.set("avoid", route.avoid.join(","));
      const res = await fetch(`/map/route?${params}`, {
        signal: route.controller.signal,
        credentials: "include",
      });
      const body = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(body?.error?.message || "Routing failed.");
      route.routes = Array.isArray(body.routes) ? body.routes : [];
      if (!route.routes.length) throw new Error("No route found between these points.");
      route.index = 0;
      route.stopIndex = 0;
      rememberRoute(route.routes[0]);
      renderAlternatives();
      applyRoute(0);
    } catch (err) {
      if (err.name === "AbortError") return;
      $("route-summary").textContent = err.message;
      setStatus(err.message, true);
    }
  }

  function applyRoute(index) {
    const chosen = route.routes[index];
    if (!chosen) return;
    route.index = index;
    mapApi.drawRoute(chosen, route.via);
    mapApi.fitRoute(chosen);
    $("route-summary").textContent = `${formatDistance(chosen.distance_m)} · ${formatDuration(chosen.duration_s)} · ${route.mode}`;
    renderRouteProgress(chosen);
    const steps = $("route-steps");
    steps.innerHTML = "";
    for (const step of chosen.steps || []) {
      const li = document.createElement("li");
      const distance = Number.isFinite(step.distance_m) ? ` (${formatDistance(step.distance_m)})` : "";
      li.textContent = `${step.text}${distance}`;
      steps.appendChild(li);
    }
    document
      .querySelectorAll("#route-alternatives button")
      .forEach((btn, i) => btn.classList.toggle("active", i === index));
  }

  function renderRouteProgress(chosen) {
    const wrap = $("route-progress");
    const nodes = [{ label: "A" }];
    route.via.forEach((_, index) => nodes.push({ label: String(index + 1) }));
    nodes.push({ label: "B" });
    wrap.innerHTML = nodes
      .map(
        (node, index) =>
          `<span class="rp-node">${node.label}</span>${
            index < nodes.length - 1 ? '<span class="rp-line"></span>' : ""
          }`,
      )
      .join("");
    wrap.classList.remove("hidden");

    const nextIndex =
      route.stopIndex < route.via.length ? route.stopIndex + 1 : nodes.length - 1;
    wrap.querySelectorAll(".rp-node").forEach((node, index) => {
      node.classList.toggle("next", index === nextIndex);
      node.classList.toggle("done", index < nextIndex);
    });

    const next = $("route-next");
    if (route.stopIndex < route.via.length) {
      const leg = chosen.legs?.[route.stopIndex];
      next.textContent = leg
        ? `Next stop ${route.stopIndex + 1} · ${formatDistance(leg.distance_m)} · ${formatDuration(leg.duration_s)}`
        : `Next stop ${route.stopIndex + 1}`;
    } else {
      next.textContent = `Destination · ${formatDistance(chosen.distance_m)} · ${formatDuration(chosen.duration_s)}`;
    }
  }

  function haversineM(lat1, lon1, lat2, lon2) {
    const r = 6371000;
    const p1 = (lat1 * Math.PI) / 180;
    const p2 = (lat2 * Math.PI) / 180;
    const dp = ((lat2 - lat1) * Math.PI) / 180;
    const dl = ((lon2 - lon1) * Math.PI) / 180;
    const a = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
    return 2 * r * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
  }

  function updateLiveNext(fix) {
    const chosen = route.routes[route.index];
    if (!chosen) return;
    const target = route.via[route.stopIndex] || route.destination;
    if (!target) return;
    const distance = haversineM(fix.lat, fix.lon, Number(target.lat), Number(target.lon));
    if (route.via[route.stopIndex] && distance < 60) {
      route.stopIndex += 1;
      renderRouteProgress(chosen);
    }
    const activeTarget = route.via[route.stopIndex] || route.destination;
    if (!activeTarget) return;
    const remaining = haversineM(
      fix.lat,
      fix.lon,
      Number(activeTarget.lat),
      Number(activeTarget.lon),
    );
    const leg =
      route.stopIndex < route.via.length
        ? chosen.legs?.[route.stopIndex]
        : chosen.legs?.[chosen.legs.length - 1];
    const remainingSeconds =
      leg && leg.distance_m > 0 ? (leg.duration_s * remaining) / leg.distance_m : null;
    $("route-next").textContent =
      route.stopIndex < route.via.length
        ? `Next stop ${route.stopIndex + 1} · ${formatDistance(remaining)} · ~${formatDuration(remainingSeconds)}`
        : `Destination · ${formatDistance(remaining)} · ~${formatDuration(remainingSeconds)}`;
  }

  async function shareRoute() {
    const chosen = route.routes[route.index];
    if (!chosen) {
      setStatus("Plan a route first.", true);
      return;
    }
    const steps = (chosen.steps || [])
      .slice(0, 10)
      .map((step, index) => `${index + 1}. ${step.text}`)
      .join("\n");
    const google = googleRouteUrl();
    const text = `Pinpoint route: ${route.origin?.label || "A"} → ${route.destination?.label || "B"}\n${formatDistance(chosen.distance_m)} · ${formatDuration(chosen.duration_s)}\n${steps}${google ? `\n${google}` : ""}`;
    try {
      if (navigator.share) {
        await navigator.share({ title: "Pinpoint route", text });
      } else {
        await navigator.clipboard.writeText(text);
        setStatus("Route copied to clipboard.");
        return;
      }
      setStatus("Route shared.");
    } catch (err) {
      if (err?.name !== "AbortError") {
        setStatus("Clipboard blocked — use GPX or KML export instead.", true);
      }
    }
  }

  function placeName(place) {
    return place.short_name || place.display_name || "Place";
  }

  function googlePlaceUrl(place) {
    return `https://www.google.com/maps/search/?api=1&query=${Number(place.latitude)},${Number(place.longitude)}`;
  }

  function organicPlaceUrl(place) {
    const name = encodeURIComponent(placeName(place));
    return `https://omaps.app/map?v=1&ll=${Number(place.latitude)},${Number(place.longitude)}&n=${name}`;
  }

  // Magic Earth: magicearth://?show_on_map&lat=..&lon=..&name=..&zoom=..
  function magicPlaceUrl(place) {
    const name = encodeURIComponent(placeName(place));
    return `magicearth://?show_on_map&lat=${Number(place.latitude)}&lon=${Number(place.longitude)}&name=${name}&zoom=16`;
  }

  // Magic Earth routes support one destination only (no origin/waypoints).
  function magicRouteUrl() {
    if (!route.destination) return null;
    const mode = {
      driving: "drive_to",
      foot: "walk_to",
      bike: "bike_to",
    }[route.mode] || "drive_to";
    const android = /Android/i.test(navigator.userAgent);
    const flag = android ? `get_directions&${mode}` : mode;
    return `magicearth://?${flag}&lat=${Number(route.destination.lat)}&lon=${Number(route.destination.lon)}`;
  }

  function googleRouteUrl() {
    if (!route.origin || !route.destination) return null;
    const mode = { driving: "driving", foot: "walking", bike: "bicycling" }[route.mode] || "driving";
    const params = new URLSearchParams({
      api: "1",
      origin: `${route.origin.lat},${route.origin.lon}`,
      destination: `${route.destination.lat},${route.destination.lon}`,
      travelmode: mode,
    });
    if (route.via.length) {
      params.set(
        "waypoints",
        route.via.map((stop) => `${stop.lat},${stop.lon}`).join("|"),
      );
    }
    return `https://www.google.com/maps/dir/?${params}`;
  }

  function organicRouteUrl() {
    if (!route.origin || !route.destination) return null;
    const mode = { driving: "drive", foot: "pedestrian", bike: "bicycle" }[route.mode] || "drive";
    const parts = [
      `origin=${route.origin.lat},${route.origin.lon}`,
      `origin_name=${encodeURIComponent(route.origin.label || "Origin")}`,
      `destination=${route.destination.lat},${route.destination.lon}`,
      `destination_name=${encodeURIComponent(route.destination.label || "Destination")}`,
      `mode=${mode}`,
    ];
    if (route.via.length) {
      parts.push(`waypoints=${route.via.map((stop) => `${stop.lat},${stop.lon}`).join("|")}`);
      parts.push(
        `waypoint_names=${route.via.map((_, i) => encodeURIComponent(`Stop ${i + 1}`)).join("|")}`,
      );
    }
    return `https://omaps.app/v2/dir?${parts.join("&")}`;
  }

  function openExternal(url) {
    if (!url) {
      setStatus("No target available.", true);
      return;
    }
    if (!/^https?:/i.test(url)) {
      // Native schemes (magicearth:, geo:, om:) go through an anchor so the
      // browser hands off to the installed app instead of a popup.
      const link = document.createElement("a");
      link.href = url;
      link.rel = "noopener";
      document.body.appendChild(link);
      link.click();
      link.remove();
      return;
    }
    window.open(url, "_blank", "noopener");
  }

  function downloadFile(name, mime, content) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = name;
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  function downloadGpx() {
    const chosen = route.routes[route.index];
    if (!chosen?.points?.length) {
      setStatus("Plan a route first.", true);
      return;
    }
    const points = chosen.points
      .map(([lat, lon]) => `<trkpt lat="${Number(lat).toFixed(6)}" lon="${Number(lon).toFixed(6)}"></trkpt>`)
      .join("");
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="Pinpoint" xmlns="http://www.topografix.com/GPX/1/1">
  <trk><name>Pinpoint route</name><trkseg>${points}</trkseg></trk>
</gpx>`;
    downloadFile("pinpoint-route.gpx", "application/gpx+xml", gpx);
    setStatus("GPX exported.");
  }

  function downloadKml() {
    const chosen = route.routes[route.index];
    if (!chosen?.points?.length) {
      setStatus("Plan a route first.", true);
      return;
    }
    const line = chosen.points
      .map(([lat, lon]) => `${Number(lon).toFixed(6)},${Number(lat).toFixed(6)},0`)
      .join(" ");
    const marks = [];
    if (route.origin) marks.push({ name: "A", ...route.origin });
    route.via.forEach((stop, index) => marks.push({ name: `Stop ${index + 1}`, ...stop }));
    if (route.destination) marks.push({ name: "B", ...route.destination });
    const placemarks = marks
      .map(
        (mark) =>
          `<Placemark><name>${xmlEscape(mark.name)}</name><Point><coordinates>${Number(mark.lon).toFixed(6)},${Number(mark.lat).toFixed(6)},0</coordinates></Point></Placemark>`,
      )
      .join("");
    const kml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <name>Pinpoint route</name>${placemarks}
  <Placemark><name>Route</name><LineString><coordinates>${line}</coordinates></LineString></Placemark>
</Document></kml>`;
    downloadFile("pinpoint-route.kml", "application/vnd.google-earth.kml+xml", kml);
    setStatus("KML exported — import in Magic Earth, Organic Maps or Google My Maps.");
  }

  function downloadGeoJson() {
    const chosen = route.routes[route.index];
    if (!chosen?.points?.length) {
      setStatus("Plan a route first.", true);
      return;
    }
    const features = [];
    const pushPoint = (name, lat, lon) =>
      features.push({
        type: "Feature",
        properties: { name },
        geometry: { type: "Point", coordinates: [Number(lon), Number(lat)] },
      });
    if (route.origin) pushPoint("A", route.origin.lat, route.origin.lon);
    route.via.forEach((stop, index) => pushPoint(`Stop ${index + 1}`, stop.lat, stop.lon));
    if (route.destination) pushPoint("B", route.destination.lat, route.destination.lon);
    features.push({
      type: "Feature",
      properties: { name: "Pinpoint route" },
      geometry: {
        type: "LineString",
        coordinates: chosen.points.map(([lat, lon]) => [Number(lon), Number(lat)]),
      },
    });
    downloadFile(
      "pinpoint-route.geojson",
      "application/geo+json",
      JSON.stringify({ type: "FeatureCollection", features }, null, 2),
    );
    setStatus("GeoJSON exported.");
  }

  function xmlEscape(value) {
    return String(value ?? "").replace(/[&<>"']/g, (c) => ({
      "&": "&amp;",
      "<": "&lt;",
      ">": "&gt;",
      '"': "&quot;",
      "'": "&apos;",
    }[c]));
  }

  function renderAlternatives() {
    const wrap = $("route-alternatives");
    wrap.innerHTML = "";
    if (route.routes.length < 2) return;
    route.routes.forEach((candidate, i) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = `${formatDistance(candidate.distance_m)} · ${formatDuration(candidate.duration_s)}`;
      btn.addEventListener("click", () => applyRoute(i));
      wrap.appendChild(btn);
    });
  }

  function formatDuration(seconds) {
    const s = Number(seconds);
    if (!Number.isFinite(s)) return "";
    if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min`;
    const h = Math.floor(s / 3600);
    const m = Math.round((s % 3600) / 60);
    return `${h} h ${m} min`;
  }

  function downloadGpx() {
    const chosen = route.routes[route.index];
    if (!chosen?.points?.length) {
      setStatus("Plan a route first.", true);
      return;
    }
    const points = chosen.points
      .map(([lat, lon]) => `<trkpt lat="${Number(lat).toFixed(6)}" lon="${Number(lon).toFixed(6)}"></trkpt>`)
      .join("");
    const gpx = `<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="Pinpoint" xmlns="http://www.topografix.com/GPX/1/1">
  <trk><name>Pinpoint route</name><trkseg>${points}</trkseg></trk>
</gpx>`;
    const blob = new Blob([gpx], { type: "application/gpx+xml" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "pinpoint-route.gpx";
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    setStatus("GPX exported.");
  }

  function setMode(mode) {
    if (!capabilities.modes.includes(mode)) return;
    route.mode = mode;
    document
      .querySelectorAll(".mode")
      .forEach((btn) => btn.classList.toggle("active", btn.dataset.mode === mode));
    if (route.origin && route.destination) planRoute();
  }

  function swapEndpoints() {
    const tmp = route.origin;
    route.origin = route.destination;
    route.destination = tmp;
    renderRouteFields();
    if (route.origin && route.destination) planRoute();
  }

  function stopFollowing() {
    if (route.watchId != null) {
      navigator.geolocation.clearWatch(route.watchId);
      route.watchId = null;
    }
  }

  function showNavBanner(step, meta, { retry = false } = {}) {
    $("nav-step").textContent = step;
    $("nav-meta").textContent = meta || "";
    $("nav-retry").classList.toggle("hidden", !retry);
    $("nav-banner").classList.remove("hidden");
  }

  function hideNavBanner() {
    $("nav-banner").classList.add("hidden");
    $("nav-retry").classList.add("hidden");
  }

  function exitNavigation() {
    stopFollowing();
    mapApi.clearSelfLocation();
    hideNavBanner();
  }

  function clearRoutePanel() {
    const wasOpen = !routePanel.classList.contains("hidden");
    exitNavigation();
    route.routes = [];
    route.destination = null;
    route.via = [];
    route.avoid = [];
    route.addingStop = false;
    document.querySelectorAll(".route-avoid input").forEach((input) => {
      input.checked = false;
    });
    $("route-stops").innerHTML = "";
    $("route-progress").classList.add("hidden");
    $("route-next").textContent = "";
    mapApi.clearRoute();
    routePanel.classList.add("hidden");
    $("route-steps").innerHTML = "";
    $("route-alternatives").innerHTML = "";
    $("route-summary").textContent = "";
    if (wasOpen) focusMap();
  }

  function geolocationErrorMessage(err) {
    if (err?.code === 1) {
      return "Location permission is blocked for this site.";
    }
    if (err?.code === 3) {
      return "Location request timed out.";
    }
    return "Your location is currently unavailable.";
  }

  function enterNavigationPreview() {
    const chosen = route.routes[route.index];
    const step = chosen?.steps?.[0]?.text || chosen?.summary || "Route preview";
    const meta = chosen
      ? `${formatDistance(chosen.distance_m)} · ${formatDuration(chosen.duration_s)} · ${route.mode}`
      : "Route preview";
    showNavBanner(step, meta);
  }

  function startFollowing() {
    if (!route.origin || !route.destination) {
      setStatus("Plan a route first.", true);
      return;
    }
    // Navigation always starts in preview mode; GPS is an enhancement, not a
    // prerequisite. A denied permission must never make Start a dead end.
    enterNavigationPreview();
    if (!navigator.geolocation) {
      showNavBanner("Location unavailable", "Route preview only — this browser has no geolocation.", {
        retry: false,
      });
      return;
    }
    stopFollowing();
    let firstFix = true;
    route.watchId = navigator.geolocation.watchPosition(
      (pos) => {
        const fix = {
          lat: pos.coords.latitude,
          lon: pos.coords.longitude,
          accuracy: pos.coords.accuracy,
        };
        mapApi.setSelfLocation(fix);
        route.origin = { lat: fix.lat, lon: fix.lon, label: "My location" };
        renderRouteFields();
        updateLiveNext(fix);
        const chosen = route.routes[route.index];
        const step = chosen?.steps?.[0]?.text || "Following route";
        const meta = chosen
          ? `Following · ${formatDistance(chosen.distance_m)} · ${formatDuration(chosen.duration_s)}`
          : "Following";
        showNavBanner(step, meta);
        if (firstFix) {
          mapApi.map.panTo([fix.lat, fix.lon], { animate: true });
          firstFix = false;
        }
        setStatus("Following your location. Route preview only — no voice guidance.");
      },
      (err) => {
        const message = geolocationErrorMessage(err);
        showNavBanner("Location unavailable", `${message} Route preview is still active.`, {
          retry: true,
        });
        setStatus(`${message} Allow location and press Enable location.`, true);
      },
      { enableHighAccuracy: true, maximumAge: 10000, timeout: 12000 },
    );
  }

  function onMapClick({ lat, lon }) {
    if (!routePanel.classList.contains("hidden") && route.addingStop) {
      route.via.push({ lat, lon });
      route.addingStop = false;
      renderRouteFields();
      if (route.destination) planRoute();
      setStatus("Stop added.");
      return;
    }
    if (!routePanel.classList.contains("hidden")) {
      route.destination = {
        lat,
        lon,
        label: `Dropped pin (${lat.toFixed(4)}, ${lon.toFixed(4)})`,
      };
      renderRouteFields();
      planRoute();
      return;
    }
    reverseAt(lat, lon);
  }

  // Search form: submit is always allowed; live suggestions only when the
  // deployment is self-hosted (Nominatim policy forbids public autocomplete).
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    // Enter replaces the pending autocomplete run; a late one would rebuild
    // the results under the keyboard user.
    clearTimeout(debounceTimer);
    runSearch(input.value);
  });
  input.addEventListener("input", () => {
    if (!capabilities.autocomplete) return;
    clearTimeout(debounceTimer);
    const q = input.value.trim();
    if (q.length < MIN_QUERY) {
      showResults(false);
      return;
    }
    debounceTimer = setTimeout(() => runSearch(q), 300);
  });
  input.addEventListener("focus", () => {
    if (input.value.trim().length >= MIN_QUERY || !capabilities.autocomplete) {
      showResults(true);
    }
  });

  chips.querySelectorAll("button[data-q]").forEach((btn) => {
    btn.addEventListener("click", () => {
      input.value = btn.dataset.q;
      runSearch(btn.dataset.q);
    });
  });

  // One delegated handler survives result re-renders (the previous
  // per-button listeners could go stale between a search response and click).
  results.addEventListener("click", (event) => {
    const btn = event.target.closest("button.result");
    if (!btn) return;
    const place = currentResults[Number(btn.dataset.index)];
    if (!place) return;
    showResults(false);
    selectPlace(place);
  });

  // Keyboard access: ArrowDown leaves the box, arrows cycle the results and
  // native Enter clicks the focused button.
  input.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowDown") return;
    const first = results.querySelector("button.result");
    if (!first || results.classList.contains("hidden")) return;
    event.preventDefault();
    first.focus();
  });
  results.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const buttons = [...results.querySelectorAll("button.result")];
    const index = buttons.indexOf(document.activeElement);
    if (index < 0) return;
    event.preventDefault();
    if (event.key === "ArrowUp" && index === 0) {
      input.focus();
      return;
    }
    const step = event.key === "ArrowDown" ? 1 : -1;
    buttons[(index + step + buttons.length) % buttons.length].focus();
  });

  $("place-close").addEventListener("click", hidePlace);
  $("place-directions").addEventListener("click", () => {
    if (selected) openRouteFor(selected);
  });
  $("place-origin").addEventListener("click", () => {
    if (!selected) return;
    route.origin = {
      lat: Number(selected.latitude),
      lon: Number(selected.longitude),
      label: selected.short_name || selected.display_name || "Origin",
    };
    renderRouteFields();
    if (route.destination) {
      showRoutePanel();
      planRoute();
    } else {
      setStatus("Origin set — choose a destination from search or the map.");
    }
  });
  $("place-copy").addEventListener("click", async () => {
    if (!selected) return;
    const text = `${selected.display_name || ""}\n${selected.latitude}, ${selected.longitude}`;
    try {
      await navigator.clipboard.writeText(text);
      setStatus("Place copied.");
    } catch (_) {
      setStatus("Select the address and copy it manually.", true);
    }
  });
  $("place-osm").addEventListener("click", () => {
    if (!selected) return;
    const url = `https://www.openstreetmap.org/?mlat=${selected.latitude}&mlon=${selected.longitude}#map=16/${selected.latitude}/${selected.longitude}`;
    window.open(url, "_blank", "noopener");
  });
  $("place-google").addEventListener("click", () => {
    if (selected) openExternal(googlePlaceUrl(selected));
  });
  $("place-organic").addEventListener("click", () => {
    if (selected) openExternal(organicPlaceUrl(selected));
  });
  $("place-magic").addEventListener("click", () => {
    if (selected) openExternal(magicPlaceUrl(selected));
  });
  $("place-save").addEventListener("click", () => {
    if (!selected) return;
    const record = placeRecord(selected);
    const saved = loadList("savedPlaces");
    const index = saved.findIndex((item) => samePlace(item, record));
    if (index >= 0) {
      saved.splice(index, 1);
      setStatus("Removed from saved places.");
    } else {
      saved.unshift(record);
      setStatus("Saved on this device.");
    }
    saveList("savedPlaces", saved);
    renderStorage();
  });
  $("clear-saved").addEventListener("click", () => {
    saveList("savedPlaces", []);
    renderStorage();
    setStatus("Saved places cleared for this account.");
  });
  $("clear-recents").addEventListener("click", () => {
    saveList("recents", []);
    renderStorage();
    setStatus("Recent places cleared for this account.");
  });
  $("clear-routes").addEventListener("click", () => {
    saveRoutes([]);
    renderRouteHistory();
    setStatus("Route history cleared for this account.");
  });

  $("route-close").addEventListener("click", () => {
    clearRoutePanel();
  });
  $("nav-exit").addEventListener("click", exitNavigation);
  $("nav-retry").addEventListener("click", startFollowing);

  document.querySelectorAll(".mode").forEach((btn) => {
    btn.addEventListener("click", () => setMode(btn.dataset.mode));
  });
  $("route-swap").addEventListener("click", swapEndpoints);
  $("route-start").addEventListener("click", startFollowing);
  $("route-clear").addEventListener("click", clearRoutePanel);
  $("route-add-stop").addEventListener("click", () => {
    route.addingStop = true;
    setStatus("Click the map to add a stop.");
  });
  document.querySelectorAll(".route-avoid input").forEach((input) => {
    input.addEventListener("change", () => {
      route.avoid = [...document.querySelectorAll(".route-avoid input:checked")].map(
        (item) => item.value,
      );
      if (route.origin && route.destination) planRoute();
    });
  });
  $("route-gpx").addEventListener("click", downloadGpx);
  $("route-share").addEventListener("click", shareRoute);
  $("route-kml").addEventListener("click", downloadKml);
  $("route-geojson").addEventListener("click", downloadGeoJson);
  $("route-google").addEventListener("click", () => openExternal(googleRouteUrl()));
  $("route-organic").addEventListener("click", () => openExternal(organicRouteUrl()));
  $("route-magic").addEventListener("click", () => {
    if (!route.destination) {
      setStatus("Plan a route first.", true);
      return;
    }
    if (route.via.length) {
      setStatus("Magic Earth links support one destination — downloading KML for stops…");
      downloadKml();
      return;
    }
    openExternal(magicRouteUrl());
  });

  document.addEventListener("keydown", (e) => {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    if (results.contains(document.activeElement)) input.focus();
    showResults(false);
    hidePlace();
  });

  renderStorage();

  // Drag/collapse the route panel (desktop); touch uses the bottom sheet.
  const dragHandle = $("route-drag");
  let panelDrag = null;
  dragHandle.addEventListener("pointerdown", (event) => {
    if (window.innerWidth < 760) return;
    const rect = routePanel.getBoundingClientRect();
    panelDrag = { startY: event.clientY, startTop: rect.top };
    dragHandle.setPointerCapture(event.pointerId);
  });
  dragHandle.addEventListener("pointermove", (event) => {
    if (!panelDrag) return;
    const top = Math.max(
      70,
      Math.min(window.innerHeight - 220, panelDrag.startTop + event.clientY - panelDrag.startY),
    );
    routePanel.style.top = `${top}px`;
  });
  const endDrag = () => {
    panelDrag = null;
  };
  dragHandle.addEventListener("pointerup", endDrag);
  dragHandle.addEventListener("pointercancel", endDrag);
  $("route-collapse").addEventListener("click", () => {
    const collapsed = routePanel.classList.toggle("collapsed");
    $("route-collapse").textContent = collapsed ? "▸" : "▾";
  });

  return {
    onMapClick,
    route: () => route,
    selected: () => selected,
    selectPlace,
    openRouteFor,
    routeFrom,
    refreshStorage: renderStorage,
  };
}

export function formatDistance(meters) {
  const m = Number(meters);
  if (!Number.isFinite(m)) return "";
  return m < 1000 ? `${Math.round(m)} m` : `${(m / 1000).toFixed(1)} km`;
}
