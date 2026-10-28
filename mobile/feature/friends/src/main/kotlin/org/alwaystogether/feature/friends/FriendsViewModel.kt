package org.alwaystogether.feature.friends

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.alwaystogether.core.domain.model.Friendship
import org.alwaystogether.core.domain.model.FriendshipStatus
import org.alwaystogether.core.domain.repository.ApiException
import org.alwaystogether.core.domain.repository.FriendshipRepository

/**
 * Friends list + request/accept/reject actions.
 *
 * @param currentUserId the signed-in user's id (from the session store), used
 *   to tell "requests I received" from "requests I sent" — the API returns
 *   both requester and receiver on every row.
 */
class FriendsViewModel(
    private val repository: FriendshipRepository,
    private val currentUserId: Long?,
) : ViewModel() {

    data class UiState(
        val loading: Boolean = true,
        val pendingIncoming: List<Friendship> = emptyList(),
        val active: List<Friendship> = emptyList(),
        val pendingOutgoing: List<Friendship> = emptyList(),
        val inviteEmail: String = "",
        val message: String? = null,
    )

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        viewModelScope.launch {
            try {
                val friends = repository.list()
                val me = currentUserId
                _state.value = _state.value.copy(
                    loading = false,
                    pendingIncoming = friends.filter {
                        it.status == FriendshipStatus.PENDING && it.receiverId == me
                    },
                    active = friends.filter { it.status == FriendshipStatus.ACCEPTED },
                    pendingOutgoing = friends.filter {
                        it.status == FriendshipStatus.PENDING && it.requesterId == me
                    },
                )
            } catch (e: ApiException) {
                _state.value = _state.value.copy(loading = false, message = e.message)
            } catch (e: Exception) {
                _state.value = _state.value.copy(loading = false, message = "Network error")
            }
        }
    }

    fun onInviteEmailChange(value: String) {
        _state.value = _state.value.copy(inviteEmail = value, message = null)
    }

    fun sendRequest() {
        val email = _state.value.inviteEmail.trim()
        if (email.isEmpty()) return
        viewModelScope.launch {
            try {
                repository.request(email)
                _state.value = _state.value.copy(
                    inviteEmail = "",
                    message = "Request sent to $email",
                )
                refresh()
            } catch (e: ApiException) {
                _state.value = _state.value.copy(message = e.message)
            }
        }
    }

    fun accept(friendshipId: Long) = respond(friendshipId, accept = true)
    fun reject(friendshipId: Long) = respond(friendshipId, accept = false)

    private fun respond(friendshipId: Long, accept: Boolean) {
        viewModelScope.launch {
            runCatching { repository.respond(friendshipId, accept) }
            refresh()
        }
    }

    fun dismissMessage() {
        _state.value = _state.value.copy(message = null)
    }

    companion object {
        fun factory(repository: FriendshipRepository, currentUserId: Long?) =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T =
                    FriendsViewModel(repository, currentUserId) as T
            }
    }
}
