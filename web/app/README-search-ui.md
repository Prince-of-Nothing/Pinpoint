# Pinpoint offline search UI

A self-contained search box and result list for the offline map. It is the UI
half of design §9: the caller injects a local search function, the component
debounces the query, renders results and handles the keyboard. It never calls
the network, so the same module works in `web/app`, the standalone
`web/map` WebView page and the SQLite WASM search backend once that lands.

```text
web/app/search-ui.js       ES module: createSearchUI()
web/app/search.css         component styles (dark + light)
web/app/search-demo.html   local demo with a Bălți / Chișinău stub provider
web/app/test/search-ui.mjs headless Chromium smoke test
```

This branch does **not** edit `web/app/index.html`, `app.js`, `app.css`,
`map.js`, `nav.js` or `web/map/map.js`. The component is new and opt-in; the
integration stream wires it into the existing screens.

## Run the demo

From the repository root:

```powershell
node web/map/serve.mjs
# open http://127.0.0.1:8080/app/search-demo.html
```

The demo uses only local files. The stub provider matches Romanian and
Cyrillic names with folded diacritics, so `balti`, `chisinau` and `кишинёв`
all work. It shows the loading, result, empty and error states; the
`Fail next search` button forces the error path. Attribution is visible in the
stage: © OpenStreetMap contributors · ODbL 1.0.

## Run the smoke test

```powershell
$env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
$env:PINPOINT_SCREENSHOT="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\search-ui.png"
node web/app/test/search-ui.mjs
```

The test serves `web/` on a random loopback port, checks the local-only asset
contract, then opens the demo in headless Chromium. It types `balti`, waits for
`Bălți`, moves with ArrowDown, selects with Enter, checks the exact `onSelect`
payload and the closed list, clears the query, checks outside-click dismissal,
the Cyrillic query, Escape, empty state, provider error state, the light/dark
tokens, `setResults`, `clear` and `destroy`, and asserts zero page errors,
console errors, failed requests and external requests. It writes the result-list screenshot to `PINPOINT_SCREENSHOT` and a
selected-state screenshot next to it as `pinpoint-search-ui-selected.png`.

If Puppeteer cannot be resolved, the test prints the `NODE_PATH` it looked at,
states that the browser smoke did not run, and exits non-zero. It does not
report a false pass. `--static-only` runs just the static checks and says so.

## API

```js
import { createSearchUI } from "./search-ui.js";

const searchUI = createSearchUI({
  container: "#search-host",        // element or selector
  search: async (query) => results, // injected local provider
  onSelect: (result) => {
    // center the map, open a place sheet, or start a route
  },
  placeholder: "Search a place…",   // default
});
```

### `createSearchUI(options)`

| Option | Type | Notes |
|---|---|---|
| `container` | `HTMLElement \| string` | Required. The component appends its own wrapper to this element. |
| `search` | `(query) => Promise<SearchResult[]>` | Required. Receives the trimmed query. May throw or reject. No network is made by the component. |
| `onSelect` | `(result) => void` | Optional. Receives the original provider object, including any extra fields. |
| `onError` | `(error) => void` | Optional diagnostic hook. The visible message stays generic. |
| `placeholder` | `string` | Default `"Search a place…"`. |
| `debounceMs` | `number` | Default `250`. Set `0` for tests or an already-cheap in-memory provider. |
| `theme` | `"light" \| "dark"` | Optional explicit theme. Without it, a `data-theme="light"` ancestor switches the light tokens; otherwise the dark defaults apply. |
| `label` | `string` | Accessible input name. Default `"Search places"`. |
| `resultsLabel` | `string` | Listbox name. Default `"Search results"`. |
| `clearLabel` | `string` | Clear button name. Default `"Clear search"`. |
| `loadingMessage` | `string` | Default `"Searching…"`. |
| `emptyMessage` | `string` | Default `"No places found."`. |
| `errorMessage` | `string` | Default `"Search failed. Try again."`. |
| `unnamedLabel` | `string` | Fallback when `name` is missing. Default `"Unnamed place"`. |

The result shape is the one from the design and the SQLite index:

```js
{
  name: "Bălți",
  kind: "city",
  lat: 47.7615,
  lon: 27.9289,
  address: "Bălți, Moldova",
  importance: 0.86
}
```

`name`, `kind` and `address` are rendered as text (never as HTML). `lat`/`lon`
fall back to a coordinate line when `address` is empty. `importance` is not
displayed; it is passed through to `onSelect`.

### Handle

```js
const searchUI = createSearchUI({ ... });

searchUI.setResults(results); // replace the list without calling the provider
searchUI.clear();             // empty input/list/status and focus the input
searchUI.focus();             // focus the input
searchUI.setTheme("light");   // or "dark"; null clears the explicit theme
searchUI.destroy();           // remove the component's DOM and listeners
searchUI.element;             // the component wrapper
```

- `setResults(results)` returns the number of accepted result objects and is
  useful when the backend pushes a merged offline/online list or restores a
  previous query.
- `clear()` cancels pending work, invalidates an in-flight response, empties
  the list and focuses the input.
- `destroy()` is idempotent. It removes only the wrapper the component created;
  it does not clear the host container's other content.

### Keyboard and accessibility

- `ArrowDown` / `ArrowUp` move through results (wrapping at the ends).
- `Enter` selects the active result. With no active result it runs the current
  query immediately, bypassing the debounce.
- `Escape` closes an open list; a second `Escape` clears the query.
- `Tab` moves focus out and closes the list. Clicking outside closes it too.
- The input is a `role="combobox"` with `aria-autocomplete="list"`,
  `aria-expanded`, `aria-controls` and `aria-activedescendant`. The list is a
  `role="listbox"` with `role="option"` children; the label, clear button and
  status region are announced. The visible focus ring is on the search bar and
  the clear button.

### States and theming

`data-state` on the wrapper is one of `idle`, `loading`, `results`, `empty` or
`error`, so hosts can animate or position the component without parsing the
DOM. `aria-busy` is set while loading. The status region uses
`role="status"`/`aria-live="polite"`.

`search.css` uses the existing app tokens when present
(`--panel`, `--panel-2`, `--border`, `--text`, `--muted`, `--accent`,
`--danger`) and falls back to the same dark values. It also exposes:

```css
.pinpoint-search {
  --pinpoint-search-bg: #0f172a;
  --pinpoint-search-border: #1f2937;
  --pinpoint-search-accent: #34d399;
  --pinpoint-search-radius: 0.75rem;
  --pinpoint-search-z: 1000;
  /* ... */
}
```

The host controls the outer width and placement. The result popover is
absolutely positioned below the bar, so give the component a non-clipping
parent (the existing `.map-topbar` works) and keep the host's own attribution
visible.

## Hookup to `web/map/search.js` (integration stream)

`web/map/search.js` does **not** exist on this branch. This component was built
against the contract the offline-search stream will provide, so the integration
can add that file without changing `search-ui.js`:

- `openSearch(options)` opens the local SQLite WASM pack (worker/OPFS) and
  resolves to a handle. The component only needs a function that takes a query
  and resolves to the result array above.
- The handle's `search(query, options)` (or a module-level `search` export after
  opening the pack) performs the ranked FTS5 lookup.
- Extra fields on a result (for example a packing bbox) are preserved on the
  object passed to `onSelect`.

### Map / WebView page

```js
import { openSearch } from "../map/search.js";
import { createMap } from "../map/map.js";
import { createSearchUI } from "../app/search-ui.js";

const map = createMap({
  container: "#map",
  styleUrl: "/map/style.json",
  pmtilesUrl: "/packs/moldova.pmtiles",
});

const offline = await openSearch({
  // Exact option names will be fixed by the search.js stream. The component
  // only depends on the returned search function.
  packUrl: "/packs/moldova-search.sqlite",
});

const searchUI = createSearchUI({
  container: "#search-host",
  search: (query) => offline.search(query, { limit: 8 }),
  onSelect: (place) => {
    map.getMap()?.flyTo({ center: [place.lon, place.lat], zoom: 15 });
    // A long-press or a route button can use the same place as a destination.
  },
});
```

The WebView page has no session or CSRF token. `openSearch` must stay local;
the page should not fall back to `/map/search` or Nominatim.

If the backend instead exposes a module-level function after its own `open`:

```js
import { openSearch, search } from "../map/search.js";

await openSearch({ packUrl: "/packs/moldova-search.sqlite" });
const searchUI = createSearchUI({
  container: "#search-host",
  search: (query) => search(query, { limit: 8 }),
  onSelect: (place) => { /* ... */ },
});
```

### Existing web app

`web/app/nav.js` currently owns `#search-form`, `#search-results` and the
`/map/search` fetch. Once the offline backend is available, the integration
stream can add a `#search-host` container to the map topbar, instantiate this
component, and route selection through the existing `selectPlace(place)` path.
The field names differ, so adapt them:

```js
onSelect: (place) => {
  selectPlace({
    display_name: place.address || place.name,
    short_name: place.name,
    latitude: place.lat,
    longitude: place.lon,
    kind: place.kind,
  });
}
```

Keep the existing place sheet, route panel and `mapApi.setSelectedPlace`
behavior. Do not send session cookies or friend data to the search provider;
offline search only reads the local pack.

## ODbL and privacy

The demo shows the required attribution and the README repeats it:

> Search data © [OpenStreetMap contributors](https://www.openstreetmap.org/copyright),
> available under [ODbL 1.0](https://opendatacommons.org/licenses/odbl/1-0/).

The component does not inject attribution itself because the map host already
has an attribution overlay and the pack's manifest carries the license. Every
host that displays offline search results must keep that attribution visible.
The component sends nothing anywhere: no analytics, no remote fonts, no tiles,
no Nominatim and no network fallback.

## Limitations

- **No backend here.** Results come only from the injected function; this
  module does not open SQLite, OPFS or a worker.
- **No request abort.** A newer query invalidates an older response with a
  token and the stale response is ignored, but the injected provider gets no
  `AbortSignal` yet. Extend the injected function if the backend supports
  cancellation.
- **Provider owns ranking and limits.** The component renders what it receives;
  it does not fuzzy-match, deduplicate or paginate. Pass `limit` in the
  provider call for large packs.
- **No attribution injection.** The host map page and About page remain
  responsible for ODbL attribution.
- **Headless Chromium only.** The test exercises the component in Chromium; it
  does not run an Android WebView, WebView2, WKWebView or a real mobile device.
  The CSS avoids CDN assets and uses only APIs available in the WebView 100
  floor, but platform smoke remains an integration task.
- **Generic error text.** The visible error message is intentionally not the
  raw provider error. Use `onError` for logging and keep user-facing details
  safe.
