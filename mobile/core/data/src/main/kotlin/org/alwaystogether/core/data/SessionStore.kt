package org.alwaystogether.core.data

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlinx.coroutines.flow.first
import org.alwaystogether.core.domain.model.AuthTokens
import org.alwaystogether.core.domain.repository.SessionRepository

private val Context.sessionDataStore by preferencesDataStore(name = "session")

/**
 * Encrypted session persistence.
 *
 * Security properties:
 *  - Tokens are AES-GCM encrypted with an Android Keystore key before touching
 *    DataStore; the key never leaves the platform (hardware-backed when
 *    available), so a rooted-device file dump yields ciphertext only.
 *  - The plaintext token exists in memory only ([memAccess]/[memCsrf]) and is
 *    handed to the network layer via providers; it is never logged.
 *
 * Lifecycle: [login] saves + caches; [restore] is called once at app start to
 * rehydrate the cache; [clear] wipes both memory and disk (logout).
 */
class SessionStore(private val context: Context) : SessionRepository {

    @Volatile private var memAccess: String? = null
    @Volatile private var memCsrf: String? = null
    @Volatile private var memUserId: Long? = null

    private val ds = context.sessionDataStore

    /** Called right after a successful login. */
    suspend fun login(tokens: AuthTokens) {
        memAccess = tokens.accessToken
        memCsrf = tokens.csrfToken
        memUserId = tokens.user.id
        ds.edit { prefs ->
            prefs[KEY_ACCESS] = encrypt(tokens.accessToken)
            prefs[KEY_CSRF] = encrypt(tokens.csrfToken)
            prefs[KEY_USER_ID] = tokens.user.id.toString()
        }
    }

    /** Called once at app start; returns true when a stored session was restored. */
    suspend fun restore(): Boolean {
        val prefs = ds.data.first()
        val encAccess = prefs[KEY_ACCESS] ?: return false
        val encCsrf = prefs[KEY_CSRF]
        return try {
            memAccess = decrypt(encAccess)
            memCsrf = encCsrf?.let { decrypt(it) }
            memUserId = prefs[KEY_USER_ID]?.toLongOrNull()
            true
        } catch (e: Exception) {
            // Keystore key gone (e.g. app data restored to new device):
            // session is unusable, wipe and force re-login.
            clear()
            false
        }
    }

    /** Wipes memory and disk (logout or unrecoverable decrypt failure). */
    override suspend fun clear() {
        memAccess = null
        memCsrf = null
        memUserId = null
        ds.edit { it.clear() }
    }

    override fun accessToken(): String? = memAccess

    override fun csrfToken(): String? = memCsrf

    fun userId(): Long? = memUserId

    // ---------------------------------------------------------------------
    // Keystore helpers
    // ---------------------------------------------------------------------

    private fun encrypt(plain: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKey())
        val iv = cipher.iv
        val ciphertext = cipher.doFinal(plain.toByteArray(Charsets.UTF_8))
        return Base64.encodeToString(iv + ciphertext, Base64.NO_WRAP)
    }

    private fun decrypt(encoded: String): String {
        val raw = Base64.decode(encoded, Base64.NO_WRAP)
        val iv = raw.copyOfRange(0, GCM_IV_LENGTH)
        val ciphertext = raw.copyOfRange(GCM_IV_LENGTH, raw.size)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKey(), GCMParameterSpec(GCM_TAG_BITS, iv))
        return String(cipher.doFinal(ciphertext), Charsets.UTF_8)
    }

    private fun getOrCreateKey(): SecretKey {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (keyStore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setRandomizedEncryptionRequired(true) // platform-provided IV
                .build(),
        )
        return generator.generateKey()
    }

    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val KEY_ALIAS = "always_together_session_key"
        private const val TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_IV_LENGTH = 12
        private const val GCM_TAG_BITS = 128
        private val KEY_ACCESS = stringPreferencesKey("access_token_enc")
        private val KEY_CSRF = stringPreferencesKey("csrf_token_enc")
        private val KEY_USER_ID = stringPreferencesKey("user_id")
    }
}
