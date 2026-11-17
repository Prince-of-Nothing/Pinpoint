const decoder = new TextDecoder();

/** Read a NUL-terminated tar text field. */
function text(bytes) {
  return decoder.decode(bytes).replace(/\0.*$/, "");
}

/**
 * Parse an uncompressed tar into path -> Uint8Array views of the original buffer.
 * Supports the ustar/GNU headers emitted by the M1 pack; directory entries are
 * skipped. Each view aliases the source buffer, so keep it alive while routing.
 */
export function parseTar(buffer) {
  const bytes = new Uint8Array(buffer);
  const tiles = new Map();
  let offset = 0;
  while (offset + 512 <= bytes.length) {
    const header = bytes.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const name = text(header.subarray(0, 100));
    const sizeText = text(header.subarray(124, 136)).trim();
    const size = Number.parseInt(sizeText, 8);
    if (!Number.isFinite(size) || size < 0) throw new Error(`bad tar size for ${name}: ${sizeText}`);
    const dataStart = offset + 512;
    if (dataStart + size > bytes.length) throw new Error(`truncated tar entry ${name}`);
    if (name.endsWith(".gph")) {
      tiles.set(name.replace(/^valhalla_tiles\//, ""), bytes.subarray(dataStart, dataStart + size));
    }
    offset = dataStart + Math.ceil(size / 512) * 512;
  }
  return tiles;
}
