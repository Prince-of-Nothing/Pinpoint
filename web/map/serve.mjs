/**
 * Tiny static file server for the `web/` tree.
 *
 * PMTiles needs HTTP Range requests. This server implements them so the map
 * preview works locally without adding a dependency. It is a development tool,
 * not the production server.
 *
 * Usage:
 *   node web/map/serve.mjs [port]
 *   PORT=9000 node web/map/serve.mjs
 */

import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const WEB_ROOT = resolve(fileURLToPath(new URL("../", import.meta.url)));

const MIME_TYPES = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".md": "text/plain; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".pmtiles": "application/octet-stream",
  ".png": "image/png",
  ".sqlite": "application/vnd.sqlite3",
  ".svg": "image/svg+xml",
  ".txt": "text/plain; charset=utf-8",
  ".wasm": "application/wasm",
  ".webp": "image/webp",
};

function contentType(filePath) {
  return MIME_TYPES[extname(filePath).toLowerCase()] ?? "application/octet-stream";
}

function sendError(response, status, message, headers = {}) {
  response.writeHead(status, {
    "Content-Type": "text/plain; charset=utf-8",
    "Content-Length": Buffer.byteLength(message),
    ...headers,
  });
  response.end(message);
}

function sendJson(response, body, method = "GET") {
  const payload = JSON.stringify(body);
  response.writeHead(200, {
    "Content-Type": "application/json; charset=utf-8",
    "Content-Length": Buffer.byteLength(payload),
    "Cache-Control": "no-store",
  });
  if (method === "HEAD") response.end();
  else response.end(payload);
}

/**
 * Preview-only stubs for the two endpoints the Rust loopback map servers
 * implement. They keep the page free of 404 console noise when it is opened
 * with this static development server. The download action itself belongs to
 * the app server; a static preview never installs data.
 */
function sendMapServerPreview(response, pathname, method) {
  if (pathname === "/map/config") {
    sendJson(
      response,
      {
        pmtilesUrl: "./fixtures/monaco.pmtiles",
        graphInstalled: false,
        packInstalled: false,
      },
      method,
    );
    return true;
  }
  if (pathname === "/map/offline/status") {
    const idle = {
      installed: false,
      bytes: 0,
      expected_bytes: 0,
      progress_bytes: 0,
      phase: "idle",
      error: null,
    };
    sendJson(response, { pack: idle, graph: idle }, method);
    return true;
  }
  return false;
}

/**
 * Create (but do not start) a static server rooted at `web/`.
 *
 * @param {{root?: string}} [options]
 * @returns {import("node:http").Server}
 */
export function createStaticServer({ root = WEB_ROOT } = {}) {
  const rootPath = resolve(root);

  return createServer((request, response) => {
    const method = request.method ?? "GET";
    if (method !== "GET" && method !== "HEAD") {
      sendError(response, 405, "Method Not Allowed", { Allow: "GET, HEAD" });
      return;
    }

    let pathname;
    try {
      pathname = decodeURIComponent(new URL(request.url ?? "/", "http://localhost").pathname);
    } catch {
      sendError(response, 400, "Bad Request");
      return;
    }

    if (sendMapServerPreview(response, pathname, method)) return;

    const parts = pathname.split(/[\\/]+/).filter(Boolean);
    if (parts.some((part) => part === "." || part === ".." || part.includes(":"))) {
      sendError(response, 403, "Forbidden");
      return;
    }

    let filePath = resolve(rootPath, ...parts);
    if (filePath !== rootPath && !filePath.startsWith(rootPath + sep)) {
      sendError(response, 403, "Forbidden");
      return;
    }

    let stats;
    try {
      stats = statSync(filePath);
      if (stats.isDirectory()) {
        filePath = resolve(filePath, "index.html");
        stats = statSync(filePath);
      }
    } catch {
      sendError(response, 404, "Not Found");
      return;
    }

    if (!stats.isFile()) {
      sendError(response, 404, "Not Found");
      return;
    }

    const headers = {
      "Content-Type": contentType(filePath),
      "Accept-Ranges": "bytes",
      "Cache-Control": "no-cache",
    };
    const range = request.headers.range;

    if (range) {
      const match = /^bytes=(\d*)-(\d*)$/.exec(range.trim());
      if (!match) {
        sendError(response, 416, "Range Not Satisfiable", {
          "Content-Range": `bytes */${stats.size}`,
        });
        return;
      }

      let start = match[1] === "" ? null : Number(match[1]);
      let end = match[2] === "" ? null : Number(match[2]);

      if (start === null) {
        const suffixLength = end ?? 0;
        if (suffixLength <= 0) {
          sendError(response, 416, "Range Not Satisfiable", {
            "Content-Range": `bytes */${stats.size}`,
          });
          return;
        }
        start = Math.max(0, stats.size - suffixLength);
        end = stats.size - 1;
      } else {
        if (end === null || end >= stats.size) end = stats.size - 1;
      }

      if (start < 0 || start >= stats.size || end < start) {
        sendError(response, 416, "Range Not Satisfiable", {
          "Content-Range": `bytes */${stats.size}`,
        });
        return;
      }

      response.writeHead(206, {
        ...headers,
        "Content-Range": `bytes ${start}-${end}/${stats.size}`,
        "Content-Length": end - start + 1,
      });

      if (method === "HEAD") response.end();
      else createReadStream(filePath, { start, end }).pipe(response);
      return;
    }

    response.writeHead(200, {
      ...headers,
      "Content-Length": stats.size,
    });

    if (method === "HEAD") response.end();
    else createReadStream(filePath).pipe(response);
  });
}

const entrypoint = process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href : "";
if (entrypoint === import.meta.url) {
  const port = Number(process.argv[2] ?? process.env.PORT ?? 8080);
  const server = createStaticServer();

  server.listen(port, "127.0.0.1", () => {
    const address = server.address();
    const actualPort = typeof address === "object" && address ? address.port : port;
    console.log(`Serving ${WEB_ROOT}`);
    console.log(`Map preview: http://127.0.0.1:${actualPort}/map/index.html`);
  });
}
