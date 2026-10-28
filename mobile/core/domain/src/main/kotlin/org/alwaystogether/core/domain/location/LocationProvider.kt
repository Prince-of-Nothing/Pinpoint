package org.alwaystogether.core.domain.location

import kotlinx.coroutines.flow.Flow
import org.alwaystogether.core.domain.route.GeoPoint

/** A device fix with its accuracy and capture time. */
data class DeviceLocation(
    val point: GeoPoint,
    val accuracyMeters: Double?,
    val timestampMillis: Long,
)

/**
 * Device location source abstraction.
 *
 * Implementations live in :core:location (Android LocationManager — the
 * platform API is used deliberately: Google's fused-location provider would
 * pull in Play Services, breaking the 0-tracker baseline).
 *
 * The implementation checks permission internally ([isPermissionGranted]);
 * requesting it is a UI concern and stays in the feature modules.
 */
interface LocationProvider {

    /** True when a location permission (fine or coarse) is granted. */
    val isPermissionGranted: Boolean

    /** One-shot fix: recent last-known, else waits briefly for a fresh one. */
    suspend fun currentLocation(): DeviceLocation?

    /** Continuous fixes; completes (closes) when permission is missing. */
    fun locationUpdates(intervalMillis: Long): Flow<DeviceLocation>
}
