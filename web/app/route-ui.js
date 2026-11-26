/**
 * Pinpoint route and turn-by-turn UI.
 *
 * A self-contained ES module that renders a route planner, route summary and a
 * visual next-maneuver banner. It has no imports and performs **no network
 * requests**: the caller injects the routing function, so the same component
 * works with the web app, the Android/desktop WebView map page, and a local
 * Valhalla/WASM or native Rust router.
 *
 * The host page owns the stylesheet (`route.css`) and the map. Draw the route
 * line from `onRoute`; clear it from `onCancel`.
 *
 * Usage:
 *   import { createRouteUI } from "./route-ui.js";
 *
 *   const ui = createRouteUI({
 *     container: "#route-ui",
 *     route: async ({ from, to, profile }) => localRouter.route(from, to, profile),
 *     onRoute: ({ route }) => mapApi.drawRoute(route),
 *     onCancel: ({ reason }) => {
 *       if (reason === "clear") mapApi.clearRoute();
 *     },
 *   });
 *
 * Wire format accepted by `onRoute`:
 *   {
 *     distance_m: 128400,
 *     duration_s: 6600,
 *     points: [[47.0105, 28.8638], ...],
 *     maneuvers: [{ instruction: "Turn right", distance_m: 120 }, ...],
 *   }
 */

const PROFILE_OPTIONS = [
  { id: "auto", label: "Auto" },
  { id: "pedestrian", label: "Walk" },
  { id: "bicycle", label: "Bike" },
];

const PROFILE_LABELS = Object.fromEntries(
  PROFILE_OPTIONS.map((profile) => [profile.id, profile.label]),
);

let instanceCount = 0;

/**
 * Format metres as the rest of the Pinpoint UI does (m under 1 km, km above).
 *
 * @param {number} meters
 * @returns {string}
 */
export function formatDistance(meters) {
  const value = Number(meters);
  if (!Number.isFinite(value) || value < 0) return "";
  return value < 1000 ? `${Math.round(value)} m` : `${(value / 1000).toFixed(1)} km`;
}

/**
 * Format seconds as "12 min" / "1 h 50 min".
 *
 * @param {number} seconds
 * @returns {string}
 */
export function formatDuration(seconds) {
  const value = Number(seconds);
  if (!Number.isFinite(value) || value < 0) return "";
  if (value < 3600) return `${Math.max(1, Math.round(value / 60))} min`;
  const hours = Math.floor(value / 3600);
  const minutes = Math.round((value % 3600) / 60);
  return `${hours} h ${minutes} min`;
}

/**
 * Parse a "lat,lon" field value.
 *
 * Accepts a comma, semicolon or whitespace between the two decimal numbers.
 * Decimal points only (a decimal comma is ambiguous with the separator).
 *
 * @param {string} text
 * @param {string} [label] Field name used in the error message.
 * @returns {{lat: number, lon: number}}
 * @throws {Error} when the text is not a valid coordinate pair.
 */
export function parseLatLon(text, label = "Coordinates") {
  const raw = String(text ?? "").trim();
  const match = /^(-?\d+(?:\.\d+)?)\s*(?:,|;|\s)\s*(-?\d+(?:\.\d+)?)$/.exec(raw);
  if (!match) {
    throw new Error(`${label}: enter coordinates as "lat, lon", for example 47.0105, 28.8638.`);
  }

  const lat = Number(match[1]);
  const lon = Number(match[2]);

  if (!Number.isFinite(lat) || Math.abs(lat) > 90) {
    throw new Error(`${label}: latitude must be between -90 and 90.`);
  }
  if (!Number.isFinite(lon) || Math.abs(lon) > 180) {
    throw new Error(`${label}: longitude must be between -180 and 180.`);
  }

  return { lat, lon };
}

function isEndpoint(value) {
  const lat = Number(value?.lat);
  const lon = Number(value?.lon);
  return (
    Number.isFinite(lat) &&
    Number.isFinite(lon) &&
    Math.abs(lat) <= 90 &&
    Math.abs(lon) <= 180
  );
}

function coerceEndpoint(value) {
  if (!isEndpoint(value)) return null;
  return { lat: Number(value.lat), lon: Number(value.lon) };
}

function coerceRequest(value) {
  if (!value || typeof value !== "object") return null;
  const from = coerceEndpoint(value.from);
  const to = coerceEndpoint(value.to);
  if (!from || !to) return null;
  const profile = PROFILE_LABELS[value.profile] ? value.profile : "auto";
  return { from, to, profile };
}

/**
 * Validate and normalise a route response. Unknown extra fields are preserved
 * for the host (for example maneuver type or location), but the contract
 * fields always come out with the expected type.
 *
 * @param {object} raw
 * @returns {object}
 * @throws {TypeError} when the two required numbers are missing/invalid.
 */
export function normalizeRoute(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    throw new TypeError("route must be an object");
  }

  const distance = Number(raw.distance_m);
  const duration = Number(raw.duration_s);
  if (!Number.isFinite(distance) || distance < 0) {
    throw new TypeError("route.distance_m must be a non-negative number");
  }
  if (!Number.isFinite(duration) || duration < 0) {
    throw new TypeError("route.duration_s must be a non-negative number");
  }

  const points = [];
  for (const point of Array.isArray(raw.points) ? raw.points : []) {
    if (!Array.isArray(point) || point.length < 2) continue;
    const lat = Number(point[0]);
    const lon = Number(point[1]);
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) continue;
    if (Math.abs(lat) > 90 || Math.abs(lon) > 180) continue;
    points.push([lat, lon]);
  }

  const maneuvers = [];
  for (const maneuver of Array.isArray(raw.maneuvers) ? raw.maneuvers : []) {
    const instruction = String(maneuver?.instruction ?? "").trim();
    if (!instruction) continue;
    const legDistance = Number(maneuver?.distance_m);
    maneuvers.push({
      ...maneuver,
      instruction,
      distance_m: Number.isFinite(legDistance) && legDistance >= 0 ? legDistance : null,
    });
  }

  return {
    ...raw,
    distance_m: distance,
    duration_s: duration,
    points,
    maneuvers,
  };
}

function errorMessage(error) {
  if (error instanceof Error && error.message.trim()) return error.message.trim();
  const text = String(error ?? "").trim();
  return text || "Routing failed.";
}

function makeElement(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/**
 * Create the route UI inside `container`.
 *
 * @param {object} options
 * @param {HTMLElement|string} options.container Element or selector. Required.
 * @param {(request: {
 *   from: {lat: number, lon: number},
 *   to: {lat: number, lon: number},
 *   profile: "auto"|"pedestrian"|"bicycle"
 * }) => Promise<object>} options.route Injected local routing function. It must
 *   resolve to `{distance_m, duration_s, points, maneuvers}`. It must never
 *   call an online service.
 * @param {(event: {route: object, request: object|null}) => void} [options.onRoute]
 *   Called after a route is displayed (including `setRoute`), so the host can
 *   draw the line and fit the map.
 * @param {(event: {reason: "stop"|"clear", route: object|null}) => void} [options.onCancel]
 *   Called for user-initiated cancellation. `reason: "stop"` ends guidance but
 *   keeps the route; `"clear"` means the route should be removed from the map.
 * @returns {{
 *   element: HTMLElement,
 *   setRoute: (route: object, request?: object) => void,
 *   clear: () => void,
 *   start: () => void,
 *   stop: () => void,
 *   getRoute: () => object|null,
 *   getRequest: () => object|null,
 *   destroy: () => void,
 * }}
 */
export function createRouteUI({ container, route, onRoute, onCancel } = {}) {
  const host =
    typeof container === "string" ? document.querySelector(container) : container;
  if (!host || typeof host.appendChild !== "function") {
    throw new TypeError("createRouteUI: container was not found");
  }

  const requestRoute = typeof route === "function" ? route : null;
  const onRouteHandler = typeof onRoute === "function" ? onRoute : null;
  const onCancelHandler = typeof onCancel === "function" ? onCancel : null;

  const instanceId = `pinpoint-route-${(instanceCount += 1)}`;
  let destroyed = false;
  let profile = "auto";
  let currentRoute = null;
  let currentRequest = null;
  let navigating = false;
  let maneuverIndex = 0;
  let requestSeq = 0;

  const root = makeElement("section", "pinpoint-route");
  root.dataset.state = "idle";
  root.setAttribute("aria-label", "Route planner");

  const form = document.createElement("form");
  form.className = "pinpoint-route__form";
  form.noValidate = true;

  function buildField({ marker, label, name, example }) {
    const field = makeElement("div", "pinpoint-route__field");
    const badge = makeElement("span", "pinpoint-route__badge", marker);
    badge.setAttribute("aria-hidden", "true");

    const control = makeElement("div", "pinpoint-route__control");
    const inputId = `${instanceId}-${name}`;
    const hintId = `${instanceId}-${name}-hint`;

    const labelEl = makeElement("label", "pinpoint-route__label", label);
    labelEl.htmlFor = inputId;

    const input = document.createElement("input");
    input.className = "pinpoint-route__input";
    input.id = inputId;
    input.name = name;
    input.type = "text";
    input.inputMode = "decimal";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.placeholder = example;
    input.setAttribute("aria-describedby", hintId);

    const hint = makeElement(
      "p",
      "pinpoint-route__hint",
      `Coordinates as latitude, longitude (${example}).`,
    );
    hint.id = hintId;

    control.append(labelEl, input, hint);
    field.append(badge, control);
    return { field, input };
  }

  const fromField = buildField({
    marker: "A",
    label: "From (latitude, longitude)",
    name: "from",
    example: "47.0105, 28.8638",
  });
  const toField = buildField({
    marker: "B",
    label: "To (latitude, longitude)",
    name: "to",
    example: "47.7615, 28.1940",
  });

  const profiles = document.createElement("fieldset");
  profiles.className = "pinpoint-route__profiles";
  const legend = makeElement("legend", "pinpoint-route__legend", "Travel mode");
  const profileRow = makeElement("div", "pinpoint-route__profile-row");

  const profileButtons = new Map();
  for (const option of PROFILE_OPTIONS) {
    const button = makeElement("button", "pinpoint-route__profile", option.label);
    button.type = "button";
    button.dataset.profile = option.id;
    button.setAttribute("aria-pressed", String(option.id === profile));
    button.addEventListener("click", () => {
      if (destroyed) return;
      profile = option.id;
      for (const [id, node] of profileButtons) {
        node.setAttribute("aria-pressed", String(id === profile));
        node.classList.toggle("is-active", id === profile);
      }
    });
    profileButtons.set(option.id, button);
    profileRow.appendChild(button);
  }
  profileButtons.get("auto")?.classList.add("is-active");
  profiles.append(legend, profileRow);

  const actions = makeElement("div", "pinpoint-route__actions");
  const routeButton = makeElement(
    "button",
    "pinpoint-route__button pinpoint-route__button--primary",
    "Route",
  );
  routeButton.type = "submit";
  routeButton.dataset.action = "route";

  const navButton = makeElement("button", "pinpoint-route__button", "Start");
  navButton.type = "button";
  navButton.dataset.action = "nav";
  navButton.disabled = true;
  navButton.setAttribute("aria-pressed", "false");

  const nextButton = makeElement(
    "button",
    "pinpoint-route__button pinpoint-route__button--quiet",
    "Next maneuver",
  );
  nextButton.type = "button";
  nextButton.dataset.action = "next";
  nextButton.hidden = true;

  const clearButton = makeElement(
    "button",
    "pinpoint-route__button pinpoint-route__button--quiet",
    "Clear",
  );
  clearButton.type = "button";
  clearButton.dataset.action = "clear";
  clearButton.disabled = true;

  actions.append(routeButton, navButton, nextButton, clearButton);

  const errorEl = makeElement("p", "pinpoint-route__error");
  errorEl.dataset.role = "error";
  errorEl.setAttribute("role", "alert");
  errorEl.hidden = true;

  const summaryEl = makeElement("p", "pinpoint-route__summary");
  summaryEl.dataset.role = "summary";
  summaryEl.setAttribute("role", "status");
  summaryEl.setAttribute("aria-live", "polite");
  summaryEl.setAttribute("aria-atomic", "true");

  const banner = makeElement("div", "pinpoint-route__banner");
  banner.dataset.role = "banner";
  banner.setAttribute("role", "status");
  banner.setAttribute("aria-live", "assertive");
  banner.setAttribute("aria-atomic", "true");
  banner.hidden = true;

  const bannerKicker = makeElement("span", "pinpoint-route__banner-kicker");
  bannerKicker.dataset.role = "maneuver-kicker";
  const bannerText = makeElement("span", "pinpoint-route__banner-text");
  bannerText.dataset.role = "maneuver";
  const bannerMeta = makeElement("span", "pinpoint-route__banner-meta");
  bannerMeta.dataset.role = "maneuver-distance";
  banner.append(bannerKicker, bannerText, bannerMeta);

  const directions = makeElement("section", "pinpoint-route__directions");
  directions.setAttribute("aria-label", "Turn-by-turn directions");
  directions.hidden = true;
  const directionsTitle = makeElement("h3", "pinpoint-route__directions-title", "Directions");
  const stepsList = makeElement("ol", "pinpoint-route__steps");
  stepsList.dataset.role = "steps";
  directions.append(directionsTitle, stepsList);

  form.append(
    fromField.field,
    toField.field,
    profiles,
    actions,
    errorEl,
    summaryEl,
    banner,
    directions,
  );
  root.appendChild(form);
  host.appendChild(root);

  function safeInvoke(callback, payload, name) {
    if (typeof callback !== "function") return;
    try {
      callback(payload);
    } catch (error) {
      console.error(`[pinpoint-route] ${name} callback failed`, error);
    }
  }

  function setState(next) {
    root.dataset.state = next;
  }

  function showError(message) {
    errorEl.textContent = message;
    errorEl.hidden = false;
  }

  function clearError() {
    errorEl.textContent = "";
    errorEl.hidden = true;
  }

  function clearBanner() {
    banner.hidden = true;
    bannerKicker.textContent = "";
    bannerText.textContent = "";
    bannerMeta.textContent = "";
  }

  function setBusy(busy) {
    routeButton.disabled = busy;
    if (busy) {
      routeButton.setAttribute("aria-busy", "true");
      form.setAttribute("aria-busy", "true");
    } else {
      routeButton.removeAttribute("aria-busy");
      form.removeAttribute("aria-busy");
    }
  }

  function resetNavigation() {
    navigating = false;
    maneuverIndex = 0;
    navButton.textContent = "Start";
    navButton.setAttribute("aria-pressed", "false");
    nextButton.hidden = true;
    clearBanner();
    for (const step of stepsList.children) step.classList.remove("is-current");
  }

  function syncFields(request) {
    if (!request) return;
    fromField.input.value = `${request.from.lat.toFixed(5)}, ${request.from.lon.toFixed(5)}`;
    toField.input.value = `${request.to.lat.toFixed(5)}, ${request.to.lon.toFixed(5)}`;
  }

  function renderSteps(routeValue) {
    stepsList.replaceChildren();
    routeValue.maneuvers.forEach((maneuver, index) => {
      const item = makeElement("li", "pinpoint-route__step");
      item.dataset.index = String(index);
      const text = makeElement("span", "pinpoint-route__step-text", maneuver.instruction);
      const meta = makeElement(
        "span",
        "pinpoint-route__step-meta",
        Number.isFinite(maneuver.distance_m) ? formatDistance(maneuver.distance_m) : "",
      );
      item.append(text, meta);
      stepsList.appendChild(item);
    });
    directions.hidden = routeValue.maneuvers.length === 0;
  }

  function highlightStep(index) {
    for (const step of stepsList.children) {
      step.classList.toggle("is-current", Number(step.dataset.index) === index);
    }
  }

  function renderBanner() {
    if (!currentRoute) {
      clearBanner();
      return;
    }

    const maneuvers = currentRoute.maneuvers;
    if (maneuvers.length === 0) {
      bannerKicker.textContent = "Route preview";
      bannerText.textContent = "No turn-by-turn instructions for this route.";
      bannerMeta.textContent = "";
      banner.hidden = false;
      nextButton.hidden = true;
      return;
    }

    const maneuver = maneuvers[maneuverIndex];
    const final = maneuverIndex === maneuvers.length - 1;
    bannerKicker.textContent = final
      ? "Final maneuver"
      : `Next maneuver · ${maneuverIndex + 1} of ${maneuvers.length}`;
    bannerText.textContent = maneuver.instruction;
    bannerMeta.textContent = Number.isFinite(maneuver.distance_m)
      ? `in ${formatDistance(maneuver.distance_m)}`
      : "";
    banner.hidden = false;
    nextButton.hidden = final;
    nextButton.disabled = final;
    highlightStep(maneuverIndex);
  }

  function applyRoute(routeValue, requestValue, { sync = false } = {}) {
    currentRoute = routeValue;
    currentRequest = requestValue;
    resetNavigation();
    syncFields(sync ? requestValue : null);

    const parts = [
      formatDistance(routeValue.distance_m),
      formatDuration(routeValue.duration_s),
    ];
    const profileLabel = requestValue ? PROFILE_LABELS[requestValue.profile] : null;
    if (profileLabel) parts.push(profileLabel);
    summaryEl.textContent = parts.filter(Boolean).join(" · ");
    summaryEl.dataset.distance = String(routeValue.distance_m);

    clearError();
    setBusy(false);
    setState("ready");
    clearButton.disabled = false;
    navButton.disabled = false;
    nextButton.hidden = true;
    renderSteps(routeValue);

    safeInvoke(
      onRouteHandler,
      { route: routeValue, request: requestValue },
      "onRoute",
    );
  }

  function resetRoute() {
    requestSeq += 1;
    currentRoute = null;
    currentRequest = null;
    resetNavigation();
    setBusy(false);
    clearError();
    summaryEl.textContent = "";
    delete summaryEl.dataset.distance;
    stepsList.replaceChildren();
    directions.hidden = true;
    navButton.disabled = true;
    clearButton.disabled = true;
    setState("idle");
  }

  function readRequest() {
    const from = parseLatLon(fromField.input.value, "From");
    const to = parseLatLon(toField.input.value, "To");
    return { from, to, profile };
  }

  async function handleSubmit(event) {
    event.preventDefault();
    if (destroyed) return;

    let requestValue;
    try {
      requestValue = readRequest();
    } catch (error) {
      showError(errorMessage(error));
      if (!currentRoute) {
        setState("error");
        summaryEl.textContent = "";
      }
      return;
    }

    if (!requestRoute) {
      showError("No offline routing engine is available for this page.");
      if (!currentRoute) {
        setState("error");
        summaryEl.textContent = "";
      }
      return;
    }

    const ticket = (requestSeq += 1);
    if (navigating) {
      safeInvoke(
        onCancelHandler,
        { reason: "stop", route: currentRoute },
        "onCancel",
      );
    }
    currentRoute = null;
    currentRequest = null;
    resetNavigation();
    clearError();
    clearButton.disabled = true;
    navButton.disabled = true;
    stepsList.replaceChildren();
    directions.hidden = true;
    summaryEl.textContent = "Routing…";
    delete summaryEl.dataset.distance;
    setBusy(true);
    setState("loading");

    try {
      const result = await requestRoute({
        from: { ...requestValue.from },
        to: { ...requestValue.to },
        profile: requestValue.profile,
      });
      if (destroyed || ticket !== requestSeq) return;
      const normalized = normalizeRoute(result);
      applyRoute(normalized, requestValue);
    } catch (error) {
      if (destroyed || ticket !== requestSeq) return;
      setBusy(false);
      setState("error");
      summaryEl.textContent = "";
      showError(errorMessage(error));
    }
  }

  function startNavigation() {
    if (destroyed || !currentRoute) return;
    navigating = true;
    maneuverIndex = 0;
    navButton.textContent = "Stop";
    navButton.setAttribute("aria-pressed", "true");
    setState("navigating");
    renderBanner();
  }

  function stopNavigation({ notify = false } = {}) {
    if (!navigating) return;
    const wasNavigating = navigating;
    resetNavigation();
    setState(currentRoute ? "ready" : "idle");
    if (notify && wasNavigating) {
      safeInvoke(
        onCancelHandler,
        { reason: "stop", route: currentRoute },
        "onCancel",
      );
    }
  }

  function setRoute(nextRoute, requestValue) {
    if (destroyed) throw new Error("createRouteUI: this instance was destroyed");
    const normalized = normalizeRoute(nextRoute);
    const coerced = coerceRequest(requestValue);
    applyRoute(normalized, coerced, { sync: Boolean(coerced) });
  }

  function advanceManeuver() {
    if (destroyed || !navigating || !currentRoute) return;
    const maneuvers = currentRoute.maneuvers;
    if (maneuverIndex >= maneuvers.length - 1) return;
    maneuverIndex += 1;
    renderBanner();
  }

  form.addEventListener("submit", handleSubmit);

  navButton.addEventListener("click", () => {
    if (destroyed) return;
    if (navigating) stopNavigation({ notify: true });
    else startNavigation();
  });

  nextButton.addEventListener("click", advanceManeuver);

  clearButton.addEventListener("click", () => {
    if (destroyed) return;
    const hadRoute = Boolean(currentRoute);
    resetRoute();
    if (hadRoute) {
      safeInvoke(onCancelHandler, { reason: "clear", route: null }, "onCancel");
    }
  });

  function clear() {
    if (destroyed) return;
    resetRoute();
  }

  function start() {
    startNavigation();
  }

  function stop() {
    stopNavigation();
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    requestSeq += 1;
    currentRoute = null;
    currentRequest = null;
    form.removeEventListener("submit", handleSubmit);
    root.remove();
  }

  return {
    element: root,
    setRoute,
    clear,
    start,
    stop,
    getRoute: () => currentRoute,
    getRequest: () => (currentRequest ? { ...currentRequest } : null),
    destroy,
  };
}
