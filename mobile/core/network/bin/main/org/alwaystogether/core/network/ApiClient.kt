package org.alwaystogether.core.network

import java.util.concurrent.TimeUnit
import kotlinx.serialization.json.Json
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Response
import okhttp3.logging.HttpLoggingInterceptor
import org.alwaystogether.core.network.dto.ErrorEnvelope
import org.alwaystogether.retrofit2.kotlinx.serialization.converter.asConverterFactory
import retrofit2.Retrofit
import retrofit2.HttpException

/**
 * Retrofit/OkHttp wiring. The ONLY place that knows HTTP details.
 *
 * Security properties:
 *  - Bearer token injected from an in-memory provider (never logged).
 *  - Logging is BODY-free in release: only method/URL/status are logged, so
 *    tokens, passwords and locations never reach logcat. (0-tracker policy:
 *    no analytics/crash SDKs are pulled in here or anywhere.)
 *  - Errors are normalized to [org.alwaystogether.core.domain.repository.ApiException]
 *    with the server's error code, so features branch on codes, not strings.
 */
class ApiClient(baseUrl: String) {

    // In-memory only: tokens live in the process, persisted separately by
    // :core:data (Keystore-backed). Set right after login, cleared on logout.
    @Volatile
    var tokenProvider: () -> String? = { null }

    @Volatile
    var csrfProvider: () -> String? = { null }

    private val json = Json {
        ignoreUnknownKeys = true // server may add fields; don't crash clients
        coerceInputValues = true // null -> non-null Kotlin defaults
    }

    private val authInterceptor = Interceptor { chain ->
        val original = chain.request()
        val authorized = original.newBuilder().apply {
            tokenProvider()?.let { header("Authorization", "Bearer $it") }
            if (original.method == "POST" || original.method == "PATCH" || original.method == "DELETE") {
                csrfProvider()?.let { header("X-CSRF-Token", it) }
            }
        }.build()
        chain.proceed(authorized)
    }

    private val logging = HttpLoggingInterceptor().apply {
        // REDACTED: never log headers/body — they carry tokens and locations.
        level = HttpLoggingInterceptor.Level.BASIC
    }

    private val http = OkHttpClient.Builder()
        .addInterceptor(authInterceptor)
        .addInterceptor(logging)
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .build()

    private val retrofit = Retrofit.Builder()
        .baseUrl(baseUrl)
        .client(http)
        .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
        .build()

    val authApi: AuthApi = retrofit.create(AuthApi::class.java)
    val friendshipApi: FriendshipApi = retrofit.create(FriendshipApi::class.java)
    val locationApi: LocationApi = retrofit.create(LocationApi::class.java)

    companion object {
        /** Wraps suspend calls, converting HTTP errors to typed [org.alwaystogether.core.domain.repository.ApiException]. */
        suspend fun <T> run(apiCall: suspend () -> T): T = try {
            apiCall()
        } catch (e: HttpException) {
            throw e.toApiException()
        }
    }
}

private fun HttpException.toApiException(): org.alwaystogether.core.domain.repository.ApiException {
    val body = response()?.errorBody()?.string()
    val envelope = body?.let {
        runCatching {
            Json { ignoreUnknownKeys = true }.decodeFromString(ErrorEnvelope.serializer(), it)
        }.getOrNull()
    }
    return org.alwaystogether.core.domain.repository.ApiException(
        code = envelope?.error?.code ?: envelope?.code ?: "HTTP_${code()}",
        message = envelope?.error?.message ?: envelope?.message ?: "Request failed (HTTP ${code()})",
        httpStatus = code(),
    )
}
