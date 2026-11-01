import net.ltgt.gradle.errorprone.errorprone

plugins {
    `java-library`
    alias(libs.plugins.errorprone)
}

group = "io.github.snovader.buffy"

dependencies {
    api(libs.jspecify)

    testImplementation(libs.jakarta.json.api)
    testImplementation(libs.jqwik)
    testImplementation(libs.junit.jupiter)

    testRuntimeOnly(libs.junit.platform.launcher)
    testRuntimeOnly(libs.parsson)

    errorprone(libs.errorprone.core)
    errorprone(libs.nullaway)
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
        vendor = JvmVendorSpec.ORACLE
    }
}

tasks.withType<JavaCompile> {
    options.release = 21
    // Error Prone reports most checks as warnings and has no flag of its own to fail on them.
    options.compilerArgs.add("-Werror")
    options.errorprone {
        error("NullAway")
        option("NullAway:OnlyNullMarked", true)
        option("NullAway:JSpecifyMode", true)
    }
}

tasks.javadoc {
    // Every check of doclint, including the one for public members, parameters and return values
    // without a comment, and failing on any warning
    (options as StandardJavadocDocletOptions).apply {
        addBooleanOption("Xdoclint:all", true)
        addBooleanOption("Werror", true)
    }
}

tasks.named<Test>("test") {
    javaLauncher = javaToolchains.launcherFor {
        languageVersion = providers.gradleProperty("testJavaVersion")
            .map { JavaLanguageVersion.of(it) }
            .orElse(JavaLanguageVersion.of(25))
    }

    useJUnitPlatform {
        includeEngines("jqwik")
    }

    // The conformance suite at the root of the repository, with the cases of the generated code in
    // its generated/, see conformance/README.md
    val conformance = rootProject.layout.projectDirectory.dir("../conformance")
    inputs.dir(conformance)
    systemProperty("buffy.conformance", conformance.asFile.absolutePath)
}
