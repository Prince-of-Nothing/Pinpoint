package org.alwaystogether.feature.sharing

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.alwaystogether.core.domain.model.Friendship
import org.alwaystogether.core.domain.model.FriendshipStatus
import org.alwaystogether.core.domain.model.LocationShare
import org.alwaystogether.core.domain.repository.FriendshipRepository
import org.alwaystogether.core.domain.repository.LocationRepository

/**
 * Location-sharing consent management. The core privacy screen: the user
 * decides, per friend, whether they share location (and at what precision).
 */
class SharingViewModel(
    private val locationRepository: LocationRepository,
    private val friendshipRepository: FriendshipRepository,
    private val currentUserId: Long?,
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        val shares: List<LocationShare> = emptyList(),
        val friends: List<Friendship> = emptyList(),
        val message: String? = null,
    ) {
        /** Friends without an active share (candidates to start sharing with). */
        val shareableFriends: List<Friendship>
            get() {
                val sharedWith = shares.map { it.friendshipId }.toSet()
                return friends.filter {
                    it.status == FriendshipStatus.ACCEPTED && it.id !in sharedWith
                }
            }
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            try {
                val shares = locationRepository.listShares()
                    .filter { currentUserId == null || it.ownerId == currentUserId }
                val friends = runCatching { friendshipRepository.list() }.getOrDefault(emptyList())
                _state.value = _state.value.copy(
                    loading = false,
                    shares = shares,
                    friends = friends.filter { it.status == FriendshipStatus.ACCEPTED },
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(loading = false, message = "Network error")
            }
        }
    }

    /** Starts sharing my location with [friendshipId]. */
    fun startSharing(friendshipId: Long, precision: String = "exact") {
        viewModelScope.launch {
            try {
                locationRepository.createShare(friendshipId, "to_friend", precision)
                refresh()
            } catch (e: Exception) {
                _state.value = _state.value.copy(message = "Could not start sharing")
            }
        }
    }

    /** Stops sharing (revokes consent — server-side, not just local). */
    fun stopSharing(shareId: Long) {
        viewModelScope.launch {
            try {
                locationRepository.updateShare(shareId, active = false, precision = null)
                refresh()
            } catch (e: Exception) {
                _state.value = _state.value.copy(message = "Could not stop sharing")
            }
        }
    }

    fun dismissMessage() {
        _state.value = _state.value.copy(message = null)
    }

    companion object {
        fun factory(
            locationRepository: LocationRepository,
            friendshipRepository: FriendshipRepository,
            currentUserId: Long?,
        ) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T =
                SharingViewModel(locationRepository, friendshipRepository, currentUserId) as T
        }
    }
}
