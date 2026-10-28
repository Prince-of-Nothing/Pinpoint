package org.alwaystogether.core.map

import android.content.Context
import android.content.Intent
import android.net.Uri

/**
 * Offline navigation export (board T14 scope: intent export + engine stub).
 *
 * Builds a standard `geo:` URI intent that any installed navigation app
 * (OsmAnd, Organic Maps, Magic Earth, ...) can consume. If the user has an
 * offline-capable nav app installed with downloaded maps, navigation works
 * fully offline — we do not need to ship a routing engine to offer offline
 * navigation.
 */
object GeoNavigation {

    /**
     * Builds an intent to navigate to [to], optionally routing from [from]
     * (when null the nav app uses the device location).
     *
     * @param label optional place name shown in the nav app.
     */
    fun navigateToIntent(
        to: MapLatLng,
        from: MapLatLng? = null,
        label: String? = null,
    ): Intent {
        val uri = if (from != null) {
            // geo:lat,lon?z=&from=lat,lon — supported by OsmAnd/Organic Maps.
            Uri.parse(
                "geo:${to.latitude},${to.longitude}" +
                    "?from=${from.latitude},${from.longitude}" +
                    (label?.let { "&q=${Uri.encode(it)}" } ?: ""),
            )
        } else {
            Uri.parse(
                "geo:${to.latitude},${to.longitude}" +
                    (label?.let { "?q=${Uri.encode(it)}" } ?: ""),
            )
        }
        return Intent(Intent.ACTION_VIEW, uri)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    /** True when some installed app can handle the geo: intent. */
    fun hasNavigationApp(context: Context, intent: Intent): Boolean =
        intent.resolveActivity(context.packageManager) != null
}
