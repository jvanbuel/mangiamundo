plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    // Screenshot tests that render the Compose UI on the JVM, without a device.
    id("app.cash.paparazzi")
}

/** The commit shown in the app, to tell old builds apart. */
fun gitCommit(): String = providers.exec {
    commandLine("git", "describe", "--always", "--dirty")
    isIgnoreExitValue = true
}.standardOutput.asText.get().trim().ifEmpty { "unknown" }

/** A setting of the release build: the environment variable, or else the Gradle property. */
fun release(variable: String, property: String): String? =
    providers.environmentVariable(variable).orElse(providers.gradleProperty(property)).orNull?.ifBlank { null }

// The upload key that signs a release, which is not in the repository (docs/release.md).
val keystoreFile = release("MANGIAMUNDO_KEYSTORE_FILE", "mangiamundo.keystoreFile")

android {
    namespace = "app.mangiamundo"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.mangiamundo"
        minSdk = 26
        targetSdk = 36
        // Play takes each number once, so a release from CI passes its run number.
        versionCode = release("MANGIAMUNDO_VERSION_CODE", "mangiamundo.versionCode")?.toInt() ?: 1
        versionName = "0.1.0"
        buildConfigField("String", "GIT_COMMIT", "\"${gitCommit()}\"")
        buildConfigField("String", "API_BASE", "\"${providers.gradleProperty("mangiamundo.apiBase").get()}\"")
        // Google sign-in asks for an ID token for the web OAuth client, which the server accepts.
        buildConfigField("String", "GOOGLE_WEB_CLIENT_ID", "\"1020416480252-1hdk15p45gq4le5an455c87ouup4fekm.apps.googleusercontent.com\"")
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    // The fonts are shared with the iOS app and the backend, in a res/ layout so that
    // they are ordinary font resources here (R.font.nunito_*).
    sourceSets["main"].res.srcDir("../../../shared/fonts/res")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    signingConfigs {
        // Without a keystore the release build still works; its bundle is just not signed.
        if (keystoreFile != null) {
            create("release") {
                storeFile = rootProject.file(keystoreFile)
                storePassword = release("MANGIAMUNDO_KEYSTORE_PASSWORD", "mangiamundo.keystorePassword")
                keyAlias = release("MANGIAMUNDO_KEY_ALIAS", "mangiamundo.keyAlias")
                keyPassword = release("MANGIAMUNDO_KEY_PASSWORD", "mangiamundo.keyPassword")
            }
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.findByName("release")
            // Not shrunk: no test runs a shrunk build, so nothing would show R8 breaking
            // kotlinx.serialization or MapLibre before it is on someone's phone.
            isMinifyEnabled = false
            // Release builds run on real phones, so they use the deployed backend.
            buildConfigField("String", "API_BASE", "\"https://mangiamundo.com\"")
        }
    }
}

/**
 * Copies the country outlines, shared with the iOS app and the backend, into a folder of
 * their own, with the licence of the fonts the app ships.
 */
abstract class CopyWorld : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val world: RegularFileProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.NONE)
    abstract val fontLicence: RegularFileProperty

    @get:OutputDirectory
    abstract val assets: DirectoryProperty

    @TaskAction
    fun copy() {
        world.get().asFile.copyTo(assets.file("world.json").get().asFile, overwrite = true)
        fontLicence.get().asFile.copyTo(assets.file("Nunito-OFL.txt").get().asFile, overwrite = true)
    }
}

// shared/ also holds the fonts, so it cannot be an assets folder as a whole.
val copyWorld = tasks.register<CopyWorld>("copyWorld") {
    world.set(layout.projectDirectory.file("../../../shared/world.json"))
    fontLicence.set(layout.projectDirectory.file("../../../shared/fonts/Nunito-OFL.txt"))
}
androidComponents.onVariants { variant ->
    variant.sources.assets?.addGeneratedSourceDirectory(copyWorld, CopyWorld::assets)
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation(project(":core"))
    implementation(platform("androidx.compose:compose-bom:2025.06.00"))
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.1")
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.core:core-splashscreen:1.0.1")
    implementation("androidx.work:work-runtime-ktx:2.10.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
    // Street map for placing a new restaurant; open source, no API key.
    implementation("org.maplibre.gl:android-sdk:11.11.0")
    // Sign in with Google through Android's Credential Manager.
    implementation("androidx.credentials:credentials:1.5.0")
    implementation("androidx.credentials:credentials-play-services-auth:1.5.0")
    implementation("com.google.android.libraries.identity.googleid:googleid:1.1.1")
    testImplementation("junit:junit:4.13.2")
}
