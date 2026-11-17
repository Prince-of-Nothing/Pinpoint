/**
 * Default offline-pack catalog for the web client.
 *
 * Browser code must use same-origin URLs only. The production Moldova pack is
 * published as a GitHub release, but GitHub release assets do not send CORS
 * headers, so a `fetch()` from the app or a WebView page cannot read them.
 * The Pinpoint server exposes a same-origin proxy instead:
 *
 *   /offline/packs/moldova/manifest.json
 *   /offline/packs/moldova/moldova.pmtiles
 *   /offline/packs/moldova/moldova-search.sqlite
 *   /offline/packs/moldova/moldova-valhalla.tar
 *
 * Never replace these with `github.com` release URLs in browser code. The
 * proxy is responsible for forwarding the files and for rewriting the
 * manifest's file URLs so every download in `downloadPack()` stays on the
 * Pinpoint origin. A cross-origin pack host is technically supported by
 * `downloadPack`, but it must send CORS headers and is not used here.
 */

const MONACO_FIXTURE_URL = new URL("./fixtures/offline-pack/manifest.json", import.meta.url).href;

export const MOLDOVA_PROXY_ROOT = "/offline/packs/moldova/";

export const MOLDOVA_PROXY_FILES = Object.freeze([
  `${MOLDOVA_PROXY_ROOT}manifest.json`,
  `${MOLDOVA_PROXY_ROOT}moldova.pmtiles`,
  `${MOLDOVA_PROXY_ROOT}moldova-search.sqlite`,
  `${MOLDOVA_PROXY_ROOT}moldova-valhalla.tar`,
]);

/**
 * `approxBytes` is a UI estimate only; the verified size comes from the
 * manifest. The Moldova entry is deliberate about being optional/manual: the
 * demo must never start a 220 MB transfer on page load or in CI.
 */
export const DEFAULT_PACK_CATALOG = Object.freeze([
  Object.freeze({
    id: "monaco",
    name: "Monaco",
    kind: "fixture",
    manifestUrl: MONACO_FIXTURE_URL,
    approxBytes: 94794,
    optional: false,
    manual: false,
    note: "Small ODbL fixture used by the automated smoke test.",
  }),
  Object.freeze({
    id: "moldova",
    name: "Moldova",
    kind: "region",
    manifestUrl: `${MOLDOVA_PROXY_ROOT}manifest.json`,
    approxBytes: 220_000_000,
    optional: true,
    manual: true,
    note: "Production pack. Optional manual download through the same-origin proxy (~220 MB).",
    proxyFiles: MOLDOVA_PROXY_FILES,
  }),
]);

/**
 * @param {string} id
 * @returns {object|undefined}
 */
export function findCatalogPack(id) {
  return DEFAULT_PACK_CATALOG.find((entry) => entry.id === id);
}
