/**
 * Pinpoint offline place search.
 *
 * Searches the M1 `places`/`places_fts` SQLite index entirely on-device. The
 * database and the SQLite WASM engine are fetched from the page's own origin;
 * there is no CDN, tile server or geocoder request, and no query text ever
 * leaves the device.
 *
 * The module is deliberately independent of `map.js` and `web/app/`: the web
 * app, Android WebView and desktop WebView can import it directly.
 *
 * ```js
 * import { openSearch } from "/map/search.js";
 *
 * const search = await openSearch({
 *   sqliteUrl: "/packs/moldova-search.sqlite",
 * });
 *
 * try {
 *   for (const place of search.search("balti", 5)) {
 *     console.log(place.name, place.lat, place.lon);
 *   }
 * } finally {
 *   search.close();
 * }
 * ```
 *
 * The vendored engine is `@sqlite.org/sqlite-wasm` 3.54.1-build1 because the
 * M1 index uses FTS5. The stock `sql.js` browser build is smaller but does not
 * compile FTS5, so it cannot query `places_fts`.
 */

const SQLITE_MODULE_URL = new URL("../vendor/sqlite3.mjs", import.meta.url);
const SQLITE_WASM_URL = new URL("../vendor/sqlite3.wasm", import.meta.url);

const DEFAULT_LIMIT = 10;
const MAX_LIMIT = 50;

/**
 * Multiplier applied to `p.importance` inside the FTS5 rank expression.
 * `rank` is negative (lower is better), so multiplying it by
 * `1 + boost * importance` promotes important places without letting
 * importance completely replace text relevance.
 */
const IMPORTANCE_BOOST = 0.75;

const SEARCH_SQL = `
  SELECT p.name AS name,
         p.kind AS kind,
         p.lat AS lat,
         p.lon AS lon,
         p.address AS address,
         p.importance AS importance
    FROM places_fts AS f
    JOIN places AS p ON p.id = f.rowid
   WHERE places_fts MATCH ?1
   ORDER BY (f.rank * (1.0 + ?2 * p.importance)) ASC,
            p.importance DESC,
            p.name COLLATE NOCASE ASC
   LIMIT ?3
`;

/**
 * The M1 index normalises Romanian diacritics and the Russian `ё` before
 * writing `places.normalized`. Mirror that folding for the query tokens so
 * `balti` finds `Bălți` and `кишин` finds `Кишинёв`.
 */
const DIACRITIC_FOLD = {
  ă: "a",
  â: "a",
  î: "i",
  ș: "s",
  ț: "t",
  ş: "s",
  ţ: "t",
  ё: "е",
};

const QUERY_TOKEN = /[\p{L}\p{N}]+/gu;

let enginePromise = null;

/**
 * Resolve a caller-supplied path against the page and require it to stay on
 * the page's origin. This is what keeps a pack on a CDN or an attacker-controlled
 * host from being loaded accidentally.
 *
 * @param {string|URL|undefined|null} value
 * @param {URL|string|undefined|null} fallback
 * @param {string} label
 * @returns {URL}
 */
function resolveLocalUrl(value, fallback, label) {
  const documentBase = globalThis.document?.baseURI;
  const locationHref = globalThis.location?.href;
  const base = documentBase ?? locationHref;
  if (!base) {
    throw new Error(`openSearch: ${label} requires a document or location`);
  }

  const raw = value ?? fallback;
  if (raw === undefined || raw === null || raw === "") {
    throw new Error(`openSearch: ${label} is required`);
  }

  const url = new URL(String(raw), base);
  const pageOrigin = globalThis.location?.origin ?? new URL(base).origin;
  if (url.origin !== pageOrigin) {
    throw new Error(
      `openSearch: ${label} must be same-origin (got ${url.origin}, expected ${pageOrigin})`,
    );
  }
  return url;
}

/**
 * Fetch a same-origin binary asset and return it as bytes. The response is
 * checked explicitly so a missing pack or a static host returning an HTML
 * error page fails with a useful message.
 *
 * @param {URL} url
 * @param {string} label
 * @returns {Promise<Uint8Array>}
 */
async function fetchBytes(url, label) {
  let response;
  try {
    response = await fetch(url, { credentials: "same-origin" });
  } catch (error) {
    throw new Error(`openSearch: failed to fetch ${label} ${url.href}: ${error}`);
  }

  if (!response.ok) {
    throw new Error(
      `openSearch: failed to fetch ${label} ${url.href}: HTTP ${response.status}`,
    );
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength === 0) {
    throw new Error(`openSearch: ${label} ${url.href} is empty`);
  }
  return bytes;
}

/**
 * Load and initialise the vendored SQLite WASM module once per page.
 *
 * `wasmBinary` is passed explicitly so Emscripten never guesses a script
 * directory or fetches a sibling URL. Only `wasmUrl`, which has already passed
 * the same-origin check, is used.
 *
 * @param {URL} wasmUrl
 * @returns {Promise<any>}
 */
function loadEngine(wasmUrl) {
  if (enginePromise) return enginePromise;

  enginePromise = (async () => {
    const [module, wasmBytes] = await Promise.all([
      import(SQLITE_MODULE_URL.href),
      fetchBytes(wasmUrl, "SQLite WASM"),
    ]);
    const initSqlite = module.default;
    if (typeof initSqlite !== "function") {
      throw new Error(
        `openSearch: ${SQLITE_MODULE_URL.href} did not export an SQLite initialiser`,
      );
    }
    return initSqlite({ wasmBinary: wasmBytes });
  })();

  enginePromise.catch(() => {
    enginePromise = null;
  });

  return enginePromise;
}

/**
 * Split a user query into Unicode letter/number runs and fold the diacritics
 * the M1 builder folds. Punctuation, emoji and operators become separators,
 * which is both safer for FTS5 syntax and friendlier for typed queries such
 * as `"Bălți, Moldova"`.
 *
 * @param {unknown} query
 * @returns {string[]}
 */
function tokenizeQuery(query) {
  const text = String(query ?? "").normalize("NFKC");
  const tokens = text.match(QUERY_TOKEN) ?? [];
  return tokens
    .map((token) =>
      token.replace(/[ăâîșțşţё]/giu, (char) => DIACRITIC_FOLD[char.toLowerCase()] ?? char),
    )
    .map((token) => token.toLowerCase())
    .filter(Boolean);
}

/**
 * Turn tokens into FTS5 prefix terms. Quoting each token makes FTS5 keywords
 * such as `OR`, `NEAR` and `*` inert; every token is a plain prefix.
 *
 * @param {string[]} tokens
 * @returns {string}
 */
function matchExpression(tokens) {
  return tokens
    .map((token) => `"${token.replace(/"/g, '""')}"*`)
    .join(" ");
}

/** Clamp a caller limit to a safe prepared-statement bound. */
function normalizeLimit(limit) {
  if (limit === undefined || limit === null) return DEFAULT_LIMIT;
  const value = Number(limit);
  if (!Number.isFinite(value)) return DEFAULT_LIMIT;
  return Math.min(MAX_LIMIT, Math.max(1, Math.trunc(value)));
}

/** Check that the database really is the M1 search schema before querying. */
function assertPinpointIndex(db) {
  const rows = db.selectArrays(
    "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('places', 'places_fts')",
  );
  const names = new Set((rows ?? []).map((row) => String(row[0])));
  if (!names.has("places") || !names.has("places_fts")) {
    throw new Error(
      "openSearch: SQLite file is not a Pinpoint search index (places/places_fts missing)",
    );
  }
}

/**
 * Open a search index on the current origin.
 *
 * @param {object} options
 * @param {string|URL} options.sqliteUrl Required. M1 `places`/`places_fts`
 *   SQLite file. Relative paths resolve against the page (`document.baseURI`).
 * @param {string|URL} [options.wasmUrl] Optional override for the vendored
 *   SQLite WASM file. Relative paths resolve against the page. It must be
 *   same-origin. The default is `web/vendor/sqlite3.wasm` relative to this
 *   module.
 * @returns {Promise<{
 *   search: (query: string, limit?: number) => Array<{
 *     name: string, kind: string, lat: number, lon: number,
 *     address: string|null, importance: number
 *   }>,
 *   close: () => void
 * }>}
 */
export async function openSearch({ sqliteUrl, wasmUrl } = {}) {
  const databaseUrl = resolveLocalUrl(sqliteUrl, undefined, "sqliteUrl");
  const engineWasmUrl = resolveLocalUrl(wasmUrl, SQLITE_WASM_URL, "wasmUrl");

  const [databaseBytes, sqlite3] = await Promise.all([
    fetchBytes(databaseUrl, "search index"),
    loadEngine(engineWasmUrl),
  ]);

  const db = new sqlite3.oo1.DB();
  let pointer;
  try {
    pointer = sqlite3.wasm.allocFromTypedArray(databaseBytes);
  } catch (error) {
    db.close();
    throw new Error(
      `openSearch: could not allocate WASM memory for ${databaseUrl.href}: ${error}`,
    );
  }

  const flags =
    sqlite3.capi.SQLITE_DESERIALIZE_FREEONCLOSE |
    sqlite3.capi.SQLITE_DESERIALIZE_RESIZEABLE;

  try {
    const result = sqlite3.capi.sqlite3_deserialize(
      db.pointer,
      "main",
      pointer,
      databaseBytes.byteLength,
      databaseBytes.byteLength,
      flags,
    );
    if (result !== sqlite3.capi.SQLITE_OK) {
      sqlite3.wasm.dealloc(pointer);
      throw new Error(
        `openSearch: SQLite could not open ${databaseUrl.href} (code ${result})`,
      );
    }

    // Search only. This also makes accidental writes from future call sites
    // fail instead of mutating a pack on the origin.
    db.exec("PRAGMA query_only = ON");
    assertPinpointIndex(db);
  } catch (error) {
    db.close();
    throw error;
  }

  let closed = false;
  let database = db;

  return {
    /**
     * Search the local index synchronously. The database is in WASM memory
     * after `openSearch` resolves, so there is no I/O here.
     *
     * Empty and punctuation-only queries return `[]` without touching FTS5.
     * FTS5 treats such a MATCH expression as malformed; this module never
     * constructs one.
     *
     * @param {string} query
     * @param {number} [limit]
     * @returns {Array<{name: string, kind: string, lat: number, lon: number,
     *   address: string|null, importance: number}>}
     */
    search(query, limit = DEFAULT_LIMIT) {
      if (closed || !database) {
        throw new Error("search: the search handle is closed");
      }

      const tokens = tokenizeQuery(query);
      if (tokens.length === 0) return [];

      const rows = database.selectObjects(SEARCH_SQL, [
        matchExpression(tokens),
        IMPORTANCE_BOOST,
        normalizeLimit(limit),
      ]);

      return (rows ?? []).map((row) => ({
        name: String(row.name ?? ""),
        kind: String(row.kind ?? "place"),
        lat: Number(row.lat),
        lon: Number(row.lon),
        address: row.address === null || row.address === undefined ? null : String(row.address),
        importance: Number(row.importance ?? 0),
      }));
    },

    /**
     * Release the WASM database. Safe to call more than once. A later
     * `search` throws rather than silently returning no results.
     */
    close() {
      if (closed) return;
      closed = true;
      database?.close();
      database = null;
    },
  };
}
