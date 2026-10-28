package org.alwaystogether.feature.friends

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import org.alwaystogether.core.domain.model.Friendship

@Composable
fun FriendsScreen(viewModel: FriendsViewModel) {
    val state by viewModel.state.collectAsStateWithLifecycle()

    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Text("Friends", style = MaterialTheme.typography.headlineSmall)
        }

        // Invite by email
        item {
            Card {
                Column(modifier = Modifier.padding(12.dp)) {
                    OutlinedTextField(
                        value = state.inviteEmail,
                        onValueChange = viewModel::onInviteEmailChange,
                        label = { Text("Friend's email") },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Button(
                        onClick = viewModel::sendRequest,
                        enabled = state.inviteEmail.isNotBlank(),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp),
                    ) {
                        Text("Send request")
                    }
                }
            }
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

        if (state.pendingIncoming.isNotEmpty()) {
            item { Text("Requests", style = MaterialTheme.typography.titleMedium) }
            items(state.pendingIncoming, key = { "in-${it.id}" }) { friendship ->
                FriendshipRow(
                    friendship = friendship,
                    displayName = friendship.requesterName,
                    actions = {
                        TextButton(onClick = { viewModel.accept(friendship.id) }) {
                            Text("Accept")
                        }
                        TextButton(onClick = { viewModel.reject(friendship.id) }) {
                            Text("Reject")
                        }
                    },
                )
            }
        }

        item { Text("Your circle", style = MaterialTheme.typography.titleMedium) }
        if (state.loading && state.active.isEmpty()) {
            item { Text("Loading…", style = MaterialTheme.typography.bodyMedium) }
        } else if (state.active.isEmpty()) {
            item {
                Text(
                    "No friends yet — invite someone by email above.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        } else {
            items(state.active, key = { "active-${it.id}" }) { friendship ->
                val other =
                    if (friendship.requesterName != "") friendship.requesterName
                    else friendship.receiverName
                FriendshipRow(friendship = friendship, displayName = other, actions = {})
            }
        }

        if (state.pendingOutgoing.isNotEmpty()) {
            item { Text("Sent requests", style = MaterialTheme.typography.titleMedium) }
            items(state.pendingOutgoing, key = { "out-${it.id}" }) { friendship ->
                FriendshipRow(
                    friendship = friendship,
                    displayName = friendship.receiverName,
                    actions = { Text("Pending", style = MaterialTheme.typography.labelMedium) },
                )
            }
        }
    }
}

@Composable
private fun FriendshipRow(
    friendship: Friendship,
    displayName: String,
    actions: @Composable () -> Unit,
) {
    Card {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
        ) {
            Text(
                text = displayName.ifBlank { "Unknown" },
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f),
            )
            actions()
        }
    }
}
