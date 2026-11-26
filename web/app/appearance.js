// Map appearance: label size + icon size, persisted on this device.
// Mirrors Magic Earth's "Map Appearance" setting without touching the data.

const $ = (id) => document.getElementById(id);
const KEY = "pinpoint.mapAppearance.v1";
const THEME_KEY = "pinpoint.theme.v1";
const DEFAULTS = { label: 12, icon: 26 };

function initTheme() {
  const root = document.documentElement;
  const toggle = $("theme-toggle");
  let theme = "light";
  try {
    theme = localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
  } catch (_) {
    // storage blocked: light is the safe default
  }
  const render = () => {
    root.dataset.theme = theme;
    document.dispatchEvent(new CustomEvent("pinpoint:theme", { detail: theme }));
    if (!toggle) return;
    toggle.textContent = theme === "dark" ? "Light theme" : "Dark theme";
    toggle.setAttribute(
      "aria-label",
      theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
    );
    toggle.setAttribute("aria-pressed", String(theme === "dark"));
  };
  render();
  toggle?.addEventListener("click", () => {
    theme = theme === "dark" ? "light" : "dark";
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch (_) {
      // keep the session usable when storage is blocked
    }
    render();
  });
}

function load() {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) || "{}");
    return {
      label: clamp(Number(raw.label) || DEFAULTS.label, 10, 18),
      icon: clamp(Number(raw.icon) || DEFAULTS.icon, 18, 40),
    };
  } catch (_) {
    return { ...DEFAULTS };
  }
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function apply(settings) {
  const root = document.documentElement;
  root.style.setProperty("--map-label-size", `${settings.label}px`);
  root.style.setProperty("--map-pin-size", `${settings.icon}px`);
  const labelInput = $("map-label-size");
  const iconInput = $("map-icon-size");
  if (labelInput) labelInput.value = String(settings.label);
  if (iconInput) iconInput.value = String(settings.icon);
  const labelValue = $("map-label-value");
  const iconValue = $("map-icon-value");
  if (labelValue) labelValue.textContent = `${settings.label}px`;
  if (iconValue) iconValue.textContent = `${settings.icon}px`;
}

function save(settings) {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings));
  } catch (_) {
    // private mode/quota: keep the current session usable
  }
}

export function initAppearance() {
  initTheme();
  let settings = load();
  apply(settings);

  $("map-label-size").addEventListener("input", (e) => {
    settings = { ...settings, label: clamp(Number(e.target.value), 10, 18) };
    apply(settings);
    save(settings);
  });
  $("map-icon-size").addEventListener("input", (e) => {
    settings = { ...settings, icon: clamp(Number(e.target.value), 18, 40) };
    apply(settings);
    save(settings);
  });
  $("map-appearance-reset").addEventListener("click", () => {
    settings = { ...DEFAULTS };
    apply(settings);
    save(settings);
  });
}
