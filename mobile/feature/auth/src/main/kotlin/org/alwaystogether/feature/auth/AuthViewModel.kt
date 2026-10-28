package org.alwaystogether.feature.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.alwaystogether.core.domain.model.User
import org.alwaystogether.core.domain.repository.ApiException
import org.alwaystogether.core.domain.repository.AuthRepository

/**
 * Login screen state machine. Pure domain-interface dependency: the feature
 * module cannot see Retrofit, OkHttp, DataStore, or the Keystore.
 */
class AuthViewModel(private val authRepository: AuthRepository) : ViewModel() {

    data class UiState(
        val email: String = "",
        val password: String = "",
        val loading: Boolean = false,
        val error: String? = null,
        val loggedInUser: User? = null,
    ) {
        val canSubmit: Boolean
            get() = email.isNotBlank() && password.isNotBlank() && !loading
    }

    private val _state = MutableStateFlow(UiState())
    val state: StateFlow<UiState> = _state.asStateFlow()

    fun onEmailChange(value: String) {
        _state.value = _state.value.copy(email = value, error = null)
    }

    fun onPasswordChange(value: String) {
        _state.value = _state.value.copy(password = value, error = null)
    }

    fun login() {
        val current = _state.value
        if (!current.canSubmit) return
        _state.value = current.copy(loading = true, error = null)
        viewModelScope.launch {
            try {
                val tokens = authRepository.login(current.email, current.password)
                _state.value = _state.value.copy(
                    loading = false,
                    loggedInUser = tokens.user,
                    password = "", // don't keep the password in state
                )
            } catch (e: ApiException) {
                _state.value = _state.value.copy(
                    loading = false,
                    error = when (e.httpStatus) {
                        401 -> "Invalid email or password"
                        429 -> "Too many attempts, try again later"
                        else -> e.message
                    },
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(
                    loading = false,
                    error = "Network error — check your connection",
                )
            }
        }
    }

    companion object {
        fun factory(authRepository: AuthRepository) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T =
                AuthViewModel(authRepository) as T
        }
    }
}
