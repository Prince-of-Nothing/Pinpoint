/**
 * Demo wiring for `route-demo.html`.
 *
 * The router below is an in-page stub: it never touches the network and always
 * returns the same synthetic Chișinău → Bălți geometry with two maneuvers. It
 * exists to exercise the route UI (states, summary, turn-by-turn banner) before
 * a native or WASM routing engine is wired in.
 */

import { createRouteUI, formatDistance, formatDuration } from "./route-ui.js";

const SVG_NS = "http://www.w3.org/2000/svg";
const PREVIEW_WIDTH = 420;
const PREVIEW_HEIGHT = 320;
const PREVIEW_PADDING = 46;

const PROFILE_LABELS = {
  auto: "Auto",
  pedestrian: "Walk",
  bicycle: "Bike",
};

// A hand-written, plausible-looking line from Chișinău to Bălți. It is not
// OSM-derived and must not be presented as a real road route.
const STUB_POINTS = [
  [47.0105, 28.8638],
  [47.07, 28.83],
  [47.15, 28.78],
  [47.24, 28.7],
  [47.33, 28.6],
  [47.42, 28.51],
  [47.51, 28.42],
  [47.6, 28.34],
  [47.69, 28.27],
  [47.7615, 28.194],
];

const STUB_MANEUVERS = [
  {
    instruction: "Head north on the M14 towards Orhei",
    distance_m: 42000,
  },
  {
    instruction:
      "At the roundabout, take the second exit onto Strada Ștefan cel Mare",
    distance_m: 86400,
  },
];

const PROFILE_DURATION_S = {
  auto: 6600,
  pedestrian: 79200,
  bicycle: 21600,
};

const preview = document.querySelector("#route-preview");
const readout = document.querySelector("#route-readout");
const stateBadge = document.querySelector("#route-state");

const demo = {
  ready: false,
  calls: [],
  cancels: [],
  lastRoute: null,
  lastRequest: null,
  ui: null,
};

function svgElement(tag, attributes = {}) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    node.setAttribute(name, String(value));
  }
  return node;
}

function project(points) {
  if (points.length === 0) return [];

  const latitudes = points.map(([lat]) => lat);
  const midLatitude = ((Math.min(...latitudes) + Math.max(...latitudes)) / 2) * (Math.PI / 180);
  const scaleLatitude = Math.cos(midLatitude);

  const xs = points.map(([, lon]) => lon * scaleLatitude);
  const ys = latitudes;
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const xRange = Math.max(maxX - minX, 1e-6);
  const yRange = Math.max(maxY - minY, 1e-6);

  const scale = Math.min(
    (PREVIEW_WIDTH - PREVIEW_PADDING * 2) / xRange,
    (PREVIEW_HEIGHT - PREVIEW_PADDING * 2) / yRange,
  );
  const offsetX = (PREVIEW_WIDTH - xRange * scale) / 2;
  const offsetY = (PREVIEW_HEIGHT - yRange * scale) / 2;

  return points.map((_, index) => ({
    x: offsetX + (xs[index] - minX) * scale,
    y: PREVIEW_HEIGHT - (offsetY + (ys[index] - minY) * scale),
  }));
}

function addMarker(point, label, color) {
  const group = svgElement("g");
  group.append(
    svgElement("circle", {
      cx: point.x,
      cy: point.y,
      r: 8,
      fill: color,
      stroke: "#070b14",
      "stroke-width": 3,
    }),
    svgElement("text", {
      x: point.x,
      y: point.y - 14,
      "text-anchor": "middle",
    }),
  );
  group.lastChild.textContent = label;
  return group;
}

function clearPreview() {
  preview.replaceChildren();
  preview.setAttribute("aria-label", "Empty route preview");
}

function drawRoute(route) {
  clearPreview();
  const projected = project(route.points);

  if (projected.length >= 2) {
    const points = projected
      .map((point) => `${point.x.toFixed(1)},${point.y.toFixed(1)}`)
      .join(" ");
    preview.append(
      svgElement("polyline", {
        points,
        fill: "none",
        stroke: "#070b14",
        "stroke-width": 10,
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
        opacity: 0.85,
      }),
      svgElement("polyline", {
        points,
        fill: "none",
        stroke: "#34d399",
        "stroke-width": 4.5,
        "stroke-linecap": "round",
        "stroke-linejoin": "round",
      }),
      addMarker(projected[0], "A", "#34d399"),
      addMarker(projected[projected.length - 1], "B", "#f87171"),
    );
  }

  preview.setAttribute(
    "aria-label",
    `Synthetic route from Chișinău to Bălți, ${formatDistance(route.distance_m)}`,
  );
}

async function stubRoute(request) {
  demo.calls.push({
    from: { ...request.from },
    to: { ...request.to },
    profile: request.profile,
  });

  // Simulate a local worker round-trip so the loading state is visible.
  await new Promise((resolve) => setTimeout(resolve, 160));

  const duration = PROFILE_DURATION_S[request.profile] ?? PROFILE_DURATION_S.auto;
  return {
    distance_m: 128400,
    duration_s: duration,
    points: STUB_POINTS.map(([lat, lon]) => [lat, lon]),
    maneuvers: STUB_MANEUVERS.map((maneuver) => ({ ...maneuver })),
  };
}

demo.ui = createRouteUI({
  container: "#route-ui",
  route: stubRoute,
  onRoute: ({ route, request }) => {
    demo.lastRoute = route;
    demo.lastRequest = request;
    drawRoute(route);
    stateBadge.dataset.state = "ready";
    stateBadge.textContent = `${route.maneuvers.length} maneuvers`;
    readout.textContent = [
      "Chișinău → Bălți",
      formatDistance(route.distance_m),
      formatDuration(route.duration_s),
      `${route.maneuvers.length} maneuvers`,
      PROFILE_LABELS[request?.profile] ?? "Route",
    ].join(" · ");
  },
  onCancel: ({ reason }) => {
    demo.cancels.push(reason);
    if (reason === "clear") {
      clearPreview();
      stateBadge.dataset.state = "empty";
      stateBadge.textContent = "No route";
      readout.textContent = "Route cleared. The map layer was removed by onCancel.";
      return;
    }
    stateBadge.dataset.state = "stopped";
    stateBadge.textContent = "Stopped";
    readout.textContent = "Guidance stopped. The route stays on the map.";
  },
});

demo.ready = true;
window.__pinpointRouteDemo = demo;
