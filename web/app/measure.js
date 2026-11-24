// Distance math for the map's measure tool. Pure so the UI stays testable.

const EARTH_RADIUS_M = 6_371_008.8;

/** Great-circle distance between two `[lat, lon]` points in metres. */
export function distanceMeters([lat1, lon1], [lat2, lon2]) {
  const rad = Math.PI / 180;
  const dLat = (lat2 - lat1) * rad;
  const dLon = (lon2 - lon1) * rad;
  const a =
    Math.sin(dLat / 2) ** 2 +
    Math.cos(lat1 * rad) * Math.cos(lat2 * rad) * Math.sin(dLon / 2) ** 2;
  return 2 * EARTH_RADIUS_M * Math.asin(Math.min(1, Math.sqrt(a)));
}

/** Total length of a polyline in metres; fewer than two points is zero. */
export function lineLengthMeters(points) {
  const clean = (Array.isArray(points) ? points : [])
    .map(([lat, lon]) => [Number(lat), Number(lon)])
    .filter(([lat, lon]) => Number.isFinite(lat) && Number.isFinite(lon));
  let total = 0;
  for (let i = 1; i < clean.length; i += 1) {
    total += distanceMeters(clean[i - 1], clean[i]);
  }
  return total;
}
