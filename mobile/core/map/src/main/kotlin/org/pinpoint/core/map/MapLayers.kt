package org.pinpoint.core.map

/**
 * Vector style layers available via OpenFreeMap over OSM data (ODbL).
 * Self-hosted deployments point [MapConfig] at their own Martin/TileServer GL
 * serving Geofabrik extracts instead - see docs/OSM_POLICY.md.
 *
 * Data is OSM (© OpenStreetMap contributors). Attribution must stay visible:
 * MapLibre's built-in logo/attribution stays enabled in MapLibreIntegration.
 */
enum class MapLayer(val id: String, val label: String, val styleUrl: String) {
    LIBERTY("liberty", "Liberty (OpenFreeMap)", "https://tiles.openfreemap.org/styles/liberty"),
    BRIGHT("bright", "Bright (OpenFreeMap)", "https://tiles.openfreemap.org/styles/bright"),
    POSITRON("positron", "Positron (OpenFreeMap)", "https://tiles.openfreemap.org/styles/positron"),
}

object MapLayers {
    const val ATTRIBUTION = "© OpenStreetMap contributors"
    val ALL: List<MapLayer> = MapLayer.entries
    fun byId(id: String): MapLayer = MapLayer.entries.firstOrNull { it.id == id } ?: MapLayer.LIBERTY
}
