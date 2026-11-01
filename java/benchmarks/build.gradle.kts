import net.ltgt.gradle.errorprone.errorprone
import java.util.Locale

plugins {
    java
    alias(libs.plugins.errorprone)
    alias(libs.plugins.jmh)
    alias(libs.plugins.protobuf)
}

// SBE's code generator, which runs at build time and isn't on the benchmarks' classpath
val sbeTool = configurations.dependencyScope("sbeTool")
val sbeToolClasspath = configurations.resolvable("sbeToolClasspath") {
    extendsFrom(sbeTool.get())
}

dependencies {
    implementation(project(":core"))
    implementation(libs.agrona)
    implementation(libs.flatbuffers.java)
    implementation(libs.msgpack.core)
    implementation(libs.protobuf.java)

    "sbeTool"(libs.sbe.tool)

    testImplementation(libs.jqwik)
    testImplementation(libs.junit.jupiter)

    testRuntimeOnly(libs.junit.platform.launcher)

    errorprone(libs.errorprone.core)
    errorprone(libs.nullaway)
}

java {
    toolchain {
        languageVersion = JavaLanguageVersion.of(25)
        vendor = JvmVendorSpec.ORACLE
    }
}

protobuf {
    protoc {
        artifact = libs.protoc.get().toString()
    }
}

val generateSbe = tasks.register<JavaExec>("generateSbe") {
    description = "Generates SBE's encoders and decoders for the fixed-size record."

    val schema = layout.projectDirectory.file("src/main/sbe/sample.xml")
    val outputDir = layout.buildDirectory.dir("generated/sources/sbe/main/java")
    inputs.file(schema)
    outputs.dir(outputDir)

    classpath = files(sbeToolClasspath)
    mainClass = "uk.co.real_logic.sbe.SbeTool"
    args(schema.asFile.absolutePath)
    systemProperty("sbe.output.dir", outputDir.get().asFile.absolutePath)
    systemProperty("sbe.target.language", "Java")
    systemProperty("sbe.validation.stop.on.error", "true")

    doFirst {
        delete(outputDir)
    }
}

sourceSets.main {
    java.srcDir(generateSbe)
}

// The same checks as core's
tasks.withType<JavaCompile>().configureEach {
    options.release = 21
    // Error Prone reports most checks as warnings and has no flag of its own to fail on them.
    options.compilerArgs.add("-Werror")
    options.errorprone {
        error("NullAway")
        option("NullAway:OnlyNullMarked", true)
        option("NullAway:JSpecifyMode", true)
        // The generated packages hold protoc's and sbe-tool's output, and FlatBuffers classes in
        // flatc's shape. Their style is their generators', which an update can change, so they
        // aren't checked. They aren't null-marked, so NullAway skips them too.
        excludedPaths = ".*/io/github/snovader/tafla/benchmarks/generated/.*"
    }
}

tasks.named<JavaCompile>("compileJmhJava") {
    // JMH sets @Param fields and calls @Setup methods after constructing a benchmark's state
    options.errorprone {
        option("NullAway:ExcludedFieldAnnotations", "org.openjdk.jmh.annotations.Param")
        option("NullAway:CustomInitializerAnnotations", "org.openjdk.jmh.annotations.Setup")
    }
}

tasks.named<JavaCompile>("jmhCompileGeneratedClasses") {
    // JMH's generated harness, which isn't ours to change
    options.errorprone.enabled = false
}

// Agrona, whose buffers SBE's flyweights read and write, uses the JDK's internal Unsafe
val agronaJvmArgs = listOf("--add-exports=java.base/jdk.internal.misc=ALL-UNNAMED")

tasks.named<Test>("test") {
    javaLauncher = javaToolchains.launcherFor {
        languageVersion = providers.gradleProperty("testJavaVersion")
            .map { JavaLanguageVersion.of(it) }
            .orElse(JavaLanguageVersion.of(25))
    }

    useJUnitPlatform {
        includeEngines("jqwik")
    }

    jvmArgs(agronaJvmArgs)
}

// `build` compiles the benchmarks and checks JMH's annotations, without running them
tasks.named("check") {
    dependsOn(tasks.named("jmhCompileGeneratedClasses"))
}

tasks.register<JavaExec>("encodingReport") {
    description = "Prints each codec's encoded sizes, and what it does with a truncated document."
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass = "io.github.snovader.tafla.benchmarks.EncodingReport"
    jvmArgs(agronaJvmArgs)
}

jmh {
    jmhVersion = libs.versions.jmh

    // Every codec of every benchmark runs in 3 forked JVMs, each of 5 warm-up and 5 measured
    // iterations of 1 s. `--jmhArgs` overrides these, as in README.md's quick run.
    benchmarkMode = listOf("avgt")
    timeUnit = "us"
    fork = 3
    warmupIterations = 5
    warmup = "1s"
    iterations = 5
    timeOnIteration = "1s"

    // A fixed heap, so that resizing it doesn't add to some iterations. msgpack-core uses
    // sun.misc.Unsafe's memory access, which JDK 24 and later warn about.
    jvmArgs = listOf("-Xms2g", "-Xmx2g", "--sun-misc-unsafe-memory-access=allow") + agronaJvmArgs

    resultFormat = "JSON"
    resultsFile = layout.buildDirectory.file("results/jmh/results.json")
}

tasks.named("jmh") {
    val results = jmh.resultsFile
    doLast {
        printFastestIterations(results.get().asFile)
    }
}

/**
 * Prints a table of each benchmark's fastest iteration, and its ratio to tafla's, from JMH's JSON
 * results: a row per benchmark and parameters other than the codec, and a column per codec.
 */
fun printFastestIterations(results: File) {
    @Suppress("UNCHECKED_CAST")
    val runs = groovy.json.JsonSlurper().parse(results) as List<Map<String, Any>>

    val rows = linkedMapOf<String, MutableMap<String, Double>>()
    var unit = ""
    for (run in runs) {
        @Suppress("UNCHECKED_CAST")
        val params = run["params"] as Map<String, String>
        @Suppress("UNCHECKED_CAST")
        val metric = run["primaryMetric"] as Map<String, Any>
        @Suppress("UNCHECKED_CAST")
        val percentiles = metric["scorePercentiles"] as Map<String, Number>

        val benchmark = run["benchmark"].toString().substringAfterLast(".jmh.").replace("Benchmark.", ": ")
        val others = params.filterKeys { it != "codec" }.map { (name, value) -> "$name=$value" }
        val row = (listOf(benchmark) + others).joinToString(" ")

        rows.getOrPut(row) { linkedMapOf() }[params.getValue("codec")] = percentiles.getValue("0.0").toDouble()
        unit = metric["scoreUnit"].toString()
    }

    val codecs = rows.values.flatMap { it.keys }.distinct()
    val width = rows.keys.maxOf { it.length }
    fun format(time: Double): String = when {
        time < 0.1 -> "%.4f"
        time < 10 -> "%.3f"
        else -> "%.1f"
    }.format(Locale.ROOT, time)

    println("\nFastest iteration in $unit, and its ratio to ${codecs.first()}'s\n")
    println("| ${"Benchmark".padEnd(width)} | ${codecs.joinToString(" | ") { it.padStart(16) }} |")
    println("| ${"-".repeat(width)} | ${codecs.joinToString(" | ") { "-".repeat(15) + ":" }} |")
    for ((row, times) in rows) {
        val reference = times.getValue(codecs.first())
        val cells = codecs.map { codec ->
            val time = times[codec] ?: return@map ""
            val ratio = "%.2f".format(Locale.ROOT, time / reference)

            if (codec == codecs.first()) format(time) else "${format(time)} ($ratio×)"
        }
        println("| ${row.padEnd(width)} | ${cells.joinToString(" | ") { it.padStart(16) }} |")
    }
}
