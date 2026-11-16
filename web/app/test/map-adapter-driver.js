// Drives the web-app map adapter for the headless integration test.
// Demo coordinates only (Monaco fixture); no account, session or friend data.

import { createMap } from "/app/map.js";

const SAMPLE_LOCATIONS = [
  {
    user_id: 1,
    user_name: "Ana",
    latitude: 43.7384,
    longitude: 7.4246,
    precision_level: "exact",
  },
  {
    user_id: 2,
    user_name: "Mihai",
    latitude: 43.7311,
    longitude: 7.4197,
    precision_level: "approximate",
  },
  {
    user_id: 3,
    user_name: "Elena",
    latitude: 43.7451,
    longitude: 7.4302,
    precision_level: "city_only",
  },
];

const ROUTE = {
  points: [
    [43.7384, 7.4246],
    [43.7343, 7.4211],
    [43.7451, 7.4302],
  ],
};

const readout = document.getElementById("click-readout");

const api = createMap({
  element: document.getElementById("map"),
  center: [43.7384, 7.4246],
  zoom: 13.2,
  theme: "dark",
  onClick({ lat, lon }) {
    readout.textContent = `${lat.toFixed(4)}, ${lon.toFixed(4)}`;
  },
  onLoad() {
    window.__mapReady = true;
  },
  onError(error) {
    window.__mapError = error?.message || String(error);
    console.error("[map-adapter-test]", error);
  },
});

window.__mapAdapter = api;
api.setFriendMarkers(SAMPLE_LOCATIONS);
api.setSelectedPlace({
  latitude: 43.7402,
  longitude: 7.4268,
  short_name: "Café du Port",
});
api.setSelfLocation({ lat: 43.737, lon: 7.423, accuracy: 120 });
api.drawRoute(ROUTE, [{ lat: 43.733, lon: 7.421 }]);

window.__pinpointTest = { api, sampleLocations: SAMPLE_LOCATIONS, route: ROUTE };
