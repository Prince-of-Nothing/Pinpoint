package org.alwaystogether.core.network

import org.alwaystogether.core.network.dto.FriendLocationDto
import org.alwaystogether.core.network.dto.FriendshipDto
import org.alwaystogether.core.network.dto.FriendshipRequest
import org.alwaystogether.core.network.dto.FriendshipsResponse
import org.alwaystogether.core.network.dto.LocationShareDto
import org.alwaystogether.core.network.dto.LocationUpdateRequest
import org.alwaystogether.core.network.dto.LoginRequest
import org.alwaystogether.core.network.dto.LoginResponse
import org.alwaystogether.core.network.dto.RespondRequest
import org.alwaystogether.core.network.dto.ShareRequest
import org.alwaystogether.core.network.dto.SharesResponse
import org.alwaystogether.core.network.dto.UserDto
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.PATCH
import retrofit2.http.POST
import retrofit2.http.Path

// Endpoint paths mirror backend/internal/handlers/app.go route registration.

interface AuthApi {
    @POST("auth/login")
    suspend fun login(@Body body: LoginRequest): LoginResponse

    @POST("auth/logout")
    suspend fun logout()

    @GET("auth/me")
    suspend fun me(): UserDto
}

interface FriendshipApi {
    @GET("friendships")
    suspend fun list(): FriendshipsResponse

    @POST("friendships")
    suspend fun request(@Body body: FriendshipRequest): FriendshipDto

    @POST("friendships/{id}/respond")
    suspend fun respond(@Path("id") id: Long, @Body body: RespondRequest): FriendshipDto
}

interface LocationApi {
    @GET("locations/shares")
    suspend fun listShares(): SharesResponse

    @POST("locations/shares")
    suspend fun createShare(@Body body: ShareRequest): LocationShareDto

    @PATCH("locations/shares/{id}")
    suspend fun updateShare(
        @Path("id") id: Long,
        @Body body: Map<String, @JvmSuppressWildcards Any>,
    ): LocationShareDto

    @POST("locations/me")
    suspend fun publishLocation(@Body body: LocationUpdateRequest)

    @GET("locations/friends")
    suspend fun friendsLatest(): FriendsLocationsResponse
}
