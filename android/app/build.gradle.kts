import org.gradle.api.tasks.Copy
import org.gradle.api.tasks.Exec

plugins {
    id("com.android.application")
}

val cargoToml = rootProject.file("../Cargo.toml")
val appVersion = Regex("(?m)^version\\s*=\\s*\"([^\"]+)\"")
    .find(cargoToml.readText())
    ?.groupValues
    ?.get(1)
    ?: "0.0.0"
val versionParts = appVersion.split("-").first().split(".").map { it.toIntOrNull() ?: 0 }
val appVersionCode = versionParts.getOrElse(0) { 0 } * 10_000 +
    versionParts.getOrElse(1) { 0 } * 100 +
    versionParts.getOrElse(2) { 0 }

// One APK per ABI, or -Pabi=universal for both ARM ABIs in one installable
// file. Universal is what the release workflow ships so users cannot download
// the wrong asset.
val requestedAbi = providers.gradleProperty("abi").orElse("arm64-v8a").get()
val abis = if (requestedAbi == "universal") {
    listOf("arm64-v8a", "armeabi-v7a")
} else {
    listOf(requestedAbi)
}
fun rustTargetFor(abi: String): String = when (abi) {
    "arm64-v8a" -> "aarch64-linux-android"
    "armeabi-v7a" -> "armv7-linux-androideabi"
    "x86_64" -> "x86_64-linux-android"
    else -> throw GradleException("Unsupported ABI '$abi' (use arm64-v8a, armeabi-v7a, x86_64 or universal)")
}
val rustTargets = abis.map { rustTargetFor(it) }

val releaseRequested = gradle.startParameter.taskNames.any { it.contains("Release", ignoreCase = true) }
val rustProfile = providers.gradleProperty("rustProfile")
    .orElse(if (releaseRequested) "release" else "debug")
    .get()
val cargoTargetDir = System.getenv("CARGO_TARGET_DIR")
    ?.takeIf { it.isNotBlank() }
    ?: rootProject.file("../target").absolutePath

android {
    namespace = "com.pinpoint.desktop"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.pinpoint.desktop"
        minSdk = 26
        targetSdk = 35
        versionCode = appVersionCode
        versionName = appVersion
        ndk {
            abiFilters += abis
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets["main"].jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs"))
    sourceSets["main"].res.srcDir(rootProject.file("res"))

    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }
}

val cargoApkBuilds = rustTargets.mapIndexed { index, target ->
    tasks.register<Exec>("cargoApkBuild$index") {
        group = "rust"
        description = "Build libpinpoint_desktop.so for $target with cargo-apk"
        workingDir = rootProject.file("..")
        val command = mutableListOf("apk", "build", "--lib", "--target", target)
        if (rustProfile == "release") {
            command += "--release"
        }
        commandLine("cargo", *command.toTypedArray())
        environment("CARGO_TARGET_DIR", cargoTargetDir)
        System.getenv("ANDROID_HOME")?.let { environment("ANDROID_HOME", it) }
        System.getenv("ANDROID_NDK_HOME")?.let { environment("ANDROID_NDK_HOME", it) }
    }
}

val copyRustJniLib = tasks.register<Copy>("copyRustJniLib") {
    group = "rust"
    description = "Copy the Rust shared libraries into the APK's jniLibs"
    dependsOn(cargoApkBuilds)
    rustTargets.forEachIndexed { index, target ->
        from("$cargoTargetDir/$target/$rustProfile/libpinpoint_desktop.so") {
            into(abis[index])
        }
    }
    into(layout.buildDirectory.dir("rustJniLibs"))
}

tasks.named("preBuild").configure {
    dependsOn(copyRustJniLib)
}
