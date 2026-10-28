// ---------------------------------------------------------------------------
// Always Together — web client configuration.
//
// Tile source policy (see THIRD_PARTY_NOTICES.md):
// The default is the Wikimedia Foundation tile endpoint, which explicitly
// welcomes third-party applications (unlike tile.openstreetmap.org, whose
// OSMF Tile Usage Policy prohibits app backends). Data is still ©
// OpenStreetMap contributors (ODbL) — attribution is rendered permanently in
// the map panel and enforced here via `attribution`.
//
// Self-hosted deployments (recommended for production): point `tiles.url`
// at your own server (Martin / TileServer GL serving Geofabrik extracts).
// ---------------------------------------------------------------------------

export const API_BASE = "/api/v1";

export const tiles = {
  // {z}/{x}/{y} are substituted client-side. No API key: the endpoint must
  // stay keyless on purpose — keys imply accounts and third-party signup.
  url: "https://maps.wikimedia.org/osm-intl/{z}/{x}/{y}.png",
  // ODbL attribution — must always be visible on the map (legal obligation).
  attribution: '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener">OpenStreetMap</a> contributors, <a href="https://wmflabs.org" target="_blank" rel="noopener">Wikimedia maps</a>',
  maxZoom: 19,
};

// Location freshness thresholds (minutes) — mirrored from the web UI states.
export const freshness = {
  warnMinutes: 10,
  staleMinutes: 30,
};

// Service worker cache version — bump when any static asset changes.
export const SW_VERSION = "v1";
