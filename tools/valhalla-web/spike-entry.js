import { serializeError } from "/lib/metrics.mjs";

const params = new URLSearchParams(location.search);
const test = params.get("test") ?? "adret";
const log = (message, data) => {
  console.log(message, data === undefined ? "" : JSON.stringify(data));
  const pre = document.getElementById("log");
  if (pre) pre.textContent += `${message} ${data === undefined ? "" : JSON.stringify(data)}\n`;
};

window.__SPIKE_STARTED = true;
try {
  const module = await import(`/scenarios/${test}.js`);
  const result = await module.run();
  window.__RESULT = result;
  log("result", { ok: result.ok, error: result.error, timings: result.timings });
} catch (error) {
  window.__RESULT = { scenario: test, ok: false, error: serializeError(error) };
  log("fatal", window.__RESULT.error);
}
window.__SPIKE_DONE = true;
