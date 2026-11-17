import { createReadStream, existsSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));

const MIME = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".wasm", "application/wasm"],
  [".css", "text/css; charset=utf-8"],
  [".tar", "application/octet-stream"],
  [".gph", "application/octet-stream"],
  [".txt", "text/plain; charset=utf-8"],
  [".md", "text/markdown; charset=utf-8"],
]);

function mime(file) {
  return MIME.get(path.extname(file).toLowerCase()) ?? "application/octet-stream";
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

export function createSpikeServer({ root = HERE, dataRoot = path.join(HERE, ".data", "sdk-dataset"), tarPath } = {}) {
  const etags = new Map();
  for (const release of ["moldova-20261116", "moldova-20261116-raw"]) {
    const manifest = readJson(path.join(dataRoot, release, "manifest.json"));
    if (manifest?.archive?.etag) {
      etags.set(path.join(dataRoot, release, "graph.tar"), manifest.archive.etag);
    }
    if (manifest?.archive?.url === "/data/moldova-valhalla.tar" && tarPath) {
      etags.set(tarPath, manifest.archive.etag);
    }
    for (const tile of Object.values(manifest?.tiles ?? {})) {
      if (tile?.path && tile?.sha256) {
        etags.set(path.join(dataRoot, release, "tiles", tile.path), tile.etag ?? `"${tile.sha256}"`);
      }
    }
  }

  const server = createServer((request, response) => {
    const started = performance.now();
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    let file;
    let pathname = decodeURIComponent(url.pathname);

    if (pathname === "/data/moldova-valhalla.tar" && tarPath) {
      file = tarPath;
    } else if (pathname.startsWith("/datasets/")) {
      const relative = pathname.slice("/datasets/".length);
      file = path.join(dataRoot, relative);
    } else if (pathname.startsWith("/node_modules/")) {
      file = path.join(root, pathname.slice(1));
    } else {
      file = path.join(root, pathname === "/" ? "spike.html" : pathname.slice(1));
    }

    const resolvedRoot = path.resolve(root);
    const resolvedFile = path.resolve(file);
    const allowed =
      resolvedFile === path.resolve(tarPath ?? "") ||
      resolvedFile.startsWith(resolvedRoot + path.sep) ||
      resolvedFile.startsWith(path.resolve(dataRoot) + path.sep);
    if (!allowed || !existsSync(resolvedFile) || !statSync(resolvedFile).isFile()) {
      response.writeHead(404, { "Content-Type": "text/plain", "Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp" });
      response.end("not found");
      return;
    }

    const stat = statSync(resolvedFile);
    const etag = etags.get(resolvedFile) ?? `"${stat.size.toString(16)}-${Math.floor(stat.mtimeMs).toString(16)}"`;
    const headers = {
      "Content-Type": mime(resolvedFile),
      "Accept-Ranges": "bytes",
      "Cache-Control": "no-store",
      ETag: etag,
      "Last-Modified": stat.mtime.toUTCString(),
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
      "Cross-Origin-Resource-Policy": "same-origin",
    };

    let start = 0;
    let end = stat.size - 1;
    const range = request.headers.range;
    if (range) {
      const match = /^bytes=(\d+)-(\d*)$/.exec(range);
      if (!match) {
        response.writeHead(416, { ...headers, "Content-Range": `bytes */${stat.size}` });
        response.end();
        return;
      }
      start = Number(match[1]);
      end = match[2] ? Number(match[2]) : stat.size - 1;
      if (start > end || start >= stat.size) {
        response.writeHead(416, { ...headers, "Content-Range": `bytes */${stat.size}` });
        response.end();
        return;
      }
      end = Math.min(end, stat.size - 1);
      response.writeHead(206, {
        ...headers,
        "Content-Range": `bytes ${start}-${end}/${stat.size}`,
        "Content-Length": String(end - start + 1),
      });
    } else {
      response.writeHead(200, { ...headers, "Content-Length": String(stat.size) });
    }

    console.log(
      JSON.stringify({
        method: request.method,
        path: pathname,
        range: range ?? null,
        status: response.statusCode,
        bytes: end - start + 1,
        ms: Math.round(performance.now() - started),
      }),
    );

    if (request.method === "HEAD") {
      response.end();
      return;
    }
    createReadStream(resolvedFile, { start, end }).pipe(response);
  });

  return server;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const port = Number(process.argv[2] ?? 8787);
  const tarPath = process.argv[3] ?? process.env.PINPOINT_MOLDOVA_TAR;
  const server = createSpikeServer({ tarPath });
  server.listen(port, "127.0.0.1", () => {
    console.log(`serving ${HERE} on http://127.0.0.1:${port}/spike.html`);
    if (!tarPath) console.log("no tar path: /data/moldova-valhalla.tar is unavailable");
  });
}
