    import { createMap } from "../map/map.js";
    import {
      downloadPack,
      deletePack,
      listPacks,
      openPack,
      verifyManifest,
    } from "./offline.js";
    import { DEFAULT_PACK_CATALOG, findCatalogPack } from "./offline-catalog.js";

    const MAP_CENTER = [7.4246, 43.7384];
    const MAP_ZOOM = 13.2;

    const $ = (id) => document.getElementById(id);

    const state = {
      catalog: DEFAULT_PACK_CATALOG,
      selected: findCatalogPack("monaco") ?? DEFAULT_PACK_CATALOG[0],
      manifestLoading: false,
      manifestRequest: 0,
      manifests: null,
      packs: [],
      swReady: false,
      busy: false,
      controller: null,
      mapHandle: null,
      mapReady: false,
      mapError: null,
      lastInstall: null,
      lastDelete: null,
      lastError: null,
      progress: null,
      progressHistory: [],
    };

    const els = {
      swStatus: $("sw-status"),
      swStatusText: $("sw-status-text"),
      manifestName: $("manifest-name"),
      manifestVersion: $("manifest-version"),
      manifestSize: $("manifest-size"),
      manifestFiles: $("manifest-files"),
      manifestZoom: $("manifest-zoom"),
      manifestAttribution: $("manifest-attribution"),
      manifestLicense: $("manifest-license"),
      download: $("download"),
      cancel: $("cancel"),
      progress: $("progress"),
      progressBar: $("progress-bar"),
      progressFill: $("progress-fill"),
      progressLabel: $("progress-label"),
      progressPercent: $("progress-percent"),
      progressBytes: $("progress-bytes"),
      progressSpeed: $("progress-speed"),
      progressFile: $("progress-file"),
      storage: $("storage"),
      manualNote: $("manual-note"),
      catalog: $("catalog"),
      catalogCount: $("catalog-count"),
      packs: $("packs"),
      packsEmpty: $("packs-empty"),
      packCount: $("pack-count"),
      map: $("map"),
      mapEmpty: $("map-empty"),
      mapStatus: $("map-status"),
      mapStatusText: $("map-status-text"),
      error: $("error"),
      errorText: $("error-text"),
    };

    function formatBytes(bytes) {
      if (!Number.isFinite(bytes) || bytes < 0) return "—";
      if (bytes < 1000) return `${bytes} B`;
      const units = ["KB", "MB", "GB", "TB"];
      let value = bytes / 1000;
      let unit = 0;
      while (value >= 1000 && unit < units.length - 1) {
        value /= 1000;
        unit += 1;
      }
      const digits = value >= 100 ? 0 : value >= 10 ? 1 : 2;
      return `${value.toFixed(digits)} ${units[unit]}`;
    }

    function formatDate(iso) {
      if (!iso) return "unknown date";
      const date = new Date(iso);
      if (Number.isNaN(date.getTime())) return "unknown date";
      return new Intl.DateTimeFormat("en", { day: "numeric", month: "short", year: "numeric" }).format(date);
    }

    function setSwState(text, value) {
      els.swStatus.dataset.state = value;
      els.swStatusText.textContent = text;
    }

    function setMapStatus(text, value) {
      els.mapStatus.dataset.state = value;
      els.mapStatusText.textContent = text;
    }

    function showError(message) {
      state.lastError = message || null;
      els.error.hidden = !message;
      els.errorText.textContent = message || "";
    }

    function downloadLabel() {
      const selected = state.selected;
      if (!selected) return "Download pack";
      if (selected.manual) {
        return `Download ${selected.name} (${formatBytes(selected.approxBytes)})`;
      }
      return `Download ${selected.name} fixture`;
    }

    function setBusy(busy, label = "") {
      state.busy = busy;
      els.download.disabled = busy || !state.manifests;
      els.download.textContent = busy ? label || "Downloading…" : downloadLabel();
      els.cancel.hidden = !busy;
      els.progress.hidden = !busy && !state.progress;
      document.body.dataset.busy = busy ? "true" : "false";
    }

    function renderProgress(progress) {
      if (!progress) return;
      const percent = Math.max(0, Math.min(100, Number(progress.percent) || 0));
      const received = Number(progress.receivedBytes) || 0;
      const total = Number(progress.totalBytes) || 0;

      els.progress.hidden = false;
      els.progressFill.style.width = `${percent.toFixed(2)}%`;
      els.progressBar.setAttribute("aria-valuenow", percent.toFixed(0));
      els.progressPercent.textContent = `${percent.toFixed(0)}%`;
      els.progressBytes.textContent = `${formatBytes(received)} / ${formatBytes(total)}`;

      const speed = Number(progress.bytesPerSecond) || 0;
      els.progressSpeed.textContent = speed > 0 ? `${formatBytes(speed)}/s` : "—";

      const phase = progress.phase;
      const label = phase === "manifest"
        ? "Fetching manifest"
        : phase === "download"
          ? "Downloading"
          : phase === "verify"
            ? "Verifying SHA-256"
            : phase === "commit"
              ? "Installing"
              : phase === "done"
                ? "Installed"
                : "Preparing";
      els.progressLabel.textContent = progress.file && phase === "download"
        ? `${label} ${progress.file}`
        : label;
      els.progressFile.textContent = progress.file
        ? `${progress.file} · file ${Math.max(1, (progress.fileIndex ?? 0) + 1)} of ${progress.filesTotal ?? 0}`
        : "—";
    }

    async function refreshStorage() {
      if (!navigator.storage?.estimate) {
        els.storage.textContent = "Storage: not reported by this browser";
        return;
      }
      try {
        const { usage, quota } = await navigator.storage.estimate();
        els.storage.textContent = `Storage: ${formatBytes(usage ?? 0)} used of ${formatBytes(quota ?? 0)}`;
      } catch {
        els.storage.textContent = "Storage: unavailable";
      }
    }

    function renderCatalog() {
      els.catalog.replaceChildren();
      els.catalogCount.textContent = String(state.catalog.length);

      for (const entry of state.catalog) {
        const item = document.createElement("li");
        item.className = "catalog-item";
        item.dataset.catalogId = entry.id;
        item.dataset.selected = String(entry.id === state.selected?.id);

        const main = document.createElement("div");
        main.className = "catalog-main";

        const name = document.createElement("p");
        name.className = "catalog-name";
        name.textContent = entry.name;
        if (entry.kind === "fixture") {
          const tag = document.createElement("span");
          tag.className = "catalog-tag";
          tag.textContent = "test fixture";
          name.append(" ", tag);
        } else if (entry.manual) {
          const tag = document.createElement("span");
          tag.className = "catalog-tag";
          tag.textContent = "optional";
          name.append(" ", tag);
        }

        const meta = document.createElement("p");
        meta.className = "catalog-meta mono";
        const origin = entry.manual ? "same-origin proxy" : "local fixture";
        const mode = entry.manual ? "manual" : "CI tested";
        meta.textContent = `≈${formatBytes(entry.approxBytes)} · ${origin} · ${mode}`;

        main.append(name, meta);

        const select = document.createElement("button");
        select.type = "button";
        select.className = "button button-small button-ghost";
        select.dataset.action = "select";
        const isSelected = entry.id === state.selected?.id;
        select.textContent = isSelected ? "Selected" : "View";
        select.disabled = isSelected;
        select.addEventListener("click", () => selectCatalogPack(entry.id));

        item.append(main, select);
        els.catalog.append(item);
      }
    }

    function renderManifest() {
      const selected = state.selected;
      const manifest = state.manifests;

      if (!selected) {
        els.manifestName.textContent = "No catalog";
        els.manifestVersion.textContent = "Manifest unavailable";
        els.download.disabled = true;
        return;
      }

      els.manifestName.textContent = manifest?.name || selected.name;
      els.manifestVersion.textContent = manifest
        ? `v${manifest.version}`
        : state.manifestLoading
          ? "Loading manifest…"
          : "Manifest unavailable";
      els.manifestSize.textContent = manifest
        ? formatBytes(manifest.totalBytes)
        : `≈${formatBytes(selected.approxBytes)}`;
      els.manifestFiles.textContent = manifest
        ? `${manifest.files.length} file${manifest.files.length === 1 ? "" : "s"}`
        : "—";
      els.manifestZoom.textContent = manifest &&
        (Number.isFinite(manifest.minzoom) || Number.isFinite(manifest.maxzoom))
        ? `z${manifest.minzoom ?? "?"}–${manifest.maxzoom ?? "?"}`
        : manifest
          ? "vector"
          : "—";
      els.manifestAttribution.textContent = manifest
        ? `${manifest.attribution} / ${manifest.license}`
        : selected.manual
          ? "Optional region pack · attribution loads with the manifest"
          : "© OpenStreetMap contributors / ODbL";
      els.manifestLicense.textContent = manifest
        ? `License: ${manifest.license}`
        : selected.manual
          ? "License: ODbL-1.0 (from the proxy manifest)"
          : "License: ODbL-1.0";
      els.manualNote.hidden = !selected.manual;
      els.download.disabled = state.busy || !manifest;
      if (!state.busy) els.download.textContent = downloadLabel();
    }

    function renderPacks() {
      els.packs.replaceChildren();
      els.packsEmpty.hidden = state.packs.length > 0;
      els.packCount.textContent = String(state.packs.length);

      for (const pack of state.packs) {
        const item = document.createElement("li");
        item.className = "pack";
        item.dataset.packId = pack.id;

        const main = document.createElement("div");
        main.className = "pack-main";

        const dot = document.createElement("span");
        dot.className = "pack-dot";
        dot.setAttribute("aria-hidden", "true");

        const text = document.createElement("div");
        text.className = "pack-text";

        const name = document.createElement("p");
        name.className = "pack-name";
        name.textContent = pack.name || pack.id;

        const meta = document.createElement("p");
        meta.className = "pack-meta mono";
        meta.textContent = `v${pack.version} · ${formatBytes(pack.totalBytes)} · installed ${formatDate(pack.installedAt)}`;

        text.append(name, meta);
        main.append(dot, text);

        const actions = document.createElement("div");
        actions.className = "pack-actions";

        const open = document.createElement("button");
        open.type = "button";
        open.className = "button button-small";
        open.dataset.action = "open";
        open.textContent = "Open map";
        open.addEventListener("click", () => openPackMap(pack.id));

        const update = document.createElement("button");
        update.type = "button";
        update.className = "button button-small button-ghost";
        update.dataset.action = "update";
        update.textContent = "Update";
        update.addEventListener("click", () =>
          startDownload(
            pack.source || state.selected?.manifestUrl,
            findCatalogPack(pack.id)?.manual ?? false,
          ),
        );

        const remove = document.createElement("button");
        remove.type = "button";
        remove.className = "button button-small button-danger";
        remove.dataset.action = "delete";
        remove.textContent = "Delete";
        remove.addEventListener("click", () => removePack(pack.id));

        actions.append(open, update, remove);
        item.append(main, actions);
        els.packs.append(item);
      }
    }

    async function refreshPacks() {
      try {
        state.packs = await listPacks();
      } catch (error) {
        showError(error?.message || String(error));
        state.packs = [];
      }
      renderPacks();
      await refreshStorage();
    }

    async function selectCatalogPack(id) {
      if (state.busy) return;
      const entry = findCatalogPack(id);
      if (!entry) return;

      const request = ++state.manifestRequest;
      state.selected = entry;
      state.manifests = null;
      state.manifestLoading = true;
      state.progress = null;
      els.progress.hidden = true;
      showError("");
      renderCatalog();
      renderManifest();

      try {
        const response = await fetch(entry.manifestUrl, { cache: "no-store" });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const raw = await response.json();
        // Validate before showing sizes so the page never advertises a pack
        // that downloadPack would reject.
        const manifest = verifyManifest(raw);
        if (request !== state.manifestRequest) return;
        state.manifests = manifest;
        state.manifestLoading = false;
        renderManifest();
      } catch (error) {
        if (request !== state.manifestRequest) return;
        state.manifests = null;
        state.manifestLoading = false;
        renderManifest();
        showError(
          `Could not load the ${entry.name} manifest from ${entry.manifestUrl}: ${error?.message ?? error}`,
        );
      }
    }

    async function waitForController(timeoutMs = 5000) {
      if (navigator.serviceWorker.controller) return true;
      return await new Promise((resolve) => {
        const timer = setTimeout(() => resolve(Boolean(navigator.serviceWorker.controller)), timeoutMs);
        navigator.serviceWorker.addEventListener(
          "controllerchange",
          () => {
            clearTimeout(timer);
            resolve(true);
          },
          { once: true },
        );
      });
    }

    async function initServiceWorker() {
      if (!("serviceWorker" in navigator)) {
        setSwState("Service worker unsupported", "error");
        return false;
      }

      try {
        await navigator.serviceWorker.register("../sw.js");
        await navigator.serviceWorker.ready;
        const controlled = await waitForController();
        state.swReady = controlled;
        if (controlled) {
          setSwState("Service worker ready", "ready");
        } else {
          setSwState("Service worker activating…", "pending");
        }
        return controlled;
      } catch (error) {
        setSwState("Service worker unavailable", "error");
        showError(`Offline serving is unavailable: ${error?.message ?? error}`);
        return false;
      }
    }

    async function swStats() {
      const controller = navigator.serviceWorker.controller;
      if (!controller) return null;
      return await new Promise((resolve) => {
        const timer = setTimeout(() => resolve(null), 2000);
        const onMessage = (event) => {
          if (event.data?.type !== "PINPOINT_SW_STATS") return;
          clearTimeout(timer);
          navigator.serviceWorker.removeEventListener("message", onMessage);
          resolve(event.data.stats ?? null);
        };
        navigator.serviceWorker.addEventListener("message", onMessage);
        controller.postMessage({ type: "PINPOINT_SW_STATS" });
      });
    }

    async function openPackMap(id) {
      try {
        const pack = await openPack(id);
        if (!pack) {
          showError(`Pack ${id} is not installed.`);
          return;
        }
        if (!pack.pmtilesUrl) {
          showError(`Pack ${id} does not contain a PMTiles archive.`);
          return;
        }

        state.mapHandle?.destroy();
        state.mapHandle = null;
        state.mapToken = (state.mapToken || 0) + 1;
        const token = state.mapToken;
        state.mapReady = false;
        state.mapError = null;
        window.__offlineMapReady = false;
        window.__offlineMapError = null;

        els.mapEmpty.hidden = true;
        setMapStatus(`Loading ${pack.pmtilesUrl}`, "loading");

        state.mapHandle = createMap({
          container: "#map",
          styleUrl: "../map/style.json",
          pmtilesUrl: pack.pmtilesUrl,
          center: MAP_CENTER,
          zoom: MAP_ZOOM,
          onLoad: (handle) => {
            const map = handle.getMap();
            let settled = false;
            const markReady = () => {
              if (settled || state.mapHandle !== handle) return;
              settled = true;
              state.mapReady = true;
              window.__offlineMapReady = true;
              setMapStatus(`${pack.name || pack.id} · offline vector tiles`, "ready");
            };

            // MapLibre's `load` event means the style is ready; wait for the
            // first idle frame so the demo never claims the tiles rendered
            // before the PMTiles range requests have painted.
            setMapStatus("Loading tiles from OPFS…", "loading");
            if (map?.loaded()) markReady();
            else map?.once("idle", markReady);
          },
          onError: (error) => {
            if (state.mapToken !== token) return;
            const message = error?.message || String(error);
            state.mapError = message;
            window.__offlineMapError = message;
            setMapStatus(`Map error: ${message}`, "error");
            showError(`Map error: ${message}`);
          },
        });
      } catch (error) {
        const message = error?.message || String(error);
        state.mapError = message;
        window.__offlineMapError = message;
        setMapStatus(`Map error: ${message}`, "error");
        showError(`Map error: ${message}`);
        els.mapEmpty.hidden = false;
      }
    }

    async function startDownload(
      manifestUrl = state.selected?.manifestUrl,
      manual = state.selected?.manual ?? false,
    ) {
      if (state.busy || !manifestUrl) return;

      const selected = state.selected;
      const labelName = selected?.manifestUrl === manifestUrl ? selected.name : "pack";
      const reserve = manual ? 500 * 1024 * 1024 : 0;

      showError("");
      state.progress = null;
      state.progressHistory = [];
      state.controller = new AbortController();
      setBusy(true, `Downloading ${labelName}…`);
      els.progress.hidden = false;

      try {
        const installed = await downloadPack(manifestUrl, {
          signal: state.controller.signal,
          minFreeBytes: reserve,
          onProgress: (progress) => {
            state.progress = progress;
            if (state.progressHistory.length < 200) {
              state.progressHistory.push({
                phase: progress.phase,
                percent: progress.percent,
                receivedBytes: progress.receivedBytes,
                totalBytes: progress.totalBytes,
                file: progress.file,
              });
            }
            renderProgress(progress);
          },
        });

        // Publish the install only after the map has reopened and the pack list
        // has refreshed: lastInstall then means "ready to use".
        await openPackMap(installed.id);
        await refreshPacks();
        setBusy(false);
        state.lastInstall = {
          id: installed.id,
          version: installed.version,
          totalBytes: installed.totalBytes,
          files: installed.files.map((file) => file.name),
          pmtilesUrl: installed.pmtilesUrl,
          installedAt: installed.installedAt,
          updated: installed.updated,
        };
      } catch (error) {
        state.progress = null;
        els.progress.hidden = true;
        setBusy(false);
        if (error?.code === "CANCELLED") {
          setMapStatus("Download cancelled", "idle");
        } else {
          showError(`${error?.code ? `[${error.code}] ` : ""}${error?.message ?? error}`);
        }
        await refreshPacks();
      } finally {
        state.controller = null;
        setBusy(false);
      }
    }

    async function removePack(id) {
      try {
        const removed = await deletePack(id);
        state.lastDelete = { id, removed };
        if (removed && state.mapHandle) {
          state.mapHandle.destroy();
          state.mapHandle = null;
          state.mapToken = (state.mapToken || 0) + 1;
          state.mapReady = false;
          window.__offlineMapReady = false;
          els.mapEmpty.hidden = false;
          setMapStatus("Waiting for a pack", "idle");
        }
        await refreshPacks();
      } catch (error) {
        showError(`Could not delete ${id}: ${error?.message ?? error}`);
      }
    }

    els.download.addEventListener("click", () => startDownload());
    els.cancel.addEventListener("click", () => state.controller?.abort());
    window.addEventListener("resize", () => state.mapHandle?.resize());

    window.__offlineDemo = {
      ready: false,
      get packs() {
        return state.packs;
      },
      get manifest() {
        return state.manifests;
      },
      get manifestLoading() {
        return state.manifestLoading;
      },
      get swReady() {
        return state.swReady;
      },
      get progress() {
        return state.progress;
      },
      get progressHistory() {
        return state.progressHistory.slice();
      },
      get mapReady() {
        return state.mapReady;
      },
      get mapError() {
        return state.mapError;
      },
      get lastInstall() {
        return state.lastInstall;
      },
      get lastDelete() {
        return state.lastDelete;
      },
      get lastError() {
        return state.lastError;
      },
      get map() {
        return state.mapHandle;
      },
      get catalog() {
        return state.catalog;
      },
      get selected() {
        return state.selected;
      },
      selectCatalogPack,
      download: startDownload,
      open: openPackMap,
      remove: removePack,
      swStats,
    };

    async function boot() {
      renderCatalog();
      renderManifest();
      const controlled = await initServiceWorker();
      if (!controlled) {
        // The page can still download into OPFS, but pack URLs need the worker.
        setMapStatus("Service worker not active", "error");
      }
      await selectCatalogPack("monaco");
      await refreshPacks();
      window.__offlineDemo.ready = true;
    }

    boot().catch((error) => {
      showError(error?.message || String(error));
      window.__offlineDemo.ready = true;
    });
