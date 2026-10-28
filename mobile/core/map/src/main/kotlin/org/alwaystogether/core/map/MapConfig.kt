package org.alwaystogether.core.map

/**
 * Map configuration. The tile/style source is data, not code — the app ships
 * a privacy-preserving default and could be pointed at a self-hosted server
 * without recompiling the map layer.
 *
 * Default: OpenFreeMap (openfreemap.org) — community-run, donation-funded,
 * zero API keys, zero telemetry, vector tiles derived from OpenStreetMap
 * data (ODbL). Alternative providers a deployment may configure:
 *  - Self-hosted tile server (e.g. Martin/Tegola serving Geofabrik extracts)
 *  - MapTiler Cloud (free tier; requires API key, has its own ToS)
 */
data class MapConfig(
    /** URL of the vector tile source (TileJSON or style URL). */
    val tileSourceUrl: String,
    /** Base URL used to download the style document. */
    val styleUrl: String,
    /** Attribution string that MUST be displayed on the map (ODbL). */
    val attribution: String = "© OpenStreetMap contributors",
) {
    companion object {
        /** Privacy-preserving default: OpenFreeMap (no keys, no telemetry). */
        val DEFAULT = MapConfig(
            tileSourceUrl = "https://tiles.openfreemap.org/planet",
            styleUrl = "https://tiles.openfreemap.org/styles/liberty",
        )
    }
}
