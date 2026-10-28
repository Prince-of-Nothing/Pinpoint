package org.alwaystogether.core.map

import android.content.Context
import org.maplibre.android.geometry.LatLng
import org.maplibre.android.geometry.LatLngBounds
import org.maplibre.android.offline.OfflineManager
import org.maplibre.android.offline.OfflineRegion
import org.maplibre.android.offline.OfflineRegionDefinition
import org.maplibre.android.offline.OfflineRegionStatus
import org.maplibre.android.offline.OfflineTilePyramidRegionDefinition
import org.maplibre.android.geometry.LatLngBounds
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * Offline map pack management over MapLibre's OfflineManager.
 *
 * The user picks a bounding box (a city/region) and a zoom range; the SDK
 * downloads all vector tiles for that region into app-private storage. Once
 * downloaded, the map renders that area with zero network access.
 *
 * Data source licensing note (see THIRD_PARTY_NOTICES.md): downloaded tiles
 * inherit the tile provider's terms. With the default OpenFreeMap source the
 * data is OSM ODbL — attribution must remain visible in the UI, which the
 * map view enforces.
 */
class OfflineMapManager(private val context: Context) {

    private val offlineManager: OfflineManager = OfflineManager.getInstance(context)

    /** Progress + completion state for a download. */
    data class DownloadProgress(
        val completedTileCount: Long,
        val requiredTileCount: Long,
        val bytesRemaining: Long,
    ) {
        val fraction: Float
            get() = if (requiredTileCount == 0L) 0f
            else (completedTileCount.toFloat() / requiredTileCount).coerceIn(0f, 1f)
    }

    data class OfflineRegionInfo(
        val id: Long,
        val name: String,
        val downloadedBytes: Long,
        val complete: Boolean,
    )

    /**
     * Downloads all tiles for the bounding box between [north]/[south] and
     * [west]/[east] (degrees) between [minZoom] and [maxZoom].
     * [onProgress] fires during download; the returned job-completion is the
     * suspend return. Callers should run this on Dispatchers.IO.
     */
    suspend fun downloadRegion(
        name: String,
        config: MapConfig,
        north: Double,
        south: Double,
        west: Double,
        east: Double,
        minZoom: Double = 8.0,
        maxZoom: Double = 15.0,
        onProgress: (DownloadProgress) -> Unit = {},
    ): Unit = suspendCancellableCoroutine { cont ->
        val bounds = LatLngBounds.Builder()
            .include(LatLng(north, west))
            .include(LatLng(south, east))
            .build()
        val definition: OfflineRegionDefinition = OfflineTilePyramidRegionDefinition(
            config.styleUrl,
            bounds,
            minZoom,
            maxZoom,
            context.resources.displayMetrics.density,
        )

        offlineManager.createOfflineRegion(
            definition,
            name.toByteArray(Charsets.UTF_8),
            object : OfflineManager.CreateOfflineRegionCallback {
                override fun onCreate(offlineRegion: OfflineRegion) {
                    offlineRegion.setObserver { status: OfflineRegionStatus ->
                        onProgress(
                            DownloadProgress(
                                completedTileCount = status.completedResourceCount,
                                requiredTileCount = status.requiredResourceCount,
                                bytesRemaining = status.remainingTileBytes,
                            ),
                        )
                        if (status.isComplete) {
                            offlineRegion.setDownloadState(OfflineRegion.STATE_INACTIVE)
                            if (cont.isActive) cont.resume(Unit)
                        }
                    }
                    // Download over any network (the user asked for it
                    // explicitly; tiles are small vector tiles).
                    offlineRegion.setDownloadState(OfflineRegion.STATE_ACTIVE)
                }

                override fun onError(error: OfflineManager.Error) {
                    if (cont.isActive) {
                        cont.resumeWithException(
                            OfflineMapException("offline region create failed: ${error.message}"),
                        )
                    }
                }
            },
        )
    }

    /** Lists all stored offline regions (for the downloads screen). */
    suspend fun listRegions(): List<OfflineRegionInfo> =
        suspendCancellableCoroutine { cont ->
            offlineManager.listOfflineRegions(object : OfflineManager.ListOfflineRegionsCallback {
                override fun onList(offlineRegions: Array<OfflineRegion>) {
                    val infos = offlineRegions.map { region ->
                        OfflineRegionInfo(
                            id = region.id,
                            name = String(region.metadata, Charsets.UTF_8),
                            downloadedBytes = region.status.completedResourceSize,
                            complete = region.status.isComplete,
                        )
                    }
                    if (cont.isActive) cont.resume(infos)
                }

                override fun onError(error: OfflineManager.Error) {
                    if (cont.isActive) {
                        cont.resumeWithException(
                            OfflineMapException("list offline regions failed: ${error.message}"),
                        )
                    }
                }
            })
        }

    /** Deletes a stored region and frees its tile storage. */
    suspend fun deleteRegion(regionId: Long): Unit = suspendCancellableCoroutine { cont ->
        offlineManager.listOfflineRegions(object : OfflineManager.ListOfflineRegionsCallback {
            override fun onList(offlineRegions: Array<OfflineRegion>) {
                val region = offlineRegions.firstOrNull { it.id == regionId }
                if (region == null) {
                    if (cont.isActive) cont.resume(Unit)
                    return
                }
                region.delete(object : OfflineRegion.OfflineRegionDeleteCallback {
                    override fun onDelete() {
                        if (cont.isActive) cont.resume(Unit)
                    }

                    override fun onError(error: OfflineRegion.OfflineRegionError) {
                        if (cont.isActive) {
                            cont.resumeWithException(
                                OfflineMapException("delete failed: ${error.message}"),
                            )
                        }
                    }
                })
            }

            override fun onError(error: OfflineManager.Error) {
                if (cont.isActive) {
                    cont.resumeWithException(
                        OfflineMapException("list for delete failed: ${error.message}"),
                    )
                }
            }
        })
    }

    /** Releases the shared OfflineManager (call from Application teardown). */
    fun destroy() {
        offlineManager.destroy()
    }
}

class OfflineMapException(message: String) : Exception(message)
