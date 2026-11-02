package org.alwaystogether.core.domain.model

/** Authenticated user (mirrors the API's user response). */
data class User(
    val id: Long,
    val email: String,
    val fullName: String,
    val role: String,
)

/** Result of a successful login. */
data class AuthTokens(
    val accessToken: String,
    val tokenType: String,
    val expiresInSeconds: Int,
    val csrfToken: String,
    val user: User,
)

enum class FriendshipStatus { PENDING, ACCEPTED, REJECTED, BLOCKED }

data class Friendship(
    val id: Long,
    val requesterId: Long,
    val requesterName: String,
    val receiverId: Long,
    val receiverName: String,
    val status: FriendshipStatus,
)

enum class ShareDirection { TO_FRIEND, FROM_FRIEND, MUTUAL }

data class LocationShare(
    val id: Long,
    val friendshipId: Long,
    val ownerId: Long,
    val direction: ShareDirection,
    val isActive: Boolean,
    val precisionLevel: String,
    val lastLocationUpdateAt: String?,
)

/** A friend's latest location as returned by /locations/friends. */
data class FriendLocation(
    val userId: Long,
    val userName: String,
    val latitude: Double,
    val longitude: Double,
    val accuracyMeters: Double?,
    val updatedAt: String,
    val precisionLevel: String,
) {
    /** Freshness derived from updatedAt at call time (server is authoritative). */
    fun isStale(nowMillis: Long, thresholdMillis: Long = 30 * 60 * 1000L): Boolean {
        val then = updatedAt.toEpochMillisOrNull() ?: return true
        return nowMillis - then > thresholdMillis
    }
}

internal fun String.toEpochMillisOrNull(): Long? =
    runCatching {
        java.time.OffsetDateTime.parse(this).toInstant().toEpochMilli()
    }.getOrNull()
