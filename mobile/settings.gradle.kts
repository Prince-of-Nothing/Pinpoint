pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "always-together-mobile"

// Clean Architecture module layout (board T04, extended with :core:domain
// for a pure-Kotlin, JVM-testable application layer).
include(":app")
include(":core:domain")
include(":core:network")
include(":core:data")
include(":core:map")
include(":feature:auth")
include(":feature:map")
include(":feature:friends")
include(":feature:sharing")
