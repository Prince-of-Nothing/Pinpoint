/**
 * Pinpoint service worker.
 *
 * Responsibilities:
 *  - Precache the offline demo shell, the shared map module and the vendored
 *    MapLibre/PMTiles assets so the manager works without a network.
 *  - Serve `/offline-packs/<id>/<file>` straight from OPFS with HTTP Range
 *    (`Accept-Ranges`, `Content-Range`, `206`) because PMTiles reads archives
 *    with range requests.
 *  - Fall back to the network only for same-origin, non-pack requests. Pack
 *    requests never leave the device; cross-origin requests are untouched.
 *
 * Bump `VERSION` when the precache list or handler logic changes. The cache
 * name is versioned, and activation removes older Pinpoint caches.
 */

const VERSION = "pinpoint-offline-v7";
const CACHE_PREFIX = "pinpoint-offline-";
const SHELL_CACHE = `${CACHE_PREFIX}${VERSION}-shell`;
const RUNTIME_CACHE = `${CACHE_PREFIX}${VERSION}-runtime`;
const SCOPE = self.registration.scope;
const SW_PATH = new URL(self.location.href).pathname;

const PACK_ROUTES = [...new Set([new URL("offline-packs/", SCOPE).pathname, "/offline-packs/"])];

/**
 * Relative to the service worker scope. All of these files are committed and
 * local; there is no CDN in this list and no runtime CDN import anywhere.
 */
const PRECACHE = [
  "app/offline-demo.html",
  "app/offline-demo.js",
  "app/offline.css",
  "app/offline.js",
  "app/offline-catalog.js",
  "app/fixtures/offline-pack/manifest.json",
  "app/fixtures/offline-pack/manifest-broken.json",
  "app/fixtures/offline-pack/LICENSE.txt",
  "map/map.js",
  "map/style.json",
  "map/fixtures/monaco.pmtiles",
  "vendor/maplibre-gl.js",
  "vendor/maplibre-gl-worker.mjs",
  "vendor/maplibre-gl.css",
  "vendor/pmtiles.js",
];

const MIME_TYPES = {
  ".css": "text/css; charset=utf-8",
  ".gz": "application/gzip",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".mbtiles": "application/vnd.mapbox-vector-tile",
  ".pbf": "application/x-protobuf",
  ".pmtiles": "application/vnd.pmtiles",
  ".sqlite": "application/vnd.sqlite3",
  ".sqlite3": "application/vnd.sqlite3",
  ".tar": "application/x-tar",
  ".txt": "text/plain; charset=utf-8",
  ".webp": "image/webp",
};

const ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const FILE_NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;

const stats = {
  packRequests: 0,
  pack206: 0,
  pack200: 0,
  pack404: 0,
  pack416: 0,
  shellCacheHits: 0,
  networkFetches: 0,
  offlineFallbacks: 0,
};

function scopedUrl(path) {
  return new URL(path, SCOPE).href;
}

function isPackPath(pathname) {
  return PACK_ROUTES.some((prefix) => pathname.startsWith(prefix));
}

function contentType(name) {
  const dot = name.lastIndexOf(".");
  const extension = dot >= 0 ? name.slice(dot).toLowerCase() : "";
  return MIME_TYPES[extension] ?? "application/octet-stream";
}

function textResponse(status, message, extraHeaders = {}) {
  return new Response(message, {
    status,
    headers: {
      "Content-Type": "text/plain; charset=utf-8",
      "Cache-Control": "no-store",
      ...extraHeaders,
    },
  });
}

/**
 * Parse a single-range `Range` header. Multi-range requests and malformed
 * headers return `null` so the caller can answer 416.
 */
function parseRange(header, size) {
  const match = /^bytes=(\d*)-(\d*)$/.exec(String(header ?? "").trim());
  if (!match) return null;

  const [, startText, endText] = match;
  if (startText === "" && endText === "") return null;

  let start;
  let end;

  if (startText === "") {
    const suffixLength = Number(endText);
    if (!Number.isInteger(suffixLength) || suffixLength <= 0) return null;
    start = Math.max(0, size - suffixLength);
    end = size - 1;
  } else {
    start = Number(startText);
    end = endText === "" ? size - 1 : Number(endText);
    if (!Number.isInteger(start) || !Number.isInteger(end)) return null;
    if (start < 0 || start >= size) return null;
    if (end >= size) end = size - 1;
    if (end < start) return null;
  }

  return { start, end };
}

async function readPackFile(pathname) {
  const prefix = PACK_ROUTES.find((route) => pathname.startsWith(route));
  if (!prefix) return null;

  const segments = pathname.slice(prefix.length).split("/");
  if (segments.length !== 2) return null;

  let id;
  let name;
  try {
    id = decodeURIComponent(segments[0]);
    name = decodeURIComponent(segments[1]);
  } catch {
    return null;
  }

  if (!ID_PATTERN.test(id) || !FILE_NAME_PATTERN.test(name)) return null;
  if (!navigator.storage?.getDirectory) return null;

  try {
    const root = await navigator.storage.getDirectory();
    const packs = await root.getDirectoryHandle("offline-packs");
    const pack = await packs.getDirectoryHandle(id);
    const files = await pack.getDirectoryHandle("files");
    const handle = await files.getFileHandle(name);
    return { id, name, file: await handle.getFile() };
  } catch {
    return null;
  }
}

async function servePack(request, pathname) {
  stats.packRequests += 1;

  const pack = await readPackFile(pathname);
  if (!pack) {
    stats.pack404 += 1;
    return textResponse(404, "Offline pack file not found");
  }

  const { file } = pack;
  const size = file.size;
  const rangeHeader = request.headers.get("range");
  const baseHeaders = {
    "Content-Type": contentType(pack.name),
    "Accept-Ranges": "bytes",
    "Cache-Control": "no-store",
    "Content-Disposition": `inline; filename="${pack.name}"`,
    "X-Pinpoint-Pack": `${pack.id}/${pack.name}`,
  };

  if (!rangeHeader) {
    stats.pack200 += 1;
    return new Response(request.method === "HEAD" ? null : file.stream(), {
      status: 200,
      headers: { ...baseHeaders, "Content-Length": String(size) },
    });
  }

  const range = parseRange(rangeHeader, size);
  if (!range) {
    stats.pack416 += 1;
    return textResponse(416, "Range Not Satisfiable", {
      "Accept-Ranges": "bytes",
      "Content-Range": `bytes */${size}`,
    });
  }

  const { start, end } = range;
  stats.pack206 += 1;
  const body = request.method === "HEAD" ? null : file.slice(start, end + 1).stream();

  return new Response(body, {
    status: 206,
    headers: {
      ...baseHeaders,
      "Content-Range": `bytes ${start}-${end}/${size}`,
      "Content-Length": String(end - start + 1),
    },
  });
}

async function serveShell(request) {
  const url = new URL(request.url);
  const shell = await caches.open(SHELL_CACHE);
  const runtime = await caches.open(RUNTIME_CACHE);

  // `offline.js` asks for manifests with `cache: "no-store"` so an update
  // check sees the published manifest while online. Honour that by going to
  // the network first and refreshing the cached copy; fall back to the cache
  // when offline.
  const bypassCache = request.cache === "no-store" || request.cache === "reload";

  if (!bypassCache) {
    const cached = await shell.match(request);
    if (cached) {
      stats.shellCacheHits += 1;
      return cached;
    }

    const runtimeCached = await runtime.match(request);
    if (runtimeCached) {
      stats.shellCacheHits += 1;
      return runtimeCached;
    }
  }

  try {
    const response = await fetch(request);
    stats.networkFetches += 1;

    const cacheable =
      request.method === "GET" &&
      response.ok &&
      response.type === "basic" &&
      url.pathname !== SW_PATH &&
      !url.pathname.startsWith("/api/") &&
      !url.pathname.startsWith("/map/") &&
      !request.headers.has("range");

    if (cacheable) {
      const copy = response.clone();
      const target = (await shell.match(request)) ? shell : runtime;
      await target.put(request, copy);
    }

    return response;
  } catch (error) {
    const cached = (await shell.match(request)) || (await runtime.match(request));
    if (cached) {
      stats.offlineFallbacks += 1;
      return cached;
    }

    if (request.mode === "navigate") {
      stats.offlineFallbacks += 1;
      const fallback = await shell.match(scopedUrl("app/offline-demo.html"));
      if (fallback) return fallback;
      return textResponse(
        503,
        "Pinpoint offline assets are not cached yet. Reconnect once to finish installing the offline shell.",
      );
    }
    throw error;
  }
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(SHELL_CACHE);
      await cache.addAll(PRECACHE.map(scopedUrl));
      await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      const keys = await caches.keys();
      await Promise.all(
        keys
          .filter(
            (key) =>
              key.startsWith(CACHE_PREFIX) && key !== SHELL_CACHE && key !== RUNTIME_CACHE,
          )
          .map((key) => caches.delete(key)),
      );
      await self.clients.claim();
    })(),
  );
});

self.addEventListener("fetch", (event) => {
  const { request } = event;
  if (request.method !== "GET" && request.method !== "HEAD") return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  if (isPackPath(url.pathname)) {
    event.respondWith(servePack(request, url.pathname));
    return;
  }

  event.respondWith(serveShell(request));
});

self.addEventListener("message", (event) => {
  const data = event.data ?? {};

  if (data.type === "PINPOINT_SW_STATS") {
    const reply = {
      type: "PINPOINT_SW_STATS",
      version: VERSION,
      stats: { ...stats },
    };
    event.source?.postMessage(reply);
  }

  if (data.type === "PINPOINT_SKIP_WAITING") {
    self.skipWaiting();
  }
});
