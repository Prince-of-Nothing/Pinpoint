// Sets the map theme before first paint. Loaded as a classic script in
// <head> so it runs synchronously; kept out of the HTML so the page can
// drop 'unsafe-inline' from script-src.
    try {
      const saved = localStorage.getItem("pinpoint-map-theme");
      const dark = saved ? saved === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    } catch {
      document.documentElement.dataset.theme = "light";
    }
