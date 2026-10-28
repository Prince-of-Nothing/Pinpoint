package org.alwaystogether.core.map

import org.alwaystogether.core.domain.route.GeoPoint
import org.alwaystogether.core.domain.route.Route
import org.alwaystogether.core.domain.route.RoutingEngine

/**
 * Fallback routing engine: straight-line "route" between two points.
 *
 * This is the honest MVP stub from board T14. It computes no real road
 * geometry but keeps the pipeline working (UI -> engine -> geo: export) so a
 * real offline engine can be dropped in later:
 *  - GraphHopper (Apache-2.0) with a pre-built .ghz regional graph, or
 *  - Valhalla (BSD-3) with an mbtiles tile archive,
 * both satisfying the 0-tracker + offline-navigation goals.
 */
class DefaultRoutingEngine : RoutingEngine {

    override val name: String = "straight-line (stub)"
    override val isOfflineCapable: Boolean = true

    override suspend fun route(from: GeoPoint, to: GeoPoint): Route {
        val points = listOf(from, to)
        return Route(
            points = points,
            distanceMeters = haversineMeters(from, to),
            durationSeconds = haversineMeters(from, to) / AVERAGE_SPEED_MPS,
            instructions = listOf("Head toward the destination"),
        )
    }

    private fun haversineMeters(a: GeoPoint, b: GeoPoint): Double {
        val earthRadius = 6_371_000.0
        val dLat = Math.toRadians(b.latitude - a.latitude)
        val dLon = Math.toRadians(b.longitude - a.longitude)
        val h = Math.sin(dLat / 2).let { it * it } +
            Math.cos(Math.toRadians(a.latitude)) *
            Math.cos(Math.toRadians(b.latitude)) *
            Math.sin(dLon / 2).let { it * it }
        return 2 * earthRadius * Math.asin(Math.sqrt(h))
    }

    private companion object {
        /** ~50 km/h average urban speed for the stub's duration estimate. */
        const val AVERAGE_SPEED_MPS = 13.9
    }
}
