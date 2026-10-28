package org.alwaystogether.core.network.dto

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// DTOs mirror the Go API's JSON exactly (backend/internal/handlers).
// Domain mapping happens in :core:data, never in feature modules.

@Serializable
data class LoginRequest(
    val email: String,
    val password: String,
)

@Serializable
data class UserDto(
    val id: Long,
    val email: String,
    @SerialName("full_name") val fullName: String = "",
    val role: String = "resident",
)

@Serializable
data class LoginResponse(
    @SerialName("access_token") val accessToken: String,
    @SerialName("token_type") val tokenType: String = "bearer",
    @SerialName("expires_in") val expiresIn: Int,
    @SerialName("csrf_token") val csrfToken: String = "",
    val user: UserDto,
)

@Serializable
data class FriendshipDto(
    val id: Long,
    @SerialName("requester_id") val requesterId: Long,
    @SerialName("requester_name") val requesterName: String = "",
    @SerialName("receiver_id") val receiverId: Long,
    @SerialName("receiver_name") val receiverName: String = "",
    val status: String,
)

@Serializable
data class FriendshipRequest(val receiverEmail: String)

@Serializable
data class FriendshipsResponse(val friendships: List<FriendshipDto> = emptyList())

@Serializable
data class RespondRequest(val action: String)

@Serializable
data class ShareRequest(
    @SerialName("friendship_id") val friendshipId: Long,
    val direction: String,
    @SerialName("precision_level") val precisionLevel: String = "exact",
)

@Serializable
data class LocationShareDto(
    val id: Long,
    @SerialName("friendship_id") val friendshipId: Long,
    @SerialName("owner_id") val ownerId: Long,
    val direction: String,
    @SerialName("is_active") val isActive: Boolean,
    @SerialName("precision_level") val precisionLevel: String,
    @SerialName("last_location_update_at") val lastLocationUpdateAt: String? = null,
)

@Serializable
data class SharesResponse(val shares: List<LocationShareDto> = emptyList())

@Serializable
data class LocationUpdateRequest(
    val latitude: Double,
    val longitude: Double,
    val accuracy: Double? = null,
)

@Serializable
data class FriendLocationDto(
    @SerialName("user_id") val userId: Long,
    @SerialName("user_name") val userName: String = "",
    val latitude: Double,
    val longitude: Double,
    val accuracy: Double? = null,
    @SerialName("updated_at") val updatedAt: String,
    @SerialName("precision_level") val precisionLevel: String = "exact",
)

@Serializable
data class FriendsLocationsResponse(
    val friends: List<FriendLocationDto> = emptyList(),
)

// RFC 7807-style error envelope: {code, message, fields?} or {error:{...}}.
@Serializable
data class ErrorEnvelope(
    val code: String? = null,
    val message: String? = null,
    val error: ErrorDetail? = null,
)

@Serializable
data class ErrorDetail(
    val code: String? = null,
    val message: String? = null,
)
