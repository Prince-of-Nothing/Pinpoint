package org.alwaystogether.feature.map

import android.content.Intent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.alwaystogether.core.map.MapConfig
import org.alwaystogether.core.map.MapLatLng
import org.alwaystogether.core.map.MapView

/**
 * Map screen: full-screen map + a downloads panel. The map view enforces
 * OSM attribution (MapLibre renders logo + attribution automatically).
 */
@Composable
fun MapScreen(viewModel: MapViewModel) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val context = LocalContext.current

    Box(modifier = Modifier.fillMaxSize()) {
        MapView(
            config = MapConfig.DEFAULT,
            center = state.center ?: defaultCenter(),
        )

        Card(
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .padding(8.dp),
        ) {
            Column(modifier = Modifier.padding(12.dp)) {
                Text("Offline maps", style = MaterialTheme.typography.titleMedium)

                if (state.downloading) {
                    LinearProgressIndicator(
                        progress = { state.downloadFraction },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp),
                    )
                    Text(
                        "Downloading tiles… keep the app open",
                        style = MaterialTheme.typography.bodySmall,
                    )
                } else {
                    Button(
                        onClick = {
                            val center = state.center ?: defaultCenter()
                            viewModel.downloadRegion(
                                MapViewModel.DownloadRequest(
                                    name = "Current area",
                                    center = center,
                                    radiusDegrees = 0.1, // ~11 km box
                                ),
                            )
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp),
                    ) {
                        Text("Download current area")
                    }
                }

                state.regions.forEach { region ->
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(region.name, style = MaterialTheme.typography.bodyMedium)
                            Text(
                                if (region.complete) "Available offline"
                                else "Partial — redownload",
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                        TextButton(onClick = { viewModel.deleteRegion(region.id) }) {
                            Text("Delete")
                        }
                    }
                }

                // Navigation hand-off: export destination to any installed
                // nav app (works offline if that app has offline maps).
                OutlinedButton(
                    onClick = {
                        val center = state.center ?: defaultCenter()
                        val intent = navigationIntent(center, from = null)
                        runCatching {
                            context.startActivity(
                                Intent.createChooser(intent, "Navigate with"),
                            )
                        }
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp),
                ) {
                    Text("Navigate here (opens nav app)")
                }

                state.message?.let { message ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(
                            message,
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(onClick = viewModel::dismissMessage) {
                            Text("OK")
                        }
                    }
                }

                Text(
                    "© OpenStreetMap contributors",
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(top = 4.dp),
                )
            }
        }
    }
}

/** Fallback center: Chișinău (deployment default; app will center on GPS later). */
private fun defaultCenter() = MapLatLng(latitude = 47.0105, longitude = 28.8638)
