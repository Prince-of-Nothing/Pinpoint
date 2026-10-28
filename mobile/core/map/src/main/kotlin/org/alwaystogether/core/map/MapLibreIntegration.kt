package org.alwaystogether.core.map

import android.content.Context
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import org.maplibre.android.MapLibre
import org.maplibre.android.camera.CameraPosition
import org.maplibre.android.camera.CameraUpdateFactory
import org.maplibre.android.geometry.LatLng
import org.maplibre.android.maps.MapView

/**
 * MapLibre Native integration point — together with [OfflineMapManager], one
 * of only two files that import org.maplibre.*. Everything above them (map
 * feature screens) talks to [MapConfig]/[MapLatLng] and [OfflineMapManager]'s
 * plain API, so a MapLibre version bump or SDK swap touches just this module.
 *
 * NOTE: this module was authored without a local Android toolchain; if the
 * CI compile job flags MapLibre API drift, the fix is local to :core:map.
 */
object MapLibreInit {
    /** Must be called once before any MapView is created (App.onCreate). */
    fun initialize(context: Context) {
        MapLibre.getInstance(context)
    }
}

/** A geographic coordinate local to the map layer (avoids leaking SDK types). */
data class MapLatLng(val latitude: Double, val longitude: Double)

/**
 * Compose wrapper around MapLibre's MapView. Style + attribution are applied
 * from [MapConfig]; the attribution text is mandatory for OSM ODbL compliance
 * and is rendered by the SDK's attribution plugin by default.
 */
@Composable
fun MapView(
    modifier: Modifier = Modifier,
    config: MapConfig = MapConfig.DEFAULT,
    center: MapLatLng? = null,
    zoom: Double = 14.0,
    onMapReady: ((org.maplibre.android.maps.MaplibreMap) -> Unit)? = null,
) {
    // The MapView is created and initialized exactly once; later recompositions
    // only adjust the camera. MapLibre's lifecycle methods (onCreate/onResume)
    // are forwarded by the hosting screen via onHostResume/onHostPause.
    val context = androidx.compose.ui.platform.LocalContext.current
    val view = remember {
        MapView(context).apply {
            onCreate(null)
            getMapAsync { map ->
                map.setStyle(config.styleUrl) {
                    // Attribution compliance (ODbL): OSM credit must be visible.
                    map.uiSettings.isAttributionEnabled = true
                    map.uiSettings.isLogoEnabled = true
                }
                center?.let {
                    map.cameraPosition = CameraPosition.Builder()
                        .target(LatLng(it.latitude, it.longitude))
                        .zoom(zoom)
                        .build()
                }
                onMapReady?.invoke(map)
            }
        }
    }

    AndroidView(
        modifier = modifier.fillMaxSize(),
        factory = { view },
    )
}

/** Lifecycle forwarding — call from the hosting screen's lifecycle events. */
fun MapView.onHostResume() = onResume()
fun MapView.onHostPause() = onPause()
fun MapView.onHostDestroy() = onDestroy()

/** Moves the camera (used by "center on me" / friend markers). */
fun org.maplibre.android.maps.MaplibreMap.moveCamera(to: MapLatLng, zoom: Double) {
    val update = CameraUpdateFactory.newLatLngZoom(LatLng(to.latitude, to.longitude), zoom)
    animateCamera(update, 400)
}
