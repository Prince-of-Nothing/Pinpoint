package org.alwaystogether.core.data

import org.alwaystogether.core.domain.model.AuthTokens
import org.alwaystogether.core.domain.model.Friendship
import org.alwaystogether.core.domain.model.FriendLocation
import org.alwaystogether.core.domain.model.LocationShare
import org.alwaystogether.core.domain.model.ShareDirection
import org.alwaystogether.core.domain.model.User
import org.alwaystogether.core.domain.repository.ApiException
import org.alwaystogether.core.domain.repository.AuthRepository
import org.alwaystogether.core.domain.repository.FriendshipRepository
import org.alwaystogether.core.domain.repository.LocationRepository
import org.alwaystogether.core.network.ApiClient
import org.alwaystogether.core.network.AuthApi
import org.alwaystogether.core.network.FriendshipApi
import org.alwaystogether.core.network.LocationApi
import org.alwaystogether.core.network.dto.FriendLocationDto
import org.alwaystogether.core.network.dto.FriendshipDto
import org.alwaystogether.core.network.dto.LocationShareDto
import org.alwaystogether.core.network.dto.LoginRequest

/**
 * Repository implementations: network DTOs -> domain models.
 * This is the only layer allowed to know both representations.
 */

class AuthRepositoryImpl(
    private val api: AuthApi,
    private val session: SessionStore,
) : AuthRepository {

    override suspend fun login(email: String, password: String): AuthTokens {
        val response = ApiClient.run {
            api.login(LoginRequest(email = email.trim(), password = password))
        }
        val tokens = AuthTokens(
            accessToken = response.accessToken,
            tokenType = response.tokenType,
            expiresInSeconds = response.expiresIn,
            csrfToken = response.csrfToken,
            user = response.user.toDomain(),
        )
        session.login(tokens) // encrypted persistence + in-memory cache
        return tokens
    }

    override suspend fun logout() {
        runCatching { ApiClient.run { api.logout() } }
        session.clear() // local wipe even if the network call failed
    }

    override suspend fun me(): User = ApiClient.run { api.me() }.toDomain()

    override fun hasSession(): Boolean = session.accessToken() != null

    private fun org.alwaystogether.core.network.dto.UserDto.toDomain() =
        User(id = id, email = email, fullName = fullName, role = role)
}

class FriendshipRepositoryImpl(private val api: FriendshipApi) : FriendshipRepository {

    override suspend fun list(): List<Friendship> =
        ApiClient.run { api.list() }.friendships.map { it.toDomain() }

    override suspend fun request(receiverEmail: String): Friendship =
        ApiClient.run {
            api.request(org.alwaystogether.core.network.dto.FriendshipRequest(receiverEmail.trim()))
        }.toDomain()

    override suspend fun respond(friendshipId: Long, accept: Boolean): Friendship =
        ApiClient.run {
            api.respond(
                friendshipId,
                org.alwaystogether.core.network.dto.RespondRequest(if (accept) "accept" else "reject"),
            )
        }.toDomain()

    private fun FriendshipDto.toDomain() = Friendship(
        id = id,
        requesterId = requesterId,
        requesterName = requesterName,
        receiverId = receiverId,
        receiverName = receiverName,
        status = org.alwaystogether.core.domain.model.FriendshipStatus.valueOf(status.uppercase()),
    )
}

class LocationRepositoryImpl(private val api: LocationApi) : LocationRepository {

    override suspend fun listShares(): List<LocationShare> =
        ApiClient.run { api.listShares() }.shares.map { it.toDomain() }

    override suspend fun createShare(
        friendshipId: Long,
        direction: String,
        precision: String,
    ): LocationShare = ApiClient.run {
        api.createShare(
            org.alwaystogether.core.network.dto.ShareRequest(
                friendshipId = friendshipId,
                direction = direction,
                precisionLevel = precision,
            ),
        )
    }.toDomain()

    override suspend fun updateShare(shareId: Long, active: Boolean, precision: String?): LocationShare =
        ApiClient.run {
            api.updateShare(
                shareId,
                buildMap {
                    put("is_active", active)
                    precision?.let { put("precision_level", it) }
                },
            )
        }.toDomain()

    override suspend fun publishLocation(latitude: Double, longitude: Double, accuracyMeters: Double?) {
        ApiClient.run {
            api.publishLocation(
                org.alwaystogether.core.network.dto.LocationUpdateRequest(
                    latitude = latitude,
                    longitude = longitude,
                    accuracy = accuracyMeters,
                ),
            )
        }
    }

    override suspend fun friendsLatest(): List<FriendLocation> =
        ApiClient.run { api.friendsLatest() }.friends.map { it.toDomain() }

    private fun LocationShareDto.toDomain() = LocationShare(
        id = id,
        friendshipId = friendshipId,
        ownerId = ownerId,
        direction = ShareDirection.valueOf(direction.uppercase()),
        isActive = isActive,
        precisionLevel = precisionLevel,
        lastLocationUpdateAt = lastLocationUpdateAt,
    )

    private fun FriendLocationDto.toDomain() = FriendLocation(
        userId = userId,
        userName = userName,
        latitude = latitude,
        longitude = longitude,
        accuracyMeters = accuracy,
        updatedAt = updatedAt,
        precisionLevel = precisionLevel,
    )
}
