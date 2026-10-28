package org.alwaystogether.core.domain.repository

import org.alwaystogether.core.domain.model.AuthTokens
import org.alwaystogether.core.domain.model.Friendship
import org.alwaystogether.core.domain.model.FriendLocation
import org.alwaystogether.core.domain.model.LocationShare
import org.alwaystogether.core.domain.model.User

/**
 * Thrown by every repository method. [code] mirrors the API's error envelope
 * (RFC 7807 style: code + message), so UI can branch on codes, not strings.
 */
class ApiException(
    val code: String,
    override val message: String,
    val httpStatus: Int? = null,
) : Exception(message)

/** Authentication + current-user operations. */
interface AuthRepository {
    suspend fun login(email: String, password: String): AuthTokens
    suspend fun logout()
    suspend fun me(): User
    /** True when a session token is stored locally (fast pre-login gate). */
    fun hasSession(): Boolean
}

/** Friend requests and lifecycle. */
interface FriendshipRepository {
    suspend fun list(): List<Friendship>
    suspend fun request(receiverEmail: String): Friendship
    suspend fun respond(friendshipId: Long, accept: Boolean): Friendship
}

/** Directional location sharing. */
interface LocationRepository {
    suspend fun listShares(): List<LocationShare>
    suspend fun createShare(friendshipId: Long, direction: String, precision: String): LocationShare
    suspend fun updateShare(shareId: Long, active: Boolean, precision: String?): LocationShare
    /** Publish own location; the only place coordinates leave the device. */
    suspend fun publishLocation(latitude: Double, longitude: Double, accuracyMeters: Double?)
    /** Consent-filtered latest locations of friends. */
    suspend fun friendsLatest(): List<FriendLocation>
}

/** Local session persistence (token storage contract). */
interface SessionRepository {
    suspend fun save(tokens: AuthTokens)
    suspend fun clear()
    fun accessToken(): String?
    fun csrfToken(): String?
}
