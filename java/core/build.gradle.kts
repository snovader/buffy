import com.vanniktech.maven.publish.JavadocJar
import com.vanniktech.maven.publish.SourcesJar
import net.ltgt.gradle.errorprone.errorprone

plugins {
    `java-library`
    alias(libs.plugins.errorprone)
    alias(libs.plugins.maven.publish)
}

group = "io.github.snovader.buffy"
// Every package of buffy has the same version, which scripts/set-version sets
version = "0.0.0"

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
    title = "buffy-core $version API"

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

// The license in every published jar: the classes', the sources' and the documentation's, whose
// task is a JVM Jar rather than a bundling one
tasks.withType<org.gradle.jvm.tasks.Jar>().configureEach {
    from(rootProject.layout.projectDirectory.file("../LICENSE")) {
        into("META-INF")
    }
}

// The key that signs a release, which RELEASING.md describes. Without it, publications aren't
// signed, so that publishToMavenLocal works anywhere. A workflow passes an unset secret as empty.
val signingKey = providers.gradleProperty("signingInMemoryKey").filter { it.isNotBlank() }

mavenPublishing {
    configureBasedOnAppliedPlugins(javadocJar = JavadocJar.Javadoc(), sourcesJar = SourcesJar.Sources())
    publishToMavenCentral()
    if (signingKey.isPresent) {
        signAllPublications()
    }

    coordinates(artifactId = "buffy-core")
    pom {
        name = "buffy-core"
        description = "Binary tables of scalars, strings and arrays, read with bounds checks"
        url = "https://github.com/snovader/buffy"
        inceptionYear = "2026"

        licenses {
            license {
                name = "MIT"
                url = "https://github.com/snovader/buffy/blob/main/LICENSE"
                distribution = "repo"
            }
        }

        developers {
            developer {
                id = "snovader"
                name = "snovader"
                url = "https://github.com/snovader"
            }
        }

        scm {
            url = "https://github.com/snovader/buffy"
            connection = "scm:git:https://github.com/snovader/buffy.git"
            developerConnection = "scm:git:ssh://git@github.com/snovader/buffy.git"
        }

        issueManagement {
            system = "GitHub"
            url = "https://github.com/snovader/buffy/issues"
        }
    }
}

// Maven Central rejects unsigned artifacts after they're uploaded, so fail before uploading them
tasks.withType<PublishToMavenRepository>().configureEach {
    val signed = signingKey.isPresent
    doFirst {
        if (!signed && repository.name == "mavenCentral") {
            throw GradleException("Publishing to Maven Central needs a signing key, see RELEASING.md")
        }
    }
}
