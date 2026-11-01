plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

rootProject.name = "tafla"
include("core", "benchmarks")

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}
