plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "buffy"
include("core", "benchmarks")

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}
