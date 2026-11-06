import net.ltgt.gradle.errorprone.errorprone

plugins {
    `java-library`
    `maven-publish`
    signing
    alias(libs.plugins.errorprone)
}

group = "io.github.snovader.tafla"
version = "0.0.0"

// The project is core, after its directory, and the artifact is tafla, as README.md names it
base {
    archivesName = "tafla"
}

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
    }

    withSourcesJar()
    withJavadocJar()
}

// The license at the root of the repository, in every jar
tasks.withType<Jar>().configureEach {
    from(rootProject.layout.projectDirectory.file("../LICENSE")) {
        into("META-INF")
    }
}

tasks.withType<JavaCompile>().configureEach {
    options.release = 21
    // Error Prone reports most checks as warnings and has no flag of its own to fail on them.
    options.compilerArgs.add("-Werror")
    options.errorprone {
        error("NullAway")
        option("NullAway:OnlyNullMarked", true)
        option("NullAway:JSpecifyMode", true)
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
    systemProperty("tafla.conformance", conformance.asFile.absolutePath)
}

publishing {
    publications {
        register<MavenPublication>("maven") {
            artifactId = base.archivesName.get()
            from(components["java"])

            pom {
                name = "tafla"
                description = "Binary tables of scalars, strings, arrays and structs, read with bounds checks"
                url = "https://github.com/snovader/tafla"
                licenses {
                    license {
                        name = "MIT"
                        url = "https://opensource.org/license/mit"
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
                    connection = "scm:git:https://github.com/snovader/tafla.git"
                    developerConnection = "scm:git:ssh://git@github.com/snovader/tafla.git"
                    url = "https://github.com/snovader/tafla"
                }
                issueManagement {
                    system = "GitHub"
                    url = "https://github.com/snovader/tafla/issues"
                }
            }
        }
    }
}

// Signs the publication with an ASCII-armored key from the signingKey and signingPassword
// properties, such as ORG_GRADLE_PROJECT_signingKey in the environment. Without them, as in
// development and CI, the publication isn't signed.
val signingKey = providers.gradleProperty("signingKey")
if (signingKey.isPresent) {
    signing {
        useInMemoryPgpKeys(signingKey.get(), providers.gradleProperty("signingPassword").orNull)
        sign(publishing.publications["maven"])
    }
}
