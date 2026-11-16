/**
 * Pinpoint offline search UI component.
 *
 * A dependency-free, network-free search box and result list that can be
 * dropped into the web app or the shared map page. The caller injects the
 * search function, so the same component works with the SQLite WASM search
 * bridge, an in-memory fixture or any other local provider. This module never
 * calls `fetch` and loads no remote assets.
 *
 * The caller loads `search.css` separately.
 */

const DEFAULT_PLACEHOLDER = "Search a place…";
const SVG_NS = "http://www.w3.org/2000/svg";

let nextInstance = 1;

/**
 * @typedef {object} SearchResult
 * @property {string} name Display name.
 * @property {string} kind Place category, for example "city" or "town".
 * @property {number} lat Latitude in WGS 84 degrees.
 * @property {number} lon Longitude in WGS 84 degrees.
 * @property {string} address Human-readable address or locality.
 * @property {number} importance Provider ranking score.
 */

/**
 * Create a search box with a debounced, keyboard-accessible result list.
 *
 * @param {object} options
 * @param {HTMLElement|string} options.container Element (or selector) that the
 *   component mounts into. The component appends its own wrapper and removes
 *   only that wrapper on `destroy`.
 * @param {(query: string) => Promise<SearchResult[]>} options.search Injected
 *   local search provider. It receives the trimmed query and must resolve to
 *   an array in the `SearchResult` shape. It may throw or reject; the component
 *   shows its error state and calls `onError`.
 * @param {(result: SearchResult) => void} [options.onSelect] Called with the
 *   original provider object when the user clicks a result or presses Enter.
 * @param {(error: unknown) => void} [options.onError] Optional diagnostic hook
 *   for provider failures. The user-facing message stays generic.
 * @param {string} [options.placeholder] Input placeholder.
 * @param {number} [options.debounceMs] Input debounce in milliseconds.
 * @param {"light"|"dark"} [options.theme] Optional explicit theme. Without it
 *   the component follows a `data-theme="light"` ancestor and otherwise uses
 *   its dark defaults.
 * @param {string} [options.label] Accessible name for the input.
 * @param {string} [options.resultsLabel] Accessible name for the listbox.
 * @param {string} [options.clearLabel] Accessible name for the clear button.
 * @param {string} [options.loadingMessage] Loading state text.
 * @param {string} [options.emptyMessage] Empty state text.
 * @param {string} [options.errorMessage] Error state text.
 * @param {string} [options.unnamedLabel] Fallback for a missing result name.
 * @returns {SearchUIHandle}
 */
export function createSearchUI({
  container,
  search,
  onSelect,
  onError,
  placeholder = DEFAULT_PLACEHOLDER,
  debounceMs = 250,
  theme,
  label = "Search places",
  resultsLabel = "Search results",
  clearLabel = "Clear search",
  loadingMessage = "Searching…",
  emptyMessage = "No places found.",
  errorMessage = "Search failed. Try again.",
  unnamedLabel = "Unnamed place",
} = {}) {
  if (typeof search !== "function") {
    throw new TypeError("createSearchUI: search must be a function");
  }
  if (onSelect !== undefined && typeof onSelect !== "function") {
    throw new TypeError("createSearchUI: onSelect must be a function when provided");
  }
  if (onError !== undefined && typeof onError !== "function") {
    throw new TypeError("createSearchUI: onError must be a function when provided");
  }

  const host =
    typeof container === "string" ? document.querySelector(container) : container;
  if (!(host instanceof Element)) {
    throw new Error("createSearchUI: container was not found");
  }

  const wait =
    Number.isFinite(debounceMs) && debounceMs >= 0 ? Number(debounceMs) : 250;
  const instance = nextInstance;
  nextInstance += 1;
  const inputId = `pinpoint-search-input-${instance}`;
  const listId = `pinpoint-search-list-${instance}`;

  const wrapper = document.createElement("div");
  wrapper.className = "pinpoint-search";
  if (theme === "light" || theme === "dark") wrapper.dataset.theme = theme;

  const bar = document.createElement("div");
  bar.className = "pinpoint-search__bar";
  bar.setAttribute("role", "search");

  const labelElement = document.createElement("label");
  labelElement.className = "pinpoint-search__label pinpoint-search__sr-only";
  labelElement.htmlFor = inputId;
  labelElement.textContent = label;

  const icon = document.createElementNS(SVG_NS, "svg");
  icon.classList.add("pinpoint-search__icon");
  icon.setAttribute("viewBox", "0 0 24 24");
  icon.setAttribute("width", "18");
  icon.setAttribute("height", "18");
  icon.setAttribute("fill", "none");
  icon.setAttribute("stroke", "currentColor");
  icon.setAttribute("stroke-width", "2");
  icon.setAttribute("stroke-linecap", "round");
  icon.setAttribute("aria-hidden", "true");
  icon.setAttribute("focusable", "false");
  const iconCircle = document.createElementNS(SVG_NS, "circle");
  iconCircle.setAttribute("cx", "11");
  iconCircle.setAttribute("cy", "11");
  iconCircle.setAttribute("r", "7");
  const iconPath = document.createElementNS(SVG_NS, "path");
  iconPath.setAttribute("d", "m16.5 16.5 4.5 4.5");
  icon.append(iconCircle, iconPath);

  const input = document.createElement("input");
  input.id = inputId;
  input.className = "pinpoint-search__input";
  input.type = "search";
  input.placeholder = placeholder;
  input.autocomplete = "off";
  input.spellcheck = false;
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.setAttribute("aria-controls", listId);
  input.setAttribute("aria-haspopup", "listbox");
  input.setAttribute("enterkeyhint", "search");

  const clearButton = document.createElement("button");
  clearButton.type = "button";
  clearButton.className = "pinpoint-search__clear";
  clearButton.setAttribute("aria-label", clearLabel);
  clearButton.title = clearLabel;
  clearButton.textContent = "\u00d7";
  clearButton.hidden = true;

  const popover = document.createElement("div");
  popover.className = "pinpoint-search__popover";
  popover.hidden = true;

  const list = document.createElement("ul");
  list.id = listId;
  list.className = "pinpoint-search__list";
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", resultsLabel);
  list.hidden = true;

  const status = document.createElement("p");
  status.className = "pinpoint-search__status";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.hidden = true;

  popover.append(list, status);
  bar.append(labelElement, icon, input, clearButton);
  wrapper.append(bar, popover);
  host.append(wrapper);

  const controller = new AbortController();
  const { signal } = controller;
  let results = [];
  let activeIndex = -1;
  let state = "idle";
  let pendingTimer = null;
  let requestToken = 0;
  let currentQuery = "";
  let destroyed = false;
  let suppressFocusOpen = false;
  let dismissed = false;

  function setState(next) {
    state = next;
    wrapper.dataset.state = next;
    if (next === "loading") wrapper.setAttribute("aria-busy", "true");
    else wrapper.removeAttribute("aria-busy");
  }

  function setStatusMessage(message, tone = "muted", showSpinner = false) {
    const children = [];
    if (showSpinner) {
      const spinner = document.createElement("span");
      spinner.className = "pinpoint-search__spinner";
      spinner.setAttribute("aria-hidden", "true");
      children.push(spinner);
    }
    const text = document.createElement("span");
    text.className = "pinpoint-search__status-text";
    text.textContent = message;
    children.push(text);
    status.replaceChildren(...children);
    status.dataset.tone = tone;
    status.hidden = false;
  }

  function hideStatus() {
    status.hidden = true;
    status.replaceChildren();
    delete status.dataset.tone;
  }

  function openPopover() {
    if (destroyed) return;
    popover.hidden = false;
    input.setAttribute("aria-expanded", "true");
  }

  function setActive(index) {
    activeIndex = index;
    const items = list.querySelectorAll(".pinpoint-search__item");
    items.forEach((item, itemIndex) => {
      const selected = itemIndex === index;
      item.classList.toggle("is-active", selected);
      item.setAttribute("aria-selected", selected ? "true" : "false");
    });
    const active = index >= 0 ? items[index] : null;
    if (active) {
      input.setAttribute("aria-activedescendant", active.id);
      if (typeof active.scrollIntoView === "function") {
        active.scrollIntoView({ block: "nearest" });
      }
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  function closePopover() {
    popover.hidden = true;
    input.setAttribute("aria-expanded", "false");
    setActive(-1);
  }

  function stringValue(value) {
    return typeof value === "string" ? value.trim() : "";
  }

  function renderList() {
    const fragment = document.createDocumentFragment();

    results.forEach((result, index) => {
      const item = document.createElement("li");
      item.className = "pinpoint-search__item";
      item.id = `${listId}-option-${index}`;
      item.setAttribute("role", "option");
      item.setAttribute("aria-selected", "false");
      item.dataset.index = String(index);

      const name = document.createElement("span");
      name.className = "pinpoint-search__name";
      name.textContent = stringValue(result?.name) || unnamedLabel;

      const kind = stringValue(result?.kind);
      if (kind) {
        const badge = document.createElement("span");
        badge.className = "pinpoint-search__kind";
        badge.textContent = kind;
        item.append(name, badge);
      } else {
        item.append(name);
      }

      const address = stringValue(result?.address);
      const lat = Number(result?.lat);
      const lon = Number(result?.lon);
      const coordinates =
        Number.isFinite(lat) && Number.isFinite(lon)
          ? `${lat.toFixed(5)}, ${lon.toFixed(5)}`
          : "";
      const meta = address || coordinates;
      if (meta) {
        const metaElement = document.createElement("span");
        metaElement.className = "pinpoint-search__address";
        metaElement.textContent = meta;
        item.append(metaElement);
      }

      fragment.append(item);
    });

    list.replaceChildren(fragment);
    list.hidden = results.length === 0;
  }

  function applyResults(nextResults, query = currentQuery, { forceOpen = false } = {}) {
    results = Array.isArray(nextResults)
      ? nextResults.filter((item) => item && typeof item === "object")
      : [];
    currentQuery = query;
    renderList();

    if (results.length === 0) {
      setStatusMessage(emptyMessage, "empty");
      setState("empty");
    } else {
      hideStatus();
      setState("results");
    }

    setActive(-1);
    // A late response must not reopen a list the user dismissed or a popover
    // the user is no longer interacting with.
    if (forceOpen || (!dismissed && document.activeElement === input)) {
      openPopover();
    } else {
      closePopover();
    }
    return results.length;
  }

  function showError(error, query) {
    results = [];
    currentQuery = query;
    list.replaceChildren();
    list.hidden = true;
    setStatusMessage(errorMessage, "error");
    setState("error");
    setActive(-1);
    if (!dismissed && document.activeElement === input) openPopover();
    else closePopover();

    if (typeof onError === "function") {
      try {
        onError(error);
      } catch {
        // A host diagnostic hook must never break the component.
      }
    }
  }

  function runSearch(rawQuery) {
    if (destroyed) return;
    const query = String(rawQuery ?? "").trim();
    if (!query) return;

    dismissed = false;
    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }

    const token = requestToken + 1;
    requestToken = token;
    currentQuery = query;
    results = [];
    renderList();
    hideStatus();
    setStatusMessage(loadingMessage, "loading", true);
    setState("loading");
    setActive(-1);
    openPopover();

    let pending;
    try {
      pending = search(query);
    } catch (error) {
      if (!destroyed && token === requestToken) showError(error, query);
      return;
    }

    Promise.resolve(pending).then(
      (value) => {
        if (destroyed || token !== requestToken) return;
        if (!Array.isArray(value)) {
          showError(
            new TypeError("search() must resolve to an array of results"),
            query,
          );
          return;
        }
        applyResults(value, query);
      },
      (error) => {
        if (destroyed || token !== requestToken) return;
        showError(error, query);
      },
    );
  }

  function focusInput() {
    if (destroyed) return;
    suppressFocusOpen = true;
    try {
      input.focus({ preventScroll: true });
    } catch {
      input.focus();
    }
    queueMicrotask(() => {
      suppressFocusOpen = false;
    });
  }

  function reset({ focus = false } = {}) {
    if (destroyed) return;
    dismissed = false;
    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
    requestToken += 1;
    currentQuery = "";
    results = [];
    renderList();
    hideStatus();
    closePopover();
    clearButton.hidden = true;
    setState("idle");
    if (focus) focusInput();
  }

  function clear() {
    if (destroyed) return;
    input.value = "";
    reset({ focus: true });
  }

  function setResults(nextResults) {
    if (destroyed) return 0;
    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
    requestToken += 1;
    dismissed = false;
    return applyResults(nextResults, input.value.trim(), { forceOpen: true });
  }

  function select(index) {
    const result = results[index];
    if (!result) return;
    closePopover();
    focusInput();
    if (typeof onSelect === "function") onSelect(result);
  }

  function onInput() {
    if (destroyed) return;
    const query = input.value.trim();
    dismissed = false;
    clearButton.hidden = query.length === 0;

    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
    requestToken += 1;
    currentQuery = query;
    results = [];
    renderList();
    hideStatus();
    closePopover();
    setState("idle");

    if (!query) return;

    pendingTimer = setTimeout(() => {
      pendingTimer = null;
      runSearch(input.value);
    }, wait);
  }

  function onKeyDown(event) {
    if (destroyed || event.isComposing) return;

    if (event.key === "ArrowDown") {
      if (results.length === 0) return;
      event.preventDefault();
      if (popover.hidden) {
        dismissed = false;
        openPopover();
      }
      const next = activeIndex + 1;
      setActive(next >= results.length ? 0 : next);
      return;
    }

    if (event.key === "ArrowUp") {
      if (results.length === 0) return;
      event.preventDefault();
      if (popover.hidden) {
        dismissed = false;
        openPopover();
      }
      const next = activeIndex - 1;
      setActive(next < 0 ? results.length - 1 : next);
      return;
    }

    if (event.key === "Enter") {
      if (activeIndex >= 0 && results[activeIndex]) {
        event.preventDefault();
        select(activeIndex);
        return;
      }
      if (state === "loading") {
        event.preventDefault();
        return;
      }
      if (input.value.trim()) {
        event.preventDefault();
        runSearch(input.value);
      }
      return;
    }

    if (event.key === "Escape") {
      if (!popover.hidden) {
        event.preventDefault();
        dismissed = true;
        closePopover();
        return;
      }
      if (input.value.trim()) {
        event.preventDefault();
        clear();
      }
    }
  }

  function onFocus() {
    if (destroyed || suppressFocusOpen) return;
    dismissed = false;
    if (state === "results" && results.length > 0) openPopover();
  }

  function onListClick(event) {
    const target = event.target instanceof Element ? event.target : null;
    const item = target?.closest(".pinpoint-search__item");
    if (!item || !list.contains(item)) return;
    const index = Number(item.dataset.index);
    if (Number.isInteger(index) && index >= 0) select(index);
  }

  function onListPointerOver(event) {
    const target = event.target instanceof Element ? event.target : null;
    const item = target?.closest(".pinpoint-search__item");
    if (!item || !list.contains(item)) return;
    const index = Number(item.dataset.index);
    if (Number.isInteger(index) && index >= 0 && index !== activeIndex) {
      setActive(index);
    }
  }

  function onDocumentPointerDown(event) {
    if (destroyed || popover.hidden) return;
    const target = event.target;
    if (target instanceof Node && wrapper.contains(target)) return;
    dismissed = true;
    closePopover();
  }

  function onFocusOut(event) {
    if (destroyed) return;
    const next = event.relatedTarget;
    // A pointer tap on a non-focusable result reports null here; let the click
    // finish and rely on the document pointerdown handler for outside taps.
    if (next === null) return;
    if (next instanceof Node && wrapper.contains(next)) return;
    dismissed = true;
    closePopover();
  }

  input.addEventListener("input", onInput, { signal });
  input.addEventListener("keydown", onKeyDown, { signal });
  input.addEventListener("focus", onFocus, { signal });
  clearButton.addEventListener("click", () => clear(), { signal });
  list.addEventListener("click", onListClick, { signal });
  list.addEventListener("pointerover", onListPointerOver, { signal });
  list.addEventListener("mousedown", (event) => event.preventDefault(), {
    signal,
  });
  document.addEventListener("pointerdown", onDocumentPointerDown, { signal });
  wrapper.addEventListener("focusout", onFocusOut, { signal });

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    if (pendingTimer !== null) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
    requestToken += 1;
    controller.abort();
    wrapper.remove();
    results = [];
    activeIndex = -1;
  }

  function setTheme(next) {
    if (destroyed) return;
    if (next === "light" || next === "dark") {
      wrapper.dataset.theme = next;
    } else {
      delete wrapper.dataset.theme;
    }
  }

  renderList();
  setState("idle");

  return {
    setResults,
    clear,
    destroy,
    focus: focusInput,
    setTheme,
    element: wrapper,
  };
}

/**
 * @typedef {object} SearchUIHandle
 * @property {(results: SearchResult[]) => number} setResults Replace the list
 *   without calling the provider. Returns the number of accepted results.
 * @property {() => void} clear Empty the input, results and status, and focus
 *   the input.
 * @property {() => void} destroy Remove the component's DOM and listeners.
 * @property {() => void} focus Focus the search input.
 * @property {(theme: "light"|"dark"|null) => void} setTheme Set or clear the
 *   explicit theme.
 * @property {HTMLElement} element The component wrapper.
 */
