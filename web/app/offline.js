/**
 * Pinpoint offline pack manager (web).
 *
 * Downloads a pack manifest plus its files, verifies byte counts and SHA-256
 * with WebCrypto, stores them in the Origin Private File System (OPFS) and
 * exposes the installed packs to the page. A companion service worker
 * (`web/sw.js`) serves `/offline-packs/<id>/<file>` from the same OPFS tree
 * with HTTP Range support so PMTiles can read it.
 *
 * Manifest contract (M1):
 *
 * ```json
 * {
 *   "id": "monaco",
 *   "version": "2026.11.16",
 *   "files": [
 *     { "name": "monaco.pmtiles", "url": "monaco.pmtiles", "bytes": 94389,
 *       "sha256": "<64 hex characters>" }
 *   ],
 *   "attribution": "© OpenStreetMap contributors",
 *   "license": "ODbL-1.0",
 *   "license_url": "https://opendatacommons.org/licenses/odbl/1-0/"
 * }
 * ```
 *
 * OPFS layout:
 *
 * ```text
 * offline-packs/
 *   monaco/
 *     pack.json                 installed metadata (written last)
 *     files/
 *       monaco.pmtiles
 *       LICENSE.txt
 * ```
 *
 * Downloads are written to `.staging-<id>-<nonce>/`. Only a fully verified
 * staging directory is moved into `<id>/`, and `pack.json` is transferred
 * last. An interrupted download therefore never appears in `listPacks()`.
 *
 * This module has no dependencies, no analytics and no hard-coded network
 * origins. The only requests it makes are the manifest and files the caller
 * asks it to download. Pack hosts may be cross-origin (Supabase Storage or
 * R2, for example); add that origin to the page CSP `connect-src`.
 */

const SCHEMA_VERSION = 1;
const OPFS_ROOT = "offline-packs";
const FILES_SUBDIR = "files";
const METADATA_FILE = "pack.json";
const MAX_FILES = 64;
const PROGRESS_INTERVAL_MS = 100;

/** Root-absolute URL prefix served by `web/sw.js`. */
export const OFFLINE_PACK_URL_PREFIX = "/offline-packs/";

const ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;
const FILE_NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const SHA256_PATTERN = /^[A-Fa-f0-9]{64}$/;

const activeManifestUrls = new Set();
const activePackIds = new Set();

/**
 * Error with a stable `code` so callers can branch without parsing messages.
 * Codes: `MANIFEST_INVALID`, `INVALID_URL`, `NETWORK`, `SIZE_MISMATCH`,
 * `HASH_MISMATCH`, `QUOTA`, `UNSUPPORTED`, `CANCELLED`, `BUSY`, `STORAGE`,
 * `NOT_FOUND`.
 */
export class OfflinePackError extends Error {
  constructor(code, message, options = {}) {
    super(message);
    this.name = "OfflinePackError";
    this.code = code;
    if (options.cause) this.cause = options.cause;
  }
}

function fail(code, message, cause) {
  throw new OfflinePackError(code, message, cause ? { cause } : undefined);
}

function isObject(value) {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function isSafeInteger(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function assertPlainText(value, field, maxLength) {
  if (typeof value !== "string") fail("MANIFEST_INVALID", `manifest.${field} must be a string`);
  const text = value.trim();
  if (!text) fail("MANIFEST_INVALID", `manifest.${field} must not be empty`);
  if (text.length > maxLength) {
    fail("MANIFEST_INVALID", `manifest.${field} is longer than ${maxLength} characters`);
  }
  if (/[\u0000-\u001f\u007f]/.test(text)) {
    fail("MANIFEST_INVALID", `manifest.${field} contains control characters`);
  }
  return text;
}

function assertPackId(value) {
  if (typeof value !== "string" || !ID_PATTERN.test(value)) {
    fail(
      "MANIFEST_INVALID",
      "manifest.id must be 1-64 characters of letters, digits, dot, dash or underscore and must not start with a dot",
    );
  }
  return value;
}

function assertFileName(value) {
  if (typeof value !== "string" || !FILE_NAME_PATTERN.test(value)) {
    fail(
      "MANIFEST_INVALID",
      `file name ${JSON.stringify(value)} is not a simple name (letters, digits, dot, dash, underscore; no traversal)`,
    );
  }
  // The regex already excludes `.` and `..`, but keep the explicit guard so a
  // future regex relaxation cannot reintroduce path traversal.
  if (value === "." || value === ".." || value.includes("/") || value.includes("\\")) {
    fail("MANIFEST_INVALID", `file name ${JSON.stringify(value)} is not allowed`);
  }
  return value;
}

/**
 * Validate and normalise an M1 manifest. Pure and synchronous; it never makes
 * a request and never touches OPFS. Throws `OfflinePackError` with code
 * `MANIFEST_INVALID` on any problem.
 *
 * @param {unknown} manifest Parsed JSON manifest.
 * @returns {object} Normalised manifest with a computed `totalBytes`.
 */
export function verifyManifest(manifest) {
  if (!isObject(manifest)) fail("MANIFEST_INVALID", "manifest must be a JSON object");

  const id = assertPackId(manifest.id);
  const version = assertPlainText(manifest.version, "version", 96);
  const attribution = assertPlainText(manifest.attribution, "attribution", 500);
  const license = assertPlainText(manifest.license, "license", 120);
  const licenseUrlRaw = assertPlainText(manifest.license_url, "license_url", 2048);
  const licenseUrl = assertHttpUrl(licenseUrlRaw, "manifest.license_url");

  if (!Array.isArray(manifest.files) || manifest.files.length === 0) {
    fail("MANIFEST_INVALID", "manifest.files must be a non-empty array");
  }
  if (manifest.files.length > MAX_FILES) {
    fail("MANIFEST_INVALID", `manifest.files may contain at most ${MAX_FILES} entries`);
  }

  const seen = new Set();
  let totalBytes = 0;

  const files = manifest.files.map((entry, index) => {
    if (!isObject(entry)) fail("MANIFEST_INVALID", `manifest.files[${index}] must be an object`);
    const name = assertFileName(entry.name);
    if (seen.has(name)) fail("MANIFEST_INVALID", `duplicate file name ${JSON.stringify(name)}`);
    seen.add(name);

    if (!isSafeInteger(entry.bytes) || entry.bytes <= 0) {
      fail("MANIFEST_INVALID", `manifest.files[${index}].bytes must be a positive integer`);
    }
    if (typeof entry.sha256 !== "string" || !SHA256_PATTERN.test(entry.sha256.trim())) {
      fail("MANIFEST_INVALID", `manifest.files[${index}].sha256 must be 64 hexadecimal characters`);
    }
    if (typeof entry.url !== "string" || !entry.url.trim()) {
      fail("MANIFEST_INVALID", `manifest.files[${index}].url must be a non-empty string`);
    }

    totalBytes += entry.bytes;
    if (!Number.isSafeInteger(totalBytes)) {
      fail("MANIFEST_INVALID", "manifest total size is too large");
    }

    return {
      name,
      url: entry.url.trim(),
      bytes: entry.bytes,
      sha256: entry.sha256.trim().toLowerCase(),
    };
  });

  return {
    ...manifest,
    id,
    version,
    attribution,
    license,
    license_url: licenseUrl,
    files,
    totalBytes,
  };
}

function assertHttpUrl(value, field) {
  let url;
  try {
    url = new URL(value);
  } catch {
    fail("INVALID_URL", `${field} is not a valid URL`);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    fail("INVALID_URL", `${field} must use http or https`);
  }
  if (url.username || url.password) {
    fail("INVALID_URL", `${field} must not contain credentials`);
  }
  return url.href;
}

function resolveUrl(raw, base, field) {
  if (typeof raw !== "string" || !raw.trim()) fail("INVALID_URL", `${field} must be a non-empty string`);
  let url;
  try {
    url = new URL(raw.trim(), base);
  } catch {
    fail("INVALID_URL", `${field} is not a valid URL`);
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    fail("INVALID_URL", `${field} must use http or https`);
  }
  if (url.username || url.password) {
    fail("INVALID_URL", `${field} must not contain credentials`);
  }
  return url.href;
}

function randomId() {
  if (globalThis.crypto?.randomUUID) return globalThis.crypto.randomUUID();
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

function throwIfAborted(signal) {
  if (signal?.aborted) fail("CANCELLED", "Download cancelled");
}

function isAbortError(error) {
  return error?.name === "AbortError" || error?.code === 20;
}

function toPackError(error, fallbackMessage = "Offline pack operation failed") {
  if (error instanceof OfflinePackError) return error;
  if (isAbortError(error)) return new OfflinePackError("CANCELLED", "Download cancelled");
  if (error?.name === "QuotaExceededError") {
    return new OfflinePackError(
      "QUOTA",
      "Not enough browser storage for this pack. Free some space and try again.",
      { cause: error },
    );
  }
  if (error?.name === "NotAllowedError") {
    return new OfflinePackError(
      "UNSUPPORTED",
      "The browser denied access to offline storage. Private windows may disable it.",
      { cause: error },
    );
  }
  const message = error instanceof Error && error.message ? error.message : String(error);
  return new OfflinePackError("STORAGE", `${fallbackMessage}: ${message}`, { cause: error });
}

async function getPacksDirectory({ create }) {
  if (!navigator.storage?.getDirectory) {
    fail("UNSUPPORTED", "This browser does not support OPFS (navigator.storage.getDirectory).");
  }
  const root = await navigator.storage.getDirectory();
  try {
    return await root.getDirectoryHandle(OPFS_ROOT, { create });
  } catch (error) {
    if (!create && (error?.name === "NotFoundError" || error?.name === "TypeMismatchError")) {
      return null;
    }
    throw error;
  }
}

async function listDirectory(directory) {
  const entries = [];
  if (typeof directory.entries === "function") {
    for await (const [name, handle] of directory.entries()) entries.push([name, handle]);
  } else {
    for await (const handle of directory.values()) entries.push([handle.name, handle]);
  }
  return entries;
}

async function removeEntryQuietly(directory, name) {
  if (!directory) return;
  try {
    await directory.removeEntry(name, { recursive: true });
  } catch {
    // Missing or already removed. Cleanup must never mask the real error.
  }
}

async function writeFile(directory, name, data) {
  let writable;
  try {
    const handle = await directory.getFileHandle(name, { create: true });
    writable = await handle.createWritable();
    await writable.write(data);
    await writable.close();
  } catch (error) {
    if (writable) {
      try {
        await writable.abort();
      } catch {
        // Ignore abort failures; the original error is more useful.
      }
    }
    throw error;
  }
}

async function writeJson(directory, name, value) {
  await writeFile(directory, name, JSON.stringify(value, null, 2));
}

async function readJson(directory, name) {
  const handle = await directory.getFileHandle(name);
  const file = await handle.getFile();
  return JSON.parse(await file.text());
}

/**
 * Move a file into `targetDirectory`, falling back to a verified copy when the
 * browser does not implement `FileSystemFileHandle.move()`.
 */
async function transferFile(handle, targetDirectory, name) {
  if (typeof handle.move === "function") {
    try {
      await handle.move(targetDirectory, name);
      return;
    } catch {
      // Some WebKit/Firefox builds expose move but reject OPFS destinations.
      // Fall through to a copy so the pack still commits.
    }
  }

  const file = await handle.getFile();
  const target = await targetDirectory.getFileHandle(name, { create: true });
  const writable = await target.createWritable();
  try {
    await writable.write(file);
  } finally {
    await writable.close();
  }
}

/**
 * Recursively transfer `source` into `target`. `pack.json` is transferred
 * last so a final directory without metadata is never listed as installed.
 */
async function transferDirectory(source, target, { deferMetadata = false } = {}) {
  const entries = await listDirectory(source);
  const regular = entries.filter(([name]) => !(deferMetadata && name === METADATA_FILE));
  const deferred = entries.filter(([name]) => deferMetadata && name === METADATA_FILE);

  for (const [name, handle] of [...regular, ...deferred]) {
    if (handle.kind === "file") {
      await transferFile(handle, target, name);
    } else if (handle.kind === "directory") {
      const child = await target.getDirectoryHandle(name, { create: true });
      await transferDirectory(handle, child);
    }
  }
}

/**
 * Transfer a staging directory to `<packs>/<id>` and remove the staging
 * directory. Metadata is moved last by `transferDirectory`.
 */
async function commitDirectory(packsDirectory, source, destinationName) {
  const target = await packsDirectory.getDirectoryHandle(destinationName, { create: true });
  await transferDirectory(source, target, { deferMetadata: true });
  await removeEntryQuietly(packsDirectory, source.name);
}

function packFileUrl(id, name) {
  return `${OFFLINE_PACK_URL_PREFIX}${encodeURIComponent(id)}/${encodeURIComponent(name)}`;
}

function findPmtilesFile(files) {
  const match = files.find((file) => file.name.toLowerCase().endsWith(".pmtiles"));
  return match?.name ?? null;
}

function toPublicPack(metadata) {
  const files = metadata.files.map((file) => ({
    name: file.name,
    bytes: file.bytes,
    sha256: file.sha256,
    url: packFileUrl(metadata.id, file.name),
  }));
  const pmtiles = findPmtilesFile(files);

  return {
    id: metadata.id,
    name: typeof metadata.name === "string" && metadata.name ? metadata.name : metadata.id,
    version: metadata.version,
    installedAt: metadata.installedAt,
    source: metadata.source,
    totalBytes: metadata.totalBytes,
    attribution: metadata.attribution,
    license: metadata.license,
    license_url: metadata.license_url,
    files,
    pmtiles,
    pmtilesUrl: pmtiles ? packFileUrl(metadata.id, pmtiles) : null,
    built_at: metadata.built_at ?? null,
    bbox: metadata.bbox ?? null,
    minzoom: metadata.minzoom ?? null,
    maxzoom: metadata.maxzoom ?? null,
  };
}

async function readMetadata(packsDirectory, id) {
  let directory;
  try {
    directory = await packsDirectory.getDirectoryHandle(id);
  } catch (error) {
    if (error?.name === "NotFoundError" || error?.name === "TypeMismatchError") return null;
    throw error;
  }

  try {
    const raw = await readJson(directory, METADATA_FILE);
    if (!isObject(raw) || raw.schema !== SCHEMA_VERSION || raw.id !== id) return null;
    const manifest = verifyManifest(raw);
    return { ...raw, totalBytes: manifest.totalBytes, files: manifest.files };
  } catch (error) {
    if (error instanceof OfflinePackError) return null;
    if (error?.name === "NotFoundError" || error?.name === "TypeMismatchError") return null;
    if (error instanceof SyntaxError) return null;
    throw error;
  }
}

/**
 * List installed packs, newest metadata first is not guaranteed; results are
 * sorted by `id` for stable rendering.
 *
 * @returns {Promise<Array<object>>}
 */
export async function listPacks() {
  const packsDirectory = await getPacksDirectory({ create: false });
  if (!packsDirectory) return [];

  const packs = [];
  for (const [name, handle] of await listDirectory(packsDirectory)) {
    if (name.startsWith(".") || handle.kind !== "directory") continue;
    try {
      const metadata = await readMetadata(packsDirectory, name);
      if (metadata) packs.push(toPublicPack(metadata));
    } catch {
      // A corrupt or half-written pack is ignored and never listed as
      // installed. `deletePack` can still remove it by id.
    }
  }

  packs.sort((a, b) => a.id.localeCompare(b.id));
  return packs;
}

/**
 * Open an installed pack. Returns `null` when the pack is not installed.
 *
 * The returned object includes `pmtilesUrl` (ready for `createMap`) and a
 * `file(name)` helper for advanced callers that need the raw bytes.
 *
 * @param {string} id
 * @returns {Promise<object|null>}
 */
export async function openPack(id) {
  assertPackId(id);
  const packsDirectory = await getPacksDirectory({ create: false });
  if (!packsDirectory) return null;
  const metadata = await readMetadata(packsDirectory, id);
  if (!metadata) return null;

  const pack = toPublicPack(metadata);
  pack.file = async (name) => {
    assertFileName(name);
    const directory = await packsDirectory.getDirectoryHandle(id);
    const filesDirectory = await directory.getDirectoryHandle(FILES_SUBDIR);
    const handle = await filesDirectory.getFileHandle(name);
    return handle.getFile();
  };
  return pack;
}

/**
 * Delete an installed pack and any abandoned staging/backup directories for
 * that id. Returns `true` when an installed directory was removed.
 *
 * @param {string} id
 * @returns {Promise<boolean>}
 */
export async function deletePack(id) {
  assertPackId(id);
  const packsDirectory = await getPacksDirectory({ create: false });
  if (!packsDirectory) return false;

  let removed = false;
  for (const [name] of await listDirectory(packsDirectory)) {
    const isPack = name === id;
    const isTemporary =
      name.startsWith(`.staging-${id}-`) ||
      name.startsWith(`.old-${id}-`) ||
      name.startsWith(`.trash-${id}-`);
    if (!isPack && !isTemporary) continue;
    if (isPack) removed = true;
    await removeEntryQuietly(packsDirectory, name);
  }
  return removed;
}

async function sha256Hex(buffer) {
  if (!globalThis.crypto?.subtle) {
    fail("UNSUPPORTED", "WebCrypto is unavailable; SHA-256 verification cannot run.");
  }
  const digest = await globalThis.crypto.subtle.digest("SHA-256", buffer);
  const bytes = new Uint8Array(digest);
  let hex = "";
  for (const byte of bytes) hex += byte.toString(16).padStart(2, "0");
  return hex;
}

async function readResponseBytes(response, { signal, onChunk }) {
  if (!response.body) {
    const buffer = await response.arrayBuffer();
    onChunk(buffer.byteLength);
    return buffer;
  }

  const reader = response.body.getReader();
  const chunks = [];
  let received = 0;

  try {
    while (true) {
      throwIfAborted(signal);
      const { done, value } = await reader.read();
      if (done) break;
      if (!value || value.byteLength === 0) continue;
      chunks.push(value);
      received += value.byteLength;
      onChunk(value.byteLength);
    }
  } catch (error) {
    try {
      await reader.cancel();
    } catch {
      // Ignore cancel failures.
    }
    throw error;
  }

  const output = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    output.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return output.buffer;
}

async function estimateFreeBytes() {
  if (!navigator.storage?.estimate) return null;
  try {
    const { quota, usage } = await navigator.storage.estimate();
    if (!Number.isFinite(quota) || !Number.isFinite(usage)) return null;
    return Math.max(0, quota - usage);
  } catch {
    return null;
  }
}

/**
 * Download, verify and install an offline pack.
 *
 * @param {string} manifestUrl Manifest URL (absolute or relative to the page).
 * @param {object} [options]
 * @param {(progress: object) => void} [options.onProgress] Called for
 *   `manifest`, `download`, `verify`, `commit` and `done` phases. The payload
 *   includes `percent`, `receivedBytes`, `totalBytes`, `file`, `fileIndex`,
 *   `filesTotal` and `bytesPerSecond`.
 * @param {AbortSignal} [options.signal] Cancels the download. Staging files are
 *   removed and no pack is installed.
 * @param {number} [options.minFreeBytes] Refuse to start when the estimated
 *   free space minus this reserve is below the pack size. The production app
 *   should pass `500 * 1024 * 1024`; the demo passes 0 so a 94 KB fixture can
 *   install on small CI disks.
 * @returns {Promise<object>} Installed pack metadata (`openPack` shape).
 */
export async function downloadPack(manifestUrl, options = {}) {
  const { onProgress, signal, minFreeBytes = 0 } = options;
  const base = globalThis.location?.href ?? "http://localhost/";
  const absoluteManifestUrl = resolveUrl(manifestUrl, base, "manifestUrl");

  if (activeManifestUrls.has(absoluteManifestUrl)) {
    fail("BUSY", "This pack manifest is already being downloaded.");
  }

  let packsDirectory = null;
  let stagingName = null;
  let stagingDirectory = null;
  let id = null;
  let lastProgressAt = 0;
  let receivedBytes = 0;
  let totalBytes = 0;
  let startedAt = globalThis.performance?.now?.() ?? Date.now();
  let currentFile = null;
  let currentFileIndex = -1;
  let currentFilesTotal = 0;
  let version = null;

  const emit = (phase, extra = {}, force = false) => {
    if (typeof onProgress !== "function") return;
    const now = globalThis.performance?.now?.() ?? Date.now();
    if (!force && now - lastProgressAt < PROGRESS_INTERVAL_MS) return;
    lastProgressAt = now;
    const percent = totalBytes > 0 ? Math.min(100, (receivedBytes / totalBytes) * 100) : 0;
    const elapsedSeconds = Math.max(0.001, (now - startedAt) / 1000);
    try {
      onProgress({
        id,
        version,
        phase,
        file: currentFile,
        fileIndex: currentFileIndex,
        filesTotal: currentFilesTotal,
        receivedBytes,
        totalBytes,
        percent,
        bytesPerSecond: receivedBytes / elapsedSeconds,
        ...extra,
      });
    } catch {
      // A broken progress callback must not abort an already verified pack.
    }
  };

  activeManifestUrls.add(absoluteManifestUrl);
  try {
    throwIfAborted(signal);
    emit("manifest", { percent: 0 }, true);

    let manifestResponse;
    try {
      manifestResponse = await fetch(absoluteManifestUrl, {
        signal,
        credentials: "same-origin",
        cache: "no-store",
      });
    } catch (error) {
      if (isAbortError(error) || signal?.aborted) fail("CANCELLED", "Download cancelled");
      fail("NETWORK", `Could not fetch the pack manifest: ${error?.message ?? error}`, error);
    }

    if (!manifestResponse.ok) {
      fail(
        "NETWORK",
        `Could not fetch the pack manifest ${absoluteManifestUrl}: HTTP ${manifestResponse.status}`,
      );
    }

    let rawManifest;
    try {
      rawManifest = await manifestResponse.json();
    } catch (error) {
      if (isAbortError(error) || signal?.aborted) fail("CANCELLED", "Download cancelled");
      fail("MANIFEST_INVALID", "The pack manifest is not valid JSON", error);
    }

    const manifest = verifyManifest(rawManifest);
    id = manifest.id;
    version = manifest.version;
    totalBytes = manifest.totalBytes;
    currentFilesTotal = manifest.files.length;

    if (activePackIds.has(id)) fail("BUSY", `Pack ${id} is already being downloaded.`);
    activePackIds.add(id);

    const freeBytes = await estimateFreeBytes();
    const reserve = Number.isFinite(minFreeBytes) && minFreeBytes > 0 ? minFreeBytes : 0;
    if (freeBytes !== null && freeBytes < totalBytes + reserve) {
      fail(
        "QUOTA",
        `Not enough browser storage for ${id}: need ${totalBytes + reserve} bytes, about ${freeBytes} available.`,
      );
    }

    packsDirectory = await getPacksDirectory({ create: true });
    stagingName = `.staging-${id}-${randomId()}`;
    stagingDirectory = await packsDirectory.getDirectoryHandle(stagingName, { create: true });
    const filesDirectory = await stagingDirectory.getDirectoryHandle(FILES_SUBDIR, { create: true });

    for (let index = 0; index < manifest.files.length; index += 1) {
      throwIfAborted(signal);
      const file = manifest.files[index];
      currentFile = file.name;
      currentFileIndex = index;
      receivedBytes = manifest.files
        .slice(0, index)
        .reduce((sum, previous) => sum + previous.bytes, 0);

      emit("download", { fileBytes: file.bytes, receivedFileBytes: 0 }, true);

      const fileUrl = resolveUrl(file.url, absoluteManifestUrl, `files[${index}].url`);
      let response;
      try {
        response = await fetch(fileUrl, {
          signal,
          credentials: "same-origin",
        });
      } catch (error) {
        if (isAbortError(error) || signal?.aborted) fail("CANCELLED", "Download cancelled");
        fail("NETWORK", `Could not fetch ${file.name}: ${error?.message ?? error}`, error);
      }

      if (!response.ok) {
        fail("NETWORK", `Could not fetch ${file.name}: HTTP ${response.status}`);
      }

      const declaredHeader = response.headers.get("content-length");
      const declaredLength = declaredHeader === null ? Number.NaN : Number(declaredHeader);
      const contentEncoding = response.headers.get("content-encoding");
      if (
        Number.isFinite(declaredLength) &&
        declaredLength !== file.bytes &&
        (!contentEncoding || contentEncoding === "identity")
      ) {
        fail(
          "SIZE_MISMATCH",
          `${file.name} declared ${declaredLength} bytes but the manifest says ${file.bytes}.`,
        );
      }

      const buffer = await readResponseBytes(response, {
        signal,
        onChunk: (delta) => {
          receivedBytes += delta;
          emit("download");
        },
      });

      if (buffer.byteLength !== file.bytes) {
        fail(
          "SIZE_MISMATCH",
          `${file.name} downloaded ${buffer.byteLength} bytes but the manifest says ${file.bytes}.`,
        );
      }

      emit("verify", { fileBytes: file.bytes, receivedFileBytes: file.bytes }, true);
      const actualHash = await sha256Hex(buffer);
      if (actualHash !== file.sha256) {
        fail(
          "HASH_MISMATCH",
          `${file.name} failed SHA-256 verification (expected ${file.sha256}, got ${actualHash}).`,
        );
      }

      try {
        await writeFile(filesDirectory, file.name, buffer);
      } catch (error) {
        throw toPackError(error, `Could not write ${file.name} to offline storage`);
      }
    }

    const installedAt = new Date().toISOString();
    const {
      id: ignoredId,
      version: ignoredVersion,
      files: ignoredFiles,
      attribution,
      license,
      license_url,
      totalBytes: ignoredTotal,
      ...extras
    } = manifest;

    const metadata = {
      ...extras,
      schema: SCHEMA_VERSION,
      id,
      version,
      installedAt,
      source: absoluteManifestUrl,
      totalBytes,
      attribution,
      license,
      license_url,
      files: manifest.files.map((file) => ({
        name: file.name,
        url: file.url,
        bytes: file.bytes,
        sha256: file.sha256,
      })),
    };

    throwIfAborted(signal);
    try {
      await writeJson(stagingDirectory, METADATA_FILE, metadata);
    } catch (error) {
      throw toPackError(error, "Could not write the pack metadata");
    }

    emit("commit", { percent: 100 }, true);
    receivedBytes = totalBytes;

    const destination = await packsDirectory.getDirectoryHandle(id).catch((error) => {
      if (error?.name === "NotFoundError" || error?.name === "TypeMismatchError") return null;
      throw error;
    });
    if (destination) {
      // The new pack is fully downloaded and verified. Remove the old version
      // before moving the verified staging directory into place. A failed
      // move leaves no pack marked installed (metadata is transferred last).
      await removeEntryQuietly(packsDirectory, id);
    }

    try {
      await commitDirectory(packsDirectory, stagingDirectory, id);
    } catch (error) {
      await removeEntryQuietly(packsDirectory, id);
      throw toPackError(error, `Could not install pack ${id}`);
    }

    stagingName = null;
    stagingDirectory = null;

    const opened = await openPack(id);
    if (!opened) fail("STORAGE", `Pack ${id} was written but could not be read back.`);

    emit("done", { percent: 100, receivedBytes: totalBytes, totalBytes }, true);

    return {
      ...opened,
      installedAt,
      updated: Boolean(destination),
    };
  } catch (error) {
    throw toPackError(error);
  } finally {
    activeManifestUrls.delete(absoluteManifestUrl);
    if (id) activePackIds.delete(id);
    if (stagingName && packsDirectory) {
      await removeEntryQuietly(packsDirectory, stagingName);
    }
  }
}
