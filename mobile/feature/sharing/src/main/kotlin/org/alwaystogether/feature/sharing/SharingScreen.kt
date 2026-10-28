package org.alwaystogether.feature.sharing

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.alwaystogether.core.domain.model.LocationShare

/**
 * Privacy-first sharing screen: one switch per friend, plus an explicit
 * statement that revocation is server-side.
 */
@Composable
fun SharingScreen(viewModel: SharingViewModel) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Text("Location sharing", style = MaterialTheme.typography.headlineSmall)
        }
        item {
            Text(
                "You control exactly who sees your location and when. " +
                    "Turning sharing off revokes access on the server immediately.",
                style = MaterialTheme.typography.bodySmall,
            )
        }

        state.message?.let { message ->
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        message,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = viewModel::dismissMessage) {
                        Text("Dismiss")
                    }
                }
            }
        }

        if (state.loading && state.shares.isEmpty()) {
            item { Text("Loading…", style = MaterialTheme.typography.bodyMedium) }
        }

        if (state.shares.isEmpty() && !state.loading) {
            item {
                Text(
                    "Not sharing your location with anyone.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }

        items(state.shares, key = { "share-${it.id}" }) { share ->
            ShareRow(
                share = share,
                friendName = state.friends
                    .firstOrNull { it.id == share.friendshipId }
                    ?.let { if (it.requesterName != "") it.requesterName else it.receiverName }
                    .orEmpty(),
                onToggle = { enabled ->
                    if (enabled) viewModel.startSharing(share.friendshipId, share.precisionLevel)
                    else viewModel.stopSharing(share.id)
                },
            )
        }

        if (state.shareableFriends.isNotEmpty()) {
            item {
                Text("Start sharing with", style = MaterialTheme.typography.titleMedium)
            }
            items(state.shareableFriends, key = { "friend-${it.id}" }) { friendship ->
                val name = if (friendship.requesterName != "") friendship.requesterName
                else friendship.receiverName
                Card {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    ) {
                        Text(
                            text = name.ifBlank { "Unknown" },
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                        TextButton(onClick = { viewModel.startSharing(friendship.id) }) {
                            Text("Share")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ShareRow(
    share: LocationShare,
    friendName: String,
    onToggle: (Boolean) -> Unit,
) {
    Card {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = friendName.ifBlank { "Friend #${share.friendshipId}" },
                    style = MaterialTheme.typography.bodyLarge,
                )
                Text(
                    text = "Precision: ${share.precisionLevel}",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            Switch(checked = share.isActive, onCheckedChange = onToggle)
        }
    }
}
