#!/usr/bin/env python3
"""Build an experimental valhalla-browser dataset from the M1 Valhalla tar.

The M1 pack is a plain `tar -cf` of `valhalla_tiles/`. valhalla-browser 0.2.1
requires Valhalla's *native indexed* graph.tar (index.bin first) plus a release
manifest and a browser-adjusted config. This script creates that layout for the
spike **only**. The valhalla-browser maintainers state that a hand-written index
is not a substitute for `valhalla_build_extract`; treat the result as an
experiment, not a shippable pack.

Usage:
  python prepare-sdk-dataset.py --tar <moldova-valhalla.tar> \
      --config <moldova-valhalla.json> --out <output-dir>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import struct
import tarfile

TILE_PREFIX = "valhalla_tiles/"
REVISION = "a60c7cbfc83e073f50887cd27e0109d02e6b64e5"
VERSION = "3.8.3"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tar_header(name: str, size: int, mtime: int = 0) -> bytes:
    header = bytearray(512)
    encoded = name.encode("utf-8")
    if len(encoded) > 100:
        raise ValueError(f"tar name too long: {name}")
    header[0 : len(encoded)] = encoded
    header[100:108] = b"0000644\0"
    header[108:116] = b"0000000\0"
    header[116:124] = b"0000000\0"
    header[124:136] = f"{size:011o}\0".encode("ascii")
    header[136:148] = f"{mtime:011o}\0".encode("ascii")
    header[148:156] = b"        "
    header[156:157] = b"0"
    header[257:263] = b"ustar\0"
    header[263:265] = b"00"
    checksum = sum(header)
    header[148:156] = f"{checksum:06o}\0 ".encode("ascii")
    return bytes(header)


def pad512(data: bytes) -> bytes:
    remainder = len(data) % 512
    return data if remainder == 0 else data + b"\0" * (512 - remainder)


def read_tiles(tar_path: pathlib.Path) -> list[dict]:
    tiles = []
    with tarfile.open(tar_path, "r") as tf:
        for member in tf.getmembers():
            if not member.name.endswith(".gph"):
                continue
            handle = tf.extractfile(member)
            if handle is None:
                raise RuntimeError(f"cannot read {member.name}")
            prefix = handle.read(8)
            graph_id = struct.unpack_from("<Q", prefix)[0] & ((1 << 46) - 1)
            if graph_id & 7 > 2:
                raise RuntimeError(f"unexpected level in {member.name}: {graph_id & 7}")
            path = member.name.removeprefix(TILE_PREFIX)
            if not path.endswith(".gph"):
                raise RuntimeError(f"unexpected path: {member.name}")
            tiles.append(
                {
                    "path": path,
                    "graph_id": graph_id,
                    "offset": member.offset_data,
                    "size": member.size,
                }
            )
    tiles.sort(key=lambda tile: tile["graph_id"])
    return tiles


def write_indexed_archive(tar_path: pathlib.Path, tiles: list[dict], out: pathlib.Path) -> dict:
    index_size = 16 * len(tiles)
    cursor = 512 + ((index_size + 511) // 512) * 512
    offsets = {}
    for tile in tiles:
        # Offset points at the tile body, after its 512-byte tar header.
        offsets[tile["graph_id"]] = cursor + 512
        cursor += 512 + ((tile["size"] + 511) // 512) * 512

    index = bytearray()
    for tile in tiles:
        index += struct.pack("<QII", offsets[tile["graph_id"]], tile["graph_id"], tile["size"])
    index = bytes(index)
    if len(index) != index_size:
        raise RuntimeError("index size mismatch")

    with tarfile.open(tar_path, "r") as tf, out.open("wb") as dest:
        dest.write(tar_header("index.bin", len(index)))
        dest.write(pad512(index))
        position = 512 + len(pad512(index))
        for tile in tiles:
            member = tf.getmember(TILE_PREFIX + tile["path"])
            handle = tf.extractfile(member)
            if handle is None:
                raise RuntimeError(f"cannot read {member.name}")
            if position + 512 != offsets[tile["graph_id"]]:
                raise RuntimeError(
                    f"layout mismatch for {tile['path']}: position={position} expected={offsets[tile['graph_id']]}"
                )
            data = handle.read()
            if len(data) != tile["size"]:
                raise RuntimeError(f"short read for {member.name}")
            dest.write(tar_header(tile["path"], len(data)))
            dest.write(data)
            padding = b"\0" * ((-len(data)) % 512)
            dest.write(padding)
            position += 512 + len(data) + len(padding)
        dest.write(b"\0" * 1024)

    size = out.stat().st_size
    with out.open("rb") as handle:
        header = handle.read(512)
        handle.seek(512)
        built_index = handle.read(len(index))
    if built_index != index:
        raise RuntimeError("index written to archive does not match")
    return {
        "size": size,
        "indexSize": len(index),
        "headerSha256": sha256(header),
        "indexSha256": sha256(index),
        "offsets": offsets,
    }


def browser_config(config_bytes: bytes) -> bytes:
    """Clear native filesystem paths that do not exist inside the WASM MEMFS.

    valhalla-browser's own dataset builder does this; keeping the M1 build's
    /data/... paths makes vb_init fail before any route can run.
    """
    config = json.loads(config_bytes)
    mjolnir = config.setdefault("mjolnir", {})
    for key in (
        "tile_dir",
        "tile_extract",
        "traffic_extract",
        "admin",
        "timezone",
        "landmarks",
        "transit_dir",
        "transit_feeds_dir",
    ):
        mjolnir.pop(key, None)
    config.setdefault("additional_data", {}).pop("elevation", None)
    return (json.dumps(config, indent=2) + "\n").encode("utf-8")


def manifest(release: str, config_bytes: bytes, archive: dict, tiles: list[dict], archive_url: str) -> dict:
    return {
        "schema": 1,
        "release": release,
        "valhallaRevision": REVISION,
        "valhallaVersion": VERSION,
        "costings": ["auto", "pedestrian", "bicycle"],
        "coverage": [26.6, 45.4, 30.2, 48.5],
        "config": {"url": "config.json", "sha256": sha256(config_bytes)},
        "archive": {
            "url": archive_url,
            "size": str(archive["size"]),
            "indexSize": str(archive["indexSize"]),
            "etag": f'"{sha256(str(archive["size"]).encode())[:64]}"',
            "headerSha256": archive["headerSha256"],
            "indexSha256": archive["indexSha256"],
        },
        "tiles": {
            str(tile["graph_id"]): {
                "path": tile["path"],
                "offset": str(archive["offsets"].get(tile["graph_id"], tile["offset"])),
                "size": str(tile["size"]),
                "sha256": tile["sha256"],
            }
            for tile in tiles
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tar", required=True, type=pathlib.Path)
    parser.add_argument("--config", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument(
        "--with-tiles",
        action="store_true",
        help="also write the individual tiles/ tree (another ~120 MB) for the individual-tiles transport test",
    )
    args = parser.parse_args()

    tiles = read_tiles(args.tar)
    config_bytes = browser_config(args.config.read_bytes())

    args.out.mkdir(parents=True, exist_ok=True)
    indexed_dir = args.out / "moldova-20261116"
    indexed_dir.mkdir(exist_ok=True)
    archive_path = indexed_dir / "graph.tar"
    archive = write_indexed_archive(args.tar, tiles, archive_path)
    index_size = archive["indexSize"]
    (indexed_dir / "config.json").write_bytes(config_bytes)

    # Hash each tile body for the manifest; optionally write the individual-tiles tree.
    tiles_dir = indexed_dir / "tiles"
    with tarfile.open(args.tar, "r") as tf:
        for tile in tiles:
            handle = tf.extractfile(TILE_PREFIX + tile["path"])
            if handle is None:
                raise RuntimeError(f"cannot read {tile['path']}")
            data = handle.read()
            tile["sha256"] = sha256(data)
            if args.with_tiles:
                target = tiles_dir / tile["path"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)

    indexed_manifest = manifest("moldova-20261116", config_bytes, archive, tiles, "graph.tar")
    (indexed_dir / "manifest.json").write_text(json.dumps(indexed_manifest, indent=2) + "\n", encoding="utf-8")

    # A deliberately invalid manifest that points at the unmodified M1 tar.
    raw_dir = args.out / "moldova-20261116-raw"
    raw_dir.mkdir(exist_ok=True)
    (raw_dir / "config.json").write_bytes(config_bytes)
    raw_size = args.tar.stat().st_size
    raw = {
        "size": raw_size,
        "indexSize": index_size,
        "headerSha256": sha256(args.tar.read_bytes()[:512]),
        "indexSha256": sha256(args.tar.read_bytes()[512 : 512 + index_size]),
        "offsets": {tile["graph_id"]: tile["offset"] for tile in tiles},
    }
    raw_manifest = manifest("moldova-20261116-raw", config_bytes, raw, tiles, "/data/moldova-valhalla.tar")
    (raw_dir / "manifest.json").write_text(json.dumps(raw_manifest, indent=2) + "\n", encoding="utf-8")

    print(f"tiles: {len(tiles)}")
    print(f"indexed archive: {archive_path} ({archive['size']} bytes)")
    print(f"index: {index_size} bytes, {len(tiles) * 16} expected")
    print(f"raw manifest: {raw_dir / 'manifest.json'}")
    print(f"individual tiles: {'written' if args.with_tiles else 'skipped (use --with-tiles)'}")


if __name__ == "__main__":
    main()
