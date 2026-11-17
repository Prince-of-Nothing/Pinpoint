/**
 * Run the Valhalla WASM spike scenarios in headless Chromium.
 *
 * Puppeteer is not a dependency of this spike; point NODE_PATH at a
 * node_modules that contains it (the repository's map smoke test documents
 * the same pattern), or install it locally.
 *
 * Usage:
 *   $env:NODE_PATH="C:\Users\Dinusauuuur\AppData\Local\Temp\opencode\node_modules"
 *   node tools/valhalla-web/run.mjs [--only adret,jansoft,vb-indexed,vb-raw]
 */

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createSpikeServer } from "./serve.mjs";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RESULTS = path.join(HERE, ".data", "results");
const TAR_PATH =
  process.env.PINPOINT_MOLDOVA_TAR ||
  path.resolve(HERE, "../../../Pinpoint/dist/offline/moldova/moldova-valhalla.tar");
const require = createRequire(import.meta.url);

let puppeteer;
try {
  puppeteer = require("puppeteer");
} catch {
  console.error("puppeteer not found; set NODE_PATH to a node_modules containing puppeteer");
  process.exit(2);
}

const onlyArg = process.argv.indexOf("--only");
const only = onlyArg >= 0 ? new Set(process.argv[onlyArg + 1].split(",")) : null;
const throttleArg = process.argv.indexOf("--throttle");
const throttle = throttleArg >= 0 ? Number(process.argv[throttleArg + 1]) : 1;

const datasetRoot = path.join(HERE, ".data", "sdk-dataset");
const hasTar = existsSync(TAR_PATH);
const hasIndexed = existsSync(path.join(datasetRoot, "moldova-20261116", "manifest.json"));
const hasRaw = existsSync(path.join(datasetRoot, "moldova-20261116-raw", "manifest.json"));
const hasIndividual = existsSync(path.join(datasetRoot, "moldova-20261116", "tiles"));
const hasJansoft = existsSync(path.join(HERE, "node_modules", "@jansoft", "mbujkanji-valhalla-wasm"));

const scenarios = [
  { name: "vb-raw", test: "valhalla-browser", manifest: "/datasets/moldova-20261116-raw/manifest.json", expected: "failure", ready: hasTar && hasRaw },
  { name: "vb-indexed", test: "valhalla-browser", manifest: "/datasets/moldova-20261116/manifest.json", expected: "route-or-exact-error", ready: hasTar && hasIndexed },
  { name: "vb-individual", test: "valhalla-browser", manifest: "/datasets/moldova-20261116/manifest.json", transport: "individual-tiles", expected: "route-or-exact-error", ready: hasTar && hasIndexed && hasIndividual },
  { name: "jansoft", test: "jansoft", expected: "failure", ready: hasTar && hasJansoft },
  { name: "adret", test: "adret", expected: "route", ready: hasTar },
  { name: "adret-corridor", test: "adret", subset: "corridor", expected: "route", ready: hasTar },
].filter((scenario) => (!only || only.has(scenario.name)) && scenario.ready);

function processTable() {
  if (process.platform !== "win32") return null;
  try {
    const json = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,WorkingSetSize,Name | ConvertTo-Json -Compress",
      ],
      { encoding: "utf8", maxBuffer: 32 * 1024 * 1024, timeout: 30_000 },
    );
    const parsed = JSON.parse(json);
    return Array.isArray(parsed) ? parsed : [parsed];
  } catch (error) {
    return { error: String(error?.message ?? error) };
  }
}

function processTreeMemory(pid, table) {
  if (!table || !Array.isArray(table)) return null;
  const byParent = new Map();
  for (const process of table) {
    const parent = Number(process.ParentProcessId);
    if (!byParent.has(parent)) byParent.set(parent, []);
    byParent.get(parent).push(process);
  }
  const seen = new Set();
  const stack = [Number(pid)];
  let bytes = 0;
  const names = new Set();
  while (stack.length) {
    const current = stack.pop();
    if (seen.has(current)) continue;
    seen.add(current);
    const process = table.find((candidate) => Number(candidate.ProcessId) === current);
    if (process) {
      bytes += Number(process.WorkingSetSize) || 0;
      names.add(process.Name);
    }
    for (const child of byParent.get(current) ?? []) stack.push(Number(child.ProcessId));
  }
  return { bytes, processes: seen.size, names: [...names] };
}

function sanitize(value) {
  return JSON.parse(JSON.stringify(value));
}

async function runScenario(server, base, scenario) {
  const profile = mkdtempSync(path.join(tmpdir(), `pinpoint-valhalla-${scenario.name}-`));
  const browser = await puppeteer.launch({
    headless: true,
    userDataDir: profile,
    protocolTimeout: 300_000,
    args: [
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-dev-shm-usage",
      "--enable-precise-memory-info",
      "--js-flags=--expose-gc",
      "--disable-gpu",
    ],
  });
  const browserPid = browser.process()?.pid;
  const page = await browser.newPage();
  page.setDefaultTimeout(180_000);
  if (Number.isFinite(throttle) && throttle > 1) await page.emulateCPUThrottling(throttle);
  const logs = [];
  const requests = [];
  const failures = [];
  page.on("console", (message) => {
    const entry = { type: message.type(), text: message.text() };
    logs.push(entry);
    if (entry.type === "error" || entry.type === "warning") console.log(`  [${scenario.name} ${entry.type}] ${entry.text}`);
  });
  page.on("pageerror", (error) => {
    logs.push({ type: "pageerror", text: error.message, stack: error.stack });
  });
  page.on("workercreated", (worker) => {
    try {
      worker.on("console", (message) => {
        const entry = { type: `worker-${message.type()}`, text: message.text() };
        logs.push(entry);
        if (entry.type === "worker-error" || entry.type === "worker-warning") console.log(`  [${scenario.name} ${entry.type}] ${entry.text}`);
      });
    } catch {
      // Older Puppeteer versions may not expose worker console messages.
    }
  });
  page.on("request", (request) => {
    requests.push({ url: request.url(), method: request.method(), type: request.resourceType() });
  });
  page.on("requestfailed", (request) => {
    failures.push({ url: request.url(), error: request.failure()?.errorText });
  });
  const responseStatuses = new Map();
  page.on("response", (response) => {
    responseStatuses.set(response.url(), response.status());
  });

  const query = new URLSearchParams({ test: scenario.test });
  if (scenario.manifest) query.set("manifest", scenario.manifest);
  if (scenario.transport) query.set("transport", scenario.transport);
  if (scenario.subset) query.set("subset", scenario.subset);
  const pageUrl = `${base}/spike.html?${query}`;
  const started = performance.now();
  await page.goto(pageUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });
  let result;
  try {
    await page.waitForFunction("window.__SPIKE_DONE === true", { timeout: 180_000 });
    result = sanitize(await page.evaluate(() => window.__RESULT));
  } catch (error) {
    result = { scenario: scenario.name, ok: false, error: { name: "HarnessTimeout", message: String(error?.message ?? error) } };
  }
  const elapsedMs = performance.now() - started;
  const metrics = await page.metrics().catch(() => null);
  const table = processTable();
  const memory = processTreeMemory(browserPid, table);

  const external = requests.filter((request) => {
    try {
      return new URL(request.url).origin !== base;
    } catch {
      return false;
    }
  });
  const withStatus = requests.map((request) => ({ ...request, status: responseStatuses.get(request.url) ?? null }));
  const output = sanitize({
    name: scenario.name,
    expected: scenario.expected,
    cpuThrottle: throttle,
    pageUrl,
    elapsedMs,
    ok: result?.ok ?? false,
    result,
    network: {
      requestCount: requests.length,
      external,
      failures,
      requests: withStatus,
    },
    pageMetrics: metrics,
    processMemory: memory,
    logs,
    profile,
  });
  await browser.close();
  return output;
}

if (!scenarios.length) {
  console.error("no runnable scenarios; check PINPOINT_MOLDOVA_TAR, prepare-sdk-dataset.py and npm run install:jansoft");
  process.exit(2);
}

mkdirSync(RESULTS, { recursive: true });
const server = createSpikeServer({ tarPath: TAR_PATH });
await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});
const address = server.address();
if (!address || typeof address === "string") throw new Error("could not determine server port");
const base = `http://127.0.0.1:${address.port}`;
console.log(`server ${base}, scenarios: ${scenarios.map((scenario) => scenario.name).join(", ")}`);

const summaries = [];
for (const scenario of scenarios) {
  console.log(`\n=== ${scenario.name} (${scenario.test}) ===`);
  const output = await runScenario(server, base, scenario);
  writeFileSync(path.join(RESULTS, `${scenario.name}.json`), JSON.stringify(output, null, 2));
  const summary = {
    name: output.name,
    ok: output.ok,
    elapsedMs: Math.round(output.elapsedMs),
    error: output.result?.error ?? output.result?.loadTiles?.error ?? null,
    routes: Object.fromEntries(
      Object.entries(output.result?.routes ?? {}).map(([costing, route]) => [
        costing,
        route.ok ? { ms: Math.round(route.ms), summary: route.summary, maneuvers: route.maneuvers } : { ok: false, error: route.error },
      ]),
    ),
    timings: output.result?.timings,
    startup: output.result?.startup,
    memory: output.result?.memory,
    processMemory: output.processMemory,
    networkRequests: output.network.requestCount,
    externalRequests: output.network.external.length,
  };
  summaries.push(summary);
  console.log(JSON.stringify(summary, null, 2));
}

server.close();
writeFileSync(path.join(RESULTS, "summary.json"), JSON.stringify(summaries, null, 2));
console.log(`\nwrote ${RESULTS}\\summary.json`);
