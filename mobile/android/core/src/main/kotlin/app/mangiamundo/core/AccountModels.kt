package app.mangiamundo.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** A code that signs another device in to the account, once, until `expiresAt` (Unix seconds). */
@Serializable
data class LinkCode(val code: String, @SerialName("expires_at") val expiresAt: Long)

/** A device signed in to the account. */
@Serializable
data class Device(
    val id: String,
    /** What the app called itself; null for devices from before devices had names. */
    val name: String? = null,
    @SerialName("created_at") val createdAt: Long,
    @SerialName("last_used_at") val lastUsedAt: Long,
    /** The device this app runs on. */
    val current: Boolean = false,
)

@Serializable
data class Account(
    @SerialName("created_at") val createdAt: Long,
    /** When the current recovery code was made; null when there is none yet. */
    @SerialName("recovery_code_created_at") val recoveryCodeCreatedAt: Long? = null,
    /** Most recently used first. */
    val devices: List<Device> = emptyList(),
    /** The public page of the user's progress, like `/p/abcd2345ef`; null when it's off. */
    @SerialName("public_profile") val publicProfile: String? = null,
    /** The ways to sign in, oldest first; empty for an account that lives on its devices. */
    val identities: List<Identity> = emptyList(),
)

/** A way to sign in: Sign in with Apple, Google or an email address. */
@Serializable
data class Identity(
    /** "apple", "google" or "email". */
    val provider: String,
    val email: String? = null,
)

/** A public link to share, with its card image and a sentence to go with it. */
@Serializable
data class ShareLink(
    val url: String,
    @SerialName("image_url") val imageUrl: String,
    val text: String,
)
