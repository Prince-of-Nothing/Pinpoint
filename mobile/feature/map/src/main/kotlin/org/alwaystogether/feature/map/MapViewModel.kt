package org.alwaystogether.feature.map

import android.content.Intent
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.alwaystogether.core.map.MapConfig
import org.alwaystogether.core.map.MapLatLng
import org.alwaystogether.core.map.OfflineMapException
import org.alwaystogether.core.map.OfflineMapManager

/**
 * Map screen state: current center, offline downloads list, download state.
 * The ViewModel talks to [OfflineMapManager] through its own thin API, so
 * features never touch MapLibre classes.
 */
class MapViewModel(
    private val offlineMaps: OfflineMapManager,
    private val mapConfig: MapConfig = MapConfig.DEFAULT,
) : ViewModel() {

    data class DownloadRequest(val name: String, val center: MapLatLng, val radiusDegrees: Double)

    data class UiState(
        val center: MapLatLng? = null,
        val zoom: Double = 14.0,
        val regions: List<OfflineMapManager.OfflineRegionInfo> = emptyList(),
        val downloading: Boolean = false,
        val downloadFraction: Float = 0f,
        val message: String? = null,
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        refreshRegions()
    }

    fun centerOn(point: MapLatLng) {
        _state.value = _state.value.copy(center = point)
    }

    fun refreshRegions() {
        viewModelScope.launch(Dispatchers.IO) {
            val regions = runCatching { offlineMaps.listRegions() }.getOrDefault(emptyList())
            _state.value = _state.value.copy(regions = regions)
        }
    }

    /** Downloads an offline pack around [request]'s center. */
    fun downloadRegion(request: DownloadRequest) {
        if (_state.value.downloading) return
        _state.value = _state.value.copy(downloading = true, downloadFraction = 0f, message = null)
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val half = request.radiusDegrees
                offlineMaps.downloadRegion(
                    name = request.name,
                    config = mapConfig,
                    north = request.center.latitude + half,
                    south = request.center.latitude - half,
                    west = request.center.longitude - half,
                    east = request.center.longitude + half,
                    onProgress = { progress ->
                        _state.value = _state.value.copy(downloadFraction = progress.fraction)
                    },
                )
                _state.value = _state.value.copy(
                    downloading = false,
                    message = "“${request.name}” downloaded — available offline",
                )
                refreshRegions()
            } catch (e: OfflineMapException) {
                _state.value = _state.value.copy(downloading = false, message = e.message)
            }
        }
    }

    fun deleteRegion(regionId: Long) {
        viewModelScope.launch(Dispatchers.IO) {
            runCatching { offlineMaps.deleteRegion(regionId) }
            refreshRegions()
        }
    }

    fun dismissMessage() {
        _state.value = _state.value.copy(message = null)
    }

    companion object {
        fun factory(offlineMaps: OfflineMapManager) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T =
                MapViewModel(offlineMaps) as T
        }
    }
}

/** Builds the navigation intent for a destination (delegates to :core:map). */
fun navigationIntent(to: MapLatLng, from: MapLatLng?): Intent =
    org.alwaystogether.core.map.GeoNavigation.navigateToIntent(to, from)
