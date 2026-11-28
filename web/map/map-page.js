// Map page controller (module). Moved out of index.html so the page CSP
// can drop 'unsafe-inline' from script-src.
    import { createMap } from "./map.js";
    import { createRouteUI } from "../app/route-ui.js";

    // The native shell passes a random per-launch token in the URL fragment.
    // Read it once, then drop it from the address bar and history so it is not
    // left in the URL. Mutating requests send it back in a header; the loopback
    // server rejects them without it.
    const MAP_TOKEN_HEADER = "X-Pinpoint-Map-Token";
    const mapToken = (() => {
      const match = /(?:^#|&)pinpoint-map-token=([^&]+)/.exec(location.hash);
      if (!match) return "";
      history.replaceState(null, "", location.pathname + location.search);
      return decodeURIComponent(match[1]);
    })();

    const MONACO = {
      id: "monaco",
      label: "Monaco fixture",
      pmtilesUrl: "./fixtures/monaco.pmtiles",
      center: [7.4246, 43.7384],
      zoom: 13.2,
      meta: "Monaco fixture · ODbL · 94 KB",
      ready: "Monaco fixture · vector tiles ready",
    };
    const MOLDOVA = {
      id: "moldova",
      label: "Moldova",
      pmtilesUrl: "/packs/moldova.pmtiles",
      center: [28.8638, 47.0105],
      zoom: 7.2,
      meta: "Moldova · z0–14 · ODbL",
      ready: "Moldova · vector tiles ready",
    };

    const ONLINE_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";
    const ONLINE_ORIGIN = "https://tiles.openfreemap.org";
    const ONLINE_TIMEOUT_MS = 10_000;
    const ONLINE_STORAGE_KEY = "pinpoint.onlineMap";
    const POLL_MS = 500;
    // The v1 catalog contract has no delete endpoint, so no Delete button is
    // rendered. Add one here only after the backend documents the route.

    // Real Moldova coordinates exercise the on-device graph.
    const DEFAULT_FROM = "47.0105, 28.8638"; // Chișinău
    const DEFAULT_TO = "47.76, 27.93"; // Bălți

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

    const statusEl = document.getElementById("status");
    const statusTextEl = document.getElementById("status-text");
    const mapMetaEl = document.getElementById("map-meta");
    const themeButton = document.getElementById("theme");
    const resetButton = document.getElementById("reset");
    const markersButton = document.getElementById("markers");
    const clickReadout = document.getElementById("click-readout");
    const offlinePanel = document.getElementById("offline-panel");
    const offlineBadge = document.getElementById("offline-badge");
    const offlineDetail = document.getElementById("offline-detail");
    const offlineGraph = document.getElementById("offline-graph");
    const offlineProgress = document.getElementById("offline-progress");
    const offlineDownload = document.getElementById("offline-download");
    const offlineError = document.getElementById("offline-error");
    const mapsPanel = document.getElementById("maps-panel");
    const mapsEmpty = document.getElementById("maps-empty");
    const mapsCoverage = document.getElementById("maps-coverage");
    const regionList = document.getElementById("region-list");
    const mapsProgress = document.getElementById("maps-progress");
    const mapsProgressText = document.getElementById("maps-progress-text");
    const mapsProgressBar = document.getElementById("maps-progress-bar");
    const mapsError = document.getElementById("maps-error");
    const onlineToggle = document.getElementById("online-map");
    const onlineError = document.getElementById("online-error");
    const onlineCredit = document.getElementById("online-credit");

    let theme = document.documentElement.dataset.theme === "dark" ? "dark" : "light";
    let markersVisible = true;
    let handle = null;
    let activeSource = null;
    let routeUI = null;
    let lastStatus = null;
    let pollTimer = null;
    let refreshInFlight = false;
    let reloadPromise = null;
    let online = false;
    let catalogMode = false;
    let catalogRegions = [];
    let configRegions = [];
    let activeDownloadRegion = null;
    let actionError = "";
    let onlineRequest = 0;

    function readOnlinePreference() {
      try {
        return localStorage.getItem(ONLINE_STORAGE_KEY) === "1";
      } catch {
        return false;
      }
    }

    function saveOnlinePreference(value) {
      try {
        localStorage.setItem(ONLINE_STORAGE_KEY, value ? "1" : "0");
      } catch {
        // Private browsing or storage disabled: the choice lasts this page.
      }
    }

    /**
     * Injected local router for createRouteUI. It fetches only this loopback
     * origin's `/map/route`; there is no online routing fallback.
     */
    async function requestLocalRoute({ from, to, profile }) {
      const params = new URLSearchParams({
        from_lat: String(from.lat),
        from_lon: String(from.lon),
        to_lat: String(to.lat),
        to_lon: String(to.lon),
        profile,
      });
      const response = await fetch(`/map/route?${params.toString()}`, {
        cache: "no-store",
        headers: { Accept: "application/json" },
      });

      let payload = null;
      try {
        payload = await response.json();
      } catch {
        payload = null;
      }

      if (!response.ok) {
        throw new Error(
          payload?.message || `Local routing failed (HTTP ${response.status}).`,
        );
      }
      if (!payload || !Array.isArray(payload.points)) {
        throw new Error("Local routing returned an invalid response.");
      }

      return {
        distance_m: payload.distance_m,
        duration_s: payload.duration_s,
        points: payload.points,
        maneuvers: payload.maneuvers,
      };
    }

    function setStatus(text, state) {
      statusTextEl.textContent = text;
      statusEl.dataset.state = state;
    }

    function syncThemeButton() {
      themeButton.textContent = theme === "dark" ? "Light mode" : "Dark mode";
      themeButton.setAttribute("aria-pressed", String(theme === "dark"));
    }

    function formatBytes(bytes) {
      const value = Number(bytes);
      if (!Number.isFinite(value) || value <= 0) return "0 KB";
      const mb = value / 1_000_000;
      if (mb >= 10) return `${Math.round(mb)} MB`;
      if (mb >= 1) return `${mb.toFixed(1)} MB`;
      return `${Math.max(1, Math.round(value / 1_000))} KB`;
    }

    async function fetchJson(url, options) {
      const response = await fetch(url, {
        cache: "no-store",
        ...options,
      });
      if (!response.ok) {
        const error = new Error(`HTTP ${response.status} from ${url}`);
        error.status = response.status;
        throw error;
      }
      return response.json();
    }

    /** Config is served by the Rust map servers; a static preview falls back. */
    async function fetchConfig() {
      try {
        const config = await fetchJson("/map/config");
        return {
          pmtilesUrl: String(config?.pmtilesUrl || MONACO.pmtilesUrl),
          graphInstalled: config?.graphInstalled === true,
          packInstalled: config?.packInstalled === true,
          regions: Array.isArray(config?.regions) ? config.regions : [],
        };
      } catch {
        return {
          pmtilesUrl: MONACO.pmtilesUrl,
          graphInstalled: false,
          packInstalled: false,
          regions: [],
        };
      }
    }

    /** The catalog is optional: a 404 keeps the single Moldova panel. */
    async function fetchCatalog() {
      try {
        const catalog = await fetchJson("/map/catalog");
        if (!catalog || !Array.isArray(catalog.regions)) return null;
        return catalog.regions.filter(
          (region) =>
            region &&
            typeof region.id === "string" &&
            region.id.length > 0 &&
            typeof region.name === "string",
        );
      } catch {
        return null;
      }
    }

    function regionCenter(bbox) {
      const values = Array.isArray(bbox) ? bbox.map(Number) : [];
      if (values.length !== 4 || values.some((value) => !Number.isFinite(value))) {
        return null;
      }
      const [west, south, east, north] = values;
      const spanLon = Math.max(0.01, east - west);
      return {
        center: [(west + east) / 2, (south + north) / 2],
        zoom: Math.max(
          2,
          Math.min(13.2, Math.round(Math.log2(360 / spanLon) * 10) / 10),
        ),
      };
    }

    /** First catalog region whose bbox contains the point. */
    function regionCovering(regions, lon, lat) {
      // shortcut: plain bbox containment, so an antimeridian region (US) needs
      // a split bbox before it can be matched here.
      return (
        regions.find((region) => {
          const values = Array.isArray(region?.bbox) ? region.bbox.map(Number) : [];
          if (values.length !== 4 || values.some((value) => !Number.isFinite(value))) {
            return false;
          }
          const [west, south, east, north] = values;
          return lon >= west && lon <= east && lat >= south && lat <= north;
        }) ?? null
      );
    }

    function sourceForRegion(region, regions = configRegions) {
      const configRegion = regions.find((entry) => entry.id === region.id);
      const frame = regionCenter(configRegion?.bbox || region.bbox) || {
        center: MOLDOVA.center,
        zoom: MOLDOVA.zoom,
      };
      const basemap = (Array.isArray(region.files) ? region.files : []).find(
        (file) => file?.role === "basemap",
      );
      const fileName =
        basemap?.installed_name || basemap?.installedName || `${region.id}.pmtiles`;
      return {
        id: region.id,
        label: region.name,
        pmtilesUrl: String(
          configRegion?.pmtilesUrl || region.pmtilesUrl || `/packs/${region.id}/${fileName}`,
        ),
        center: frame.center,
        zoom: frame.zoom,
        meta: `${region.name}${region.version ? ` · ${region.version}` : ""} · ODbL`,
        ready: `${region.name} · vector tiles ready`,
      };
    }

    /** Default to the installed region the server selects in /map/config. */
    function pickInitialSource(config) {
      const installed =
        config.regions.find(
          (region) =>
            region.installed === true && region.pmtilesUrl === config.pmtilesUrl,
        ) || config.regions.find((region) => region.installed === true);
      if (installed) return sourceForRegion(installed, config.regions);
      if (config.packInstalled) return { ...MOLDOVA, pmtilesUrl: config.pmtilesUrl };
      return MONACO;
    }

    /** Only the one OpenFreeMap host may appear in the online style. */
    function usesOnlyOpenFreeMap(style) {
      const urls = [];
      if (typeof style.sprite === "string") urls.push(style.sprite);
      if (typeof style.glyphs === "string") urls.push(style.glyphs);
      for (const source of Object.values(style.sources ?? {})) {
        if (!source) continue;
        if (typeof source.url === "string") urls.push(source.url);
        if (Array.isArray(source.tiles)) urls.push(...source.tiles);
      }
      return urls.every((url) => {
        try {
          return new URL(String(url), ONLINE_STYLE_URL).origin === ONLINE_ORIGIN;
        } catch {
          return false;
        }
      });
    }

    /**
     * Fetch the online style once, before MapLibre sees it. The 10s abort keeps
     * a dead network from blocking the toggle; a failure leaves the local
     * PMTiles style rendered and is not retried automatically.
     */
    async function fetchOnlineStyle() {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), ONLINE_TIMEOUT_MS);
      try {
        const response = await fetch(ONLINE_STYLE_URL, {
          signal: controller.signal,
          headers: { Accept: "application/json" },
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const style = await response.json();
        if (!style || style.version !== 8 || !usesOnlyOpenFreeMap(style)) {
          throw new Error("unexpected online style");
        }
        return style;
      } finally {
        clearTimeout(timer);
      }
    }

    function syncOnlineUi() {
      onlineToggle.checked = online;
      onlineCredit.hidden = !online;
    }

    function showOnlineError(message) {
      onlineError.textContent = message;
      onlineError.hidden = !message;
    }

    /** The first local style must finish before a style switch can apply. */
    function waitForMapReady(timeout = 15000) {
      if (window.__pinpointMapReady === true) return Promise.resolve();
      return new Promise((resolve, reject) => {
        const started = Date.now();
        const timer = setInterval(() => {
          if (window.__pinpointMapReady === true) {
            clearInterval(timer);
            resolve();
          } else if (
            typeof window.__pinpointMapError === "string" ||
            Date.now() - started > timeout
          ) {
            clearInterval(timer);
            reject(new Error(window.__pinpointMapError || "The map did not load."));
          }
        }, 100);
      });
    }

    /**
     * Opt in to the OpenFreeMap Liberty style. The style is fetched and checked
     * before MapLibre sees it, so a network failure keeps the offline map. One
     * attempt per toggle; there is no retry loop.
     */
    async function setOnline(enabled, { persist = true } = {}) {
      const request = ++onlineRequest;
      if (!handle) {
        online = false;
        syncOnlineUi();
        return;
      }

      showOnlineError("");

      try {
        await waitForMapReady();
      } catch {
        online = false;
        syncOnlineUi();
        showOnlineError("Online map unavailable. Using the offline map.");
        return;
      }

      if (!enabled) {
        if (!activeSource) return;
        setStatus("Restoring offline map…", "loading");
        try {
          await handle.setStyle("./style.json", activeSource.pmtilesUrl);
          online = false;
          if (persist) saveOnlinePreference(false);
          syncOnlineUi();
          setStatus(activeSource.ready, "ready");
          mapMetaEl.textContent = activeSource.meta;
        } catch (error) {
          syncOnlineUi();
          showOnlineError("Could not restore the offline style.");
          setStatus(`Map error: ${error?.message || error}`, "error");
        }
        return;
      }

      setStatus("Loading online map…", "loading");
      let style = null;
      try {
        style = await fetchOnlineStyle();
      } catch {
        if (request !== onlineRequest) return;
        online = false;
        if (persist) saveOnlinePreference(false);
        syncOnlineUi();
        showOnlineError("Online map unavailable. Using the offline map.");
        setStatus(activeSource?.ready || "Offline map ready", "ready");
        return;
      }

      if (request !== onlineRequest) return;

      try {
        await handle.setStyle(style);
        if (request !== onlineRequest) return;
        online = true;
        if (persist) saveOnlinePreference(true);
        syncOnlineUi();
        setStatus("Online map · OpenFreeMap Liberty", "ready");
        mapMetaEl.textContent = "OpenFreeMap Liberty · worldwide";
      } catch (error) {
        online = false;
        if (persist) saveOnlinePreference(false);
        syncOnlineUi();
        let restored = false;
        if (activeSource) {
          try {
            await handle.setStyle("./style.json", activeSource.pmtilesUrl);
            restored = true;
          } catch {
            // Report the failed switch below; the local style is the fallback.
          }
        }
        showOnlineError(
          restored || !activeSource
            ? "Online map unavailable. Using the offline map."
            : "Online map could not be applied.",
        );
        if (restored) {
          setStatus(activeSource.ready, "ready");
          mapMetaEl.textContent = activeSource.meta;
        } else {
          setStatus(`Map error: ${error?.message || error}`, "error");
        }
      }
    }

    function createPreviewMap(source) {
      setStatus(`Loading ${source.label}…`, "loading");
      window.__pinpointMapReady = false;
      mapMetaEl.textContent = source.meta;

      let created = null;
      created = createMap({
        container: "#map",
        styleUrl: "./style.json",
        pmtilesUrl: source.pmtilesUrl,
        center: source.center,
        zoom: source.zoom,
        onLoad: (loadedHandle) => {
          if (created !== handle) return;
          setStatus(source.ready, "ready");
          window.__pinpointMapReady = true;
          handle?.setTheme(theme);
          handle?.setLocations(markersVisible ? SAMPLE_LOCATIONS : []);
          const route = routeUI?.getRoute?.();
          if (route) loadedHandle.drawRoute(route);
          loadedHandle.getMap()?.on("moveend", () => renderCoverage());
          renderCoverage();
        },
        onError: (error) => {
          if (created !== handle) return;
          const message = error?.message || String(error);
          // MapLibre cancels in-flight PMTiles reads when the page replaces a
          // map after installing the pack. That is not a map error.
          if (/abort|cancel/i.test(message)) return;
          setStatus(`Map error: ${message}`, "error");
          window.__pinpointMapError = message;
        },
      });
      return created;
    }

    /** Replace the map when the install changes the PMTiles URL. */
    function installMap(source) {
      const previous = handle;
      handle = null;
      if (previous) previous.destroy();
      activeSource = source;
      handle = createPreviewMap(source);
      handle.onClick(({ lat, lon }) => {
        clickReadout.textContent = `${lat.toFixed(4)}, ${lon.toFixed(4)}`;
      });
      window.__pinpointPreview = {
        handle,
        locations: SAMPLE_LOCATIONS,
        get source() {
          return activeSource;
        },
        get online() {
          return online;
        },
        get map() {
          return handle?.getMap() ?? null;
        },
      };
    }

    function successState(status) {
      return status?.pack?.installed === true && status?.graph?.installed === true;
    }

    function runningState(status) {
      const phases = [status?.pack?.phase, status?.graph?.phase];
      return phases.includes("downloading") || phases.includes("verifying");
    }

    function regionDownloadBytes(region) {
      const files = Array.isArray(region.files) ? region.files : [];
      return files.reduce(
        (total, file) => total + Number(file?.transfer_bytes || 0),
        0,
      );
    }

    function regionState(region) {
      if (region.installed !== true) return "idle";
      if (
        region.version &&
        region.installed_version &&
        region.installed_version !== region.version
      ) {
        return "update";
      }
      return "installed";
    }

    function addRegionButton(actions, region, action, label, ariaLabel, disabled = false) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "region-button";
      button.dataset.action = action;
      button.dataset.region = region.id;
      button.textContent = label;
      button.setAttribute("aria-label", ariaLabel);
      button.disabled = disabled;
      button.addEventListener("click", () => {
        if (action === "show") showRegion(region);
        else void startRegionDownload(region.id);
      });
      actions.append(button);
    }

    /**
     * "This area" hint: names the catalog region under the view centre and
     * offers the same action as its row. Hidden while a download runs, when
     * nothing covers the centre and when that region is already active.
     */
    function renderCoverage(center = handle?.getMap()?.getCenter()) {
      const region = center
        ? regionCovering(catalogRegions, Number(center.lng), Number(center.lat))
        : null;
      if (
        !region ||
        runningState(lastStatus) ||
        activeDownloadRegion ||
        (!online && activeSource?.id === region.id)
      ) {
        mapsCoverage.hidden = true;
        mapsCoverage.replaceChildren();
        return;
      }

      const state = regionState(region);
      const bytes = regionDownloadBytes(region);
      const sizeLabel = bytes > 0 ? formatBytes(bytes) : "Size unknown";
      const text = document.createElement("span");
      text.textContent = `${region.name} covers this view · ${sizeLabel}`;
      mapsCoverage.replaceChildren(text);

      if (state === "installed") {
        addRegionButton(
          mapsCoverage,
          region,
          "show",
          "Show",
          `Show ${region.name} on the map`,
        );
      } else if (state === "update") {
        addRegionButton(
          mapsCoverage,
          region,
          "update",
          "Update",
          `Update ${region.name} offline map to ${region.version || "the latest version"}`,
        );
      } else {
        addRegionButton(
          mapsCoverage,
          region,
          "download",
          "Download",
          `Download ${region.name} offline map (${sizeLabel})`,
        );
      }
      mapsCoverage.hidden = false;
    }

    function renderRegions() {
      regionList.replaceChildren();
      mapsEmpty.hidden = catalogRegions.length > 0;
      mapsEmpty.textContent = catalogRegions.length
        ? ""
        : "No offline regions available.";

      for (const region of catalogRegions) {
        const state = regionState(region);
        const bytes = regionDownloadBytes(region);
        const sizeLabel = bytes > 0 ? formatBytes(bytes) : "Size unknown";
        const item = document.createElement("li");
        item.className = "region";
        item.dataset.region = region.id;
        item.dataset.state = state;

        const info = document.createElement("div");
        info.className = "region__info";

        const name = document.createElement("span");
        name.className = "region__name";
        name.textContent = region.name;

        const size = document.createElement("span");
        size.className = "region__size";
        size.textContent = sizeLabel;

        const status = document.createElement("span");
        status.className = "region__state";
        status.textContent =
          state === "installed"
            ? "Installed"
            : state === "update"
              ? `Update available${region.version ? ` · ${region.version}` : ""}`
              : "Not installed";

        info.append(name, size, status);
        item.append(info);

        const actions = document.createElement("div");
        actions.className = "region__actions";
        if (state === "idle") {
          addRegionButton(
            actions,
            region,
            "download",
            "Download",
            `Download ${region.name} offline map (${sizeLabel})`,
          );
        }
        if (state === "update") {
          addRegionButton(
            actions,
            region,
            "update",
            "Update",
            `Update ${region.name} offline map to ${region.version || "the latest version"}`,
          );
        }
        if (state !== "idle") {
          addRegionButton(
            actions,
            region,
            "show",
            "Show",
            `Show ${region.name} on the map`,
            !online && activeSource?.id === region.id,
          );
        }
        // Delete needs a documented backend route; the v1 catalog has none,
        // so no Delete button is offered.
        item.append(actions);
        regionList.append(item);
      }
      renderCoverage();
    }

    function showRegion(region) {
      if (online) {
        onlineRequest += 1;
        online = false;
        saveOnlinePreference(false);
        syncOnlineUi();
        showOnlineError("");
      }
      installMap(sourceForRegion(region));
      renderRegions();
    }

    async function startRegionDownload(regionId) {
      const region = catalogRegions.find((entry) => entry.id === regionId);
      activeDownloadRegion = regionId;
      actionError = "";
      mapsError.hidden = true;
      mapsPanel.dataset.state = "downloading";
      if (region) mapsProgressText.textContent = `Starting ${region.name}…`;

      try {
        const response = await fetch("/map/offline/download", {
          method: "POST",
          cache: "no-store",
          headers: {
            "Content-Type": "application/json",
            Accept: "application/json",
            [MAP_TOKEN_HEADER]: mapToken,
          },
          body: JSON.stringify({ region: regionId }),
        });
        let payload = null;
        try {
          payload = await response.json();
        } catch {
          payload = null;
        }
        if (response.status !== 202 && response.status !== 409) {
          throw new Error(
            payload?.message || `Download failed (HTTP ${response.status}).`,
          );
        }
        if (payload) renderMaps(payload);
        schedulePoll(POLL_MS);
      } catch (error) {
        activeDownloadRegion = null;
        actionError = error?.message || "The download could not start.";
        mapsError.hidden = false;
        mapsError.textContent = `${region?.name || regionId}: ${actionError}`;
        mapsPanel.dataset.state = "error";
        renderCoverage();
      }
    }

    function markRegionInstalled(regionId) {
      const region = catalogRegions.find((entry) => entry.id === regionId);
      if (!region) return;
      const changed =
        region.installed !== true || region.installed_version !== region.version;
      region.installed = true;
      region.installed_version = region.version;
      activeDownloadRegion = null;
      if (!changed) return;
      renderRegions();
      void adoptInstalledRegion(region);
    }

    /** Point config and, offline, the map at a freshly installed region. */
    function adoptInstalledRegion(region) {
      if (reloadPromise) return reloadPromise;
      reloadPromise = (async () => {
        const config = await fetchConfig();
        configRegions = config.regions;
        const source = sourceForRegion(region, config.regions);
        activeSource = source;
        if (!online) installMap(source);
        renderRegions();
      })().finally(() => {
        reloadPromise = null;
      });
      return reloadPromise;
    }

    function renderMaps(status) {
      lastStatus = status;
      const pack = status?.pack ?? {};
      const graph = status?.graph ?? {};
      const running = runningState(status);
      const phases = [pack.phase, graph.phase];
      const phase = phases.includes("verifying")
        ? "Verifying"
        : phases.includes("downloading")
          ? "Downloading"
          : "";
      const regionId = String(status?.region || activeDownloadRegion || "");
      const region = catalogRegions.find((entry) => entry.id === regionId);
      const expected =
        Number(status?.expected_download_bytes || 0) ||
        Number(pack.download_bytes || 0) + Number(graph.download_bytes || 0);
      const done =
        Number(pack.downloaded_bytes || 0) + Number(graph.downloaded_bytes || 0);

      if (running) {
        const label = region?.name || regionId || "Offline map";
        mapsProgress.hidden = false;
        mapsProgressText.textContent = `${phase || "Downloading"} ${label} · ${formatBytes(done)} of ${formatBytes(expected)}`;
        mapsProgressBar.max = Math.max(1, expected);
        mapsProgressBar.value = Math.min(expected || 1, done);
        mapsProgressBar.setAttribute("aria-label", `${label} download progress`);
      } else {
        mapsProgress.hidden = true;
        mapsProgressBar.value = 0;
      }

      const statusError = String(pack.error || graph.error || "");
      const message =
        actionError ||
        (statusError
          ? `${region?.name ? `${region.name}: ` : ""}${statusError}`
          : "");
      mapsError.hidden = !message;
      mapsError.textContent = message;
      mapsPanel.dataset.state = message ? "error" : running ? "downloading" : "ready";

      if (
        !running &&
        pack.installed === true &&
        graph.installed === true &&
        regionId
      ) {
        markRegionInstalled(regionId);
      }
      renderCoverage();
    }

    function renderStatus(status) {
      if (catalogMode) renderMaps(status);
      else renderOffline(status);
    }

    function renderOfflineUnavailable() {
      offlinePanel.dataset.state = "unavailable";
      offlineBadge.textContent = "Local server only";
      offlineDetail.textContent = "Install state is unavailable on this preview server.";
      offlineGraph.textContent = "Routing graph: local server only";
      offlineGraph.dataset.installed = "false";
      offlineProgress.hidden = true;
      offlineDownload.hidden = true;
      offlineError.hidden = true;
    }

    function renderOffline(status) {
      lastStatus = status;
      const pack = status?.pack ?? {};
      const graph = status?.graph ?? {};
      const installedBytes =
        Number(pack.expected_bytes || 0) + Number(graph.expected_bytes || 0);
      // The graph is transferred gzip-compressed, so the download total is
      // smaller than the installed size. Always compute the label from the
      // status the server reports instead of hard-coding a size.
      const expected =
        Number(pack.download_bytes || 0) + Number(graph.download_bytes || 0);
      const progress =
        Number(pack.downloaded_bytes || 0) + Number(graph.downloaded_bytes || 0);
      const installed = pack.installed === true && graph.installed === true;
      const running = runningState(status);
      const error = String(pack.error || graph.error || "");
      const totalLabel = formatBytes(expected);
      const diskLabel = formatBytes(installedBytes);

      // The static preview server reports expected_bytes: 0 because it has no
      // Rust download manager. Keep the card honest instead of offering a
      // download that cannot work there.
      if (expected === 0 && !installed && !running && !error) {
        offlinePanel.dataset.state = "unavailable";
        offlineBadge.textContent = "Preview";
        offlineDetail.textContent = "Open the app's map window to install offline data.";
        offlineGraph.textContent = "Routing graph: app server only";
        offlineGraph.dataset.installed = "false";
        offlineProgress.hidden = true;
        offlineDownload.hidden = true;
        offlineError.hidden = true;
        return;
      }

      const installedWithError = installed && Boolean(error);
      offlinePanel.dataset.state =
        installedWithError || (!installed && !running && error)
          ? "error"
          : installed
            ? "installed"
            : running
              ? "downloading"
              : "idle";

      if (installedWithError) {
        offlineBadge.textContent = "Update failed";
        offlineDetail.textContent = "The installed offline map was kept.";
      } else if (installed) {
        offlineBadge.textContent = "Installed";
        offlineDetail.textContent = `Moldova · ${diskLabel} on disk`;
      } else if (running) {
        const phase = [pack.phase, graph.phase].includes("verifying")
          ? "Verifying"
          : "Downloading";
        offlineBadge.textContent = phase;
        offlineDetail.textContent = `${phase} ${formatBytes(progress)} of ${totalLabel}`;
      } else if (error) {
        offlineBadge.textContent = "Failed";
        offlineDetail.textContent = "The offline map was not installed.";
      } else {
        offlineBadge.textContent = "Not installed";
        offlineDetail.textContent = `Moldova map and routing graph · ${totalLabel}`;
      }

      offlineGraph.textContent = graph.installed
        ? "Routing graph installed"
        : "Download routing data";
      offlineGraph.dataset.installed = graph.installed ? "true" : "false";

      offlineProgress.hidden = !running;
      offlineProgress.max = Math.max(1, expected);
      offlineProgress.value = Math.min(expected || 1, progress);
      offlineProgress.dataset.progress = String(progress);
      offlineProgress.dataset.expected = String(expected);

      offlineDownload.hidden = (installed && !error) || running;
      offlineDownload.disabled = running;
      offlineDownload.textContent = error
        ? `Retry download (${totalLabel})`
        : `Download Moldova map (${totalLabel})`;

      offlineError.hidden = !error;
      offlineError.textContent = error;

      if (installed && activeSource?.pmtilesUrl !== MOLDOVA.pmtilesUrl) {
        void reloadWithInstalledPack();
      }
    }

    async function refreshStatus({ render = true } = {}) {
      if (refreshInFlight) return lastStatus;
      refreshInFlight = true;
      try {
        const status = await fetchJson("/map/offline/status");
        if (render) renderStatus(status);
        return status;
      } catch {
        if (render && !lastStatus) {
          if (catalogMode) {
            mapsPanel.dataset.state = "error";
            mapsError.hidden = false;
            mapsError.textContent = "Install state is unavailable on this server.";
          } else {
            renderOfflineUnavailable();
          }
        }
        return null;
      } finally {
        refreshInFlight = false;
      }
    }

    function reloadWithInstalledPack() {
      if (reloadPromise) return reloadPromise;
      reloadPromise = (async () => {
        const config = await fetchConfig();
        if (!config.packInstalled) return;
        if (activeSource?.pmtilesUrl !== MOLDOVA.pmtilesUrl) {
          installMap(MOLDOVA);
        }
      })().finally(() => {
        reloadPromise = null;
      });
      return reloadPromise;
    }

    function schedulePoll(delay = POLL_MS) {
      if (pollTimer) clearTimeout(pollTimer);
      pollTimer = setTimeout(async () => {
        pollTimer = null;
        const status = await refreshStatus();
        if (status && runningState(status)) {
          schedulePoll();
        } else if (!catalogMode && status && successState(status)) {
          await reloadWithInstalledPack();
          if (lastStatus && successState(lastStatus)) {
            offlineBadge.textContent = "Installed";
            offlineDetail.textContent = `Moldova · ${formatBytes(
              Number(lastStatus.pack.expected_bytes || 0) +
                Number(lastStatus.graph.expected_bytes || 0),
            )} on disk`;
          }
        }
      }, delay);
    }

    async function startLegacyDownload() {
      offlineDownload.disabled = true;
      offlineError.hidden = true;
      offlineBadge.textContent = "Starting…";
      try {
        const response = await fetch("/map/offline/download", {
          method: "POST",
          cache: "no-store",
          headers: { [MAP_TOKEN_HEADER]: mapToken },
        });
        let payload = null;
        try {
          payload = await response.json();
        } catch {
          payload = null;
        }
        if (response.status !== 202 && response.status !== 409) {
          throw new Error(
            payload?.message || `Download failed (HTTP ${response.status}).`,
          );
        }
        if (payload) renderOffline(payload);
        schedulePoll(120);
      } catch (error) {
        offlineDownload.disabled = false;
        offlinePanel.dataset.state = "error";
        offlineBadge.textContent = "Failed";
        offlineDetail.textContent = "The offline map was not downloaded.";
        offlineError.hidden = false;
        offlineError.textContent =
          error?.message || "The offline map download could not start.";
      }
    }

    function setupRouteUI() {
      routeUI = createRouteUI({
        container: "#route-panel",
        route: requestLocalRoute,
        onRoute: ({ route }) => {
          handle?.drawRoute(route);
        },
        onCancel: ({ reason }) => {
          if (reason === "clear") handle?.clearRoute();
        },
      });

      const fromInput = routeUI.element.querySelector('input[name="from"]');
      const toInput = routeUI.element.querySelector('input[name="to"]');
      if (fromInput) fromInput.value = DEFAULT_FROM;
      if (toInput) toInput.value = DEFAULT_TO;

      for (const button of routeUI.element.querySelectorAll(".pinpoint-route__profile")) {
        if (button.dataset.profile === "auto") continue;
        button.disabled = true;
        button.title = "Only the offline car profile is available.";
      }

      window.__pinpointRoute = {
        ui: routeUI,
        route: requestLocalRoute,
        defaults: { from: DEFAULT_FROM, to: DEFAULT_TO },
      };
    }

    async function boot() {
      syncThemeButton();
      syncOnlineUi();

      const [config, catalog] = await Promise.all([fetchConfig(), fetchCatalog()]);
      configRegions = config.regions;
      catalogMode = catalog !== null;
      catalogRegions = catalog || [];

      mapsPanel.hidden = !catalogMode;
      offlinePanel.hidden = catalogMode;
      if (catalogMode) renderRegions();

      installMap(pickInitialSource(config));
      setupRouteUI();

      const status = await refreshStatus();
      if (status && runningState(status)) schedulePoll(POLL_MS);

      if (readOnlinePreference()) void setOnline(true);

      window.__pinpointMaps = {
        get regions() {
          return catalogRegions;
        },
        get online() {
          return online;
        },
        refresh: () => refreshStatus(),
        panel: mapsPanel,
        formatBytes,
        coverageAt: (lon, lat) => renderCoverage({ lng: lon, lat }),
      };

      window.__pinpointOffline = {
        get status() {
          return lastStatus;
        },
        refresh: () => refreshStatus(),
        panel: offlinePanel,
        button: offlineDownload,
        formatBytes,
      };
    }

    themeButton.addEventListener("click", () => {
      theme = theme === "dark" ? "light" : "dark";
      document.documentElement.dataset.theme = theme;
      try {
        localStorage.setItem("pinpoint-map-theme", theme);
      } catch {
        // Private browsing or storage disabled: theme still applies for this page.
      }
      handle?.setTheme(theme);
      syncThemeButton();
    });

    resetButton.addEventListener("click", () => {
      if (!activeSource) return;
      const view = online
        ? { center: [0, 0], zoom: 1.4 }
        : { center: activeSource.center, zoom: activeSource.zoom };
      handle?.getMap()?.flyTo({ ...view, duration: 650 });
    });

    markersButton.addEventListener("click", () => {
      markersVisible = !markersVisible;
      handle?.setLocations(markersVisible ? SAMPLE_LOCATIONS : []);
      markersButton.textContent = markersVisible ? "Hide markers" : "Show markers";
      markersButton.setAttribute("aria-pressed", String(markersVisible));
    });

    onlineToggle.addEventListener("change", () => {
      void setOnline(onlineToggle.checked);
    });

    offlineDownload.addEventListener("click", () => {
      void startLegacyDownload();
    });

    void boot();
