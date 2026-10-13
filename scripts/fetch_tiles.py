#!/usr/bin/env python3
"""
fetch_tiles.py — the "maps-requests" workflow.

Downloads OpenStreetMap XYZ tiles ONCE per requested region and stores them
as a tile pack committed to this repository under maps-requests/<name>/.
Clients (web SPA, weba/webk, Kotlin mobile app) then download tiles from our
own GitHub repo / release assets / deployed server — never hammering the
public OSM tile servers with repeated traffic.

Usage:
    python3 scripts/fetch_tiles.py \
        --name paris \
        --bbox 48.81,2.29,48.90,2.40 \
        --zoom 11-13 \
        [--out maps-requests] [--upstream https://tile.openstreetmap.org] \
        [--workers 2] [--delay 0.05]

Resumable: existing PNGs are skipped, so re-running after an interruption
only fetches what is missing. A manifest.json (bbox, zooms, per-tile size +
sha256) is regenerated at the end so apps can plan selective downloads.

NOTE: this maintenance script runs on a developer machine or CI, not in the
server runtime (the backend itself stays pure Go). Respect the OSM tile
usage policy: identify yourself via --user-agent / TILES_USER_AGENT.
"""

import argparse
import concurrent.futures as cf
import hashlib
import json
import os
import sys
import time
import urllib.request

MAX_TILES_WARN = 50_000


def deg2tile(z: int, lat: float, lon: float):
    n = 2 ** z
    x = int((lon + 180.0) / 360.0 * n)
    lat_rad = math.radians(lat)
    y = int((1.0 - math.log(math.tan(lat_rad) + 1 / math.cos(lat_rad)) / math.pi) / 2.0 * n)
    return max(0, min(n - 1, x)), max(0, min(n - 1, y))


import math  # noqa: E402  (kept after helpers for readability of formulas)


def tile_range(z, s, w, n_, e):
    xmin, _ = deg2tile(z, n_, w)
    xmax, _ = deg2tile(z, s, e)
    _, ymin = deg2tile(z, n_, w)
    _, ymax = deg2tile(z, s, e)
    return xmin, xmax, ymin, ymax


def fetch_tile(upstream: str, rel: str, dest: str, ua: str, delay: float) -> tuple[str, int, str]:
    if os.path.exists(dest):
        data = open(dest, "rb").read()
        return rel, len(data), "skipped"
    url = f"{upstream.rstrip('/')}/{rel}"
    req = urllib.request.Request(url, headers={"User-Agent": ua})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=20) as resp:
                data = resp.read()
            if not data.startswith(b"\x89PNG"):
                raise ValueError(f"non-PNG payload for {rel}")
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            tmp = dest + ".tmp"
            with open(tmp, "wb") as fh:
                fh.write(data)
            os.replace(tmp, dest)
            time.sleep(delay)
            return rel, len(data), "fetched"
        except Exception as exc:  # retry with backoff
            if attempt == 2:
                raise SystemExit(f"failed to fetch {rel}: {exc}")
            time.sleep(1.5 * (attempt + 1))
    raise AssertionError


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--name", required=True, help="pack name (folder under --out)")
    ap.add_argument("--bbox", required=True, help="south,west,north,east in degrees")
    ap.add_argument("--zoom", required=True, help="single zoom (e.g. 12) or range (e.g. 11-13)")
    ap.add_argument("--out", default="maps-requests", help="repo directory for packs")
    ap.add_argument("--upstream", default=os.environ.get("TILES_UPSTREAM", "https://tile.openstreetmap.org"))
    ap.add_argument("--ua", default=os.environ.get(
        "TILES_USER_AGENT",
        "AlwaysTogether/1.0 (+https://github.com/always-together/always-together; contact@your-domain.example)"))
    ap.add_argument("--workers", type=int, default=2, help="parallel downloads (keep low!)")
    ap.add_argument("--delay", type=float, default=0.05, help="seconds between requests per worker")
    args = ap.parse_args()

    s, w, n_, e = (float(v) for v in args.bbox.split(","))
    if not (-90 <= s < n_ <= 90 and -180 <= w < e <= 180):
        raise SystemExit("invalid bbox (need south<north, west<east within valid ranges)")
    if "-" in args.zoom:
        zmin, zmax = (int(v) for v in args.zoom.split("-"))
    else:
        zmin = zmax = int(args.zoom)

    jobs = []
    for z in range(zmin, zmax + 1):
        xmin, xmax, ymin, ymax = tile_range(z, s, w, n_, e)
        for x in range(xmin, xmax + 1):
            for y in range(ymin, ymax + 1):
                jobs.append((z, x, y))
    if len(jobs) > MAX_TILES_WARN:
        raise SystemExit(f"region needs {len(jobs)} tiles (> {MAX_TILES_WARN}); narrow bbox/zoom")

    pack_dir = os.path.join(args.out, args.name)
    print(f"pack '{args.name}': {len(jobs)} tile(s) z{zmin}-{zmax} bbox {s},{w},{n_},{e} -> {pack_dir}")

    tiles_meta = {}
    with cf.ThreadPoolExecutor(max_workers=args.workers) as pool:
        futs = []
        for (z, x, y) in jobs:
            rel = f"{z}/{x}/{y}.png"
            dest = os.path.join(pack_dir, rel)
            futs.append(pool.submit(fetch_tile, args.upstream, rel, dest, args.ua, args.delay))
        done = 0
        for fut in cf.as_completed(futs):
            rel, size, status = fut.result()
            h = hashlib.sha256(open(os.path.join(pack_dir, rel), "rb").read()).hexdigest()
            tiles_meta[rel] = {"bytes": size, "sha256": h}
            done += 1
            if done % 25 == 0 or done == len(jobs):
                print(f"  {done}/{len(jobs)} ({status} {rel})")

    manifest = {
        "name": args.name,
        "scheme": "xyz",
        "projection": "EPSG:3857",
        "bbox": {"south": s, "west": w, "north": n_, "east": e},
        "zoom_min": zmin,
        "zoom_max": zmax,
        "tile_count": len(tiles_meta),
        "total_bytes": sum(t["bytes"] for t in tiles_meta.values()),
        "attribution": "(c) OpenStreetMap contributors, ODbL — https://www.openstreetmap.org/copyright",
        "source": args.upstream,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "tiles": dict(sorted(tiles_meta.items())),
    }
    os.makedirs(pack_dir, exist_ok=True)
    with open(os.path.join(pack_dir, "manifest.json"), "w") as fh:
        json.dump(manifest, fh, indent=1)
    print(f"wrote {pack_dir}/manifest.json ({manifest['total_bytes']} bytes of tiles)")
    print("next: git add maps-requests/<name> && commit — clients download from GitHub/release/server cache.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
