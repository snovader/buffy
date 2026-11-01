#!/usr/bin/env python3
"""Compare uint32 variants on JDK 25, or the production runtime on Java 21+.

No production sources are modified. Generated sources, classes and raw results
live in benchmarks/build/uint32. Requires the normal Gradle dependency cache.
"""

import argparse
import csv
import os
from pathlib import Path
import subprocess


HERE = Path(__file__).resolve().parent
JAVA = HERE.parents[2]
BUILD = JAVA / "benchmarks/build/uint32"
PACKAGE = "io.github.snovader.buffy"
VARIANTS = ("current", "repeat", "scalar", "simd")
BASELINE = "cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b"

SCALAR_READ = """    public long[] getUint32Array(int position) {
        var values = new long[arrayLength(position, Integer.BYTES)];
        var start = position + offsetSize;
        for (int i = 0; i < values.length; i++) {
            values[i] = Integer.toUnsignedLong(view.getInt(start + i * Integer.BYTES));
        }

        return values;
    }

"""

SCALAR_WRITE = """    public void setUint32Array(long[] values) {
        startArray(values.length, Integer.BYTES);
        var start = position;
        for (int i = 0; i < values.length; i++) {
            view.putInt(start + i * Integer.BYTES, (int) values[i]);
        }
        position = start + values.length * Integer.BYTES;
    }

"""

SIMD_FIELDS = """
    private static final VectorSpecies<Long> LONGS = LongVector.SPECIES_PREFERRED;
    // Equal lane counts: widening/narrowing changes the vector's bit width.
    private static final VectorSpecies<Integer> INTS = VectorSpecies.of(
        int.class, VectorShape.forBitSize(LONGS.vectorBitSize() / 2)
    );
    private static final VectorSpecies<Byte> BYTES = INTS.withLanes(byte.class);
"""

SIMD_READ = """    public long[] getUint32Array(int position) {
        var values = new long[arrayLength(position, Integer.BYTES)];
        var start = position + offsetSize;
        var bound = LONGS.loopBound(values.length);
        int i = 0;
        for (; i < bound; i += LONGS.length()) {
            // Reinterpretation assembles lanes in little-endian order.
            var packed = ByteVector.fromArray(BYTES, bytes, start + i * Integer.BYTES).reinterpretAsInts();
            var widened = (LongVector) packed.convertShape(VectorOperators.I2L, LONGS, 0);
            widened.and(0xffff_ffffL).intoArray(values, i);
        }
        for (; i < values.length; i++) {
            values[i] = Integer.toUnsignedLong(view.getInt(start + i * Integer.BYTES));
        }

        return values;
    }

"""

SIMD_WRITE = """    public void setUint32Array(long[] values) {
        startArray(values.length, Integer.BYTES);
        var start = position;
        var bound = LONGS.loopBound(values.length);
        int i = 0;
        for (; i < bound; i += LONGS.length()) {
            var wide = LongVector.fromArray(LONGS, values, i);
            var packed = (IntVector) wide.convertShape(VectorOperators.L2I, INTS, 0);
            packed.reinterpretAsBytes().intoArray(bytes, start + i * Integer.BYTES);
        }
        for (; i < values.length; i++) {
            view.putInt(start + i * Integer.BYTES, (int) values[i]);
        }
        position = start + values.length * Integer.BYTES;
    }

"""


def replace_method(source, start, end, replacement):
    assert source.count(start) == source.count(end) == 1
    first = source.index(start)
    last = source.index(end, first)
    return source[:first] + replacement + source[last:]


def generate_sources(baseline=BASELINE, *, production=False, writers_only=False):
    baseline_buffer = subprocess.check_output([
        "git", "show", f"{baseline}:java/core/src/main/java/io/github/snovader/buffy/ByteBuffer.java"
    ], cwd=JAVA, text=True)
    source_dir = BUILD / "src"
    source_dir.mkdir(parents=True, exist_ok=True)
    core = JAVA / "core/src/main/java/io/github/snovader/buffy"
    baseline_table = None
    if writers_only:
        baseline_buffer = (core / "ByteBuffer.java").read_text()
        baseline_table = subprocess.check_output([
            "git", "show", f"{baseline}:java/core/src/main/java/io/github/snovader/buffy/TableBuilder.java"
        ], cwd=JAVA, text=True)
    bench = JAVA / "benchmarks/src/main/java/io/github/snovader/buffy/benchmarks"
    sources = []
    variants = ("current", "production") if production else VARIANTS

    def save(path, content):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        sources.append(str(path))

    # A separate reference keeps fixture generation out of the timed variants' JIT profiles.
    for variant in (*variants, "reference"):
        target = source_dir / variant
        for original in core.glob("*.java"):
            if original.name == "package-info.java":
                continue
            source = (baseline_buffer if original.name == "ByteBuffer.java" and variant != "production"
                else original.read_text())
            if original.name == "TableBuilder.java" and variant != "production" and baseline_table is not None:
                source = baseline_table
            source = source.replace(
                f"package {PACKAGE};", f"package experiment.{variant};"
            )
            if original.name == "ByteBuffer.java" and variant in ("scalar", "simd"):
                source = replace_method(
                    source, "    public long[] getUint32Array(",
                    "    public long[] getInt64Array(",
                    SIMD_READ if variant == "simd" else SCALAR_READ,
                )
                source = replace_method(
                    source, "    public void setUint32Array(",
                    "    public void setInt64Array(",
                    SIMD_WRITE if variant == "simd" else SCALAR_WRITE,
                )
                if variant == "simd":
                    source = source.replace(
                        "import java.nio.*;",
                        "import java.nio.*;\nimport jdk.incubator.vector.*;",
                    ).replace("public final class ByteBuffer {", "public final class ByteBuffer {" + SIMD_FIELDS)
            save(target / original.name, source)

        # The actual document codec, including its builders and complete read path.
        source = (bench / "codecs/BuffyCodec.java").read_text()
        source = source.replace(
            f"package {PACKAGE}.benchmarks.codecs;",
            f"package experiment.{variant};\nimport {PACKAGE}.benchmarks.codecs.*;",
        ).replace(f"import {PACKAGE}.*;", "")
        save(target / "BuffyCodec.java", source)

    for name in ("DocumentInput", "ItemInput", "ListInput", "RecordInput", "SampleInput", "Workloads"):
        save(source_dir / "inputs" / f"{name}.java", (bench / f"{name}.java").read_text())
    for name in ("Codec", "DocumentWorkload", "Workload"):
        save(source_dir / "inputs" / f"{name}.java", (bench / "codecs" / f"{name}.java").read_text())

    template = (HERE / "Harness.java.template").read_text()
    vector_info = """        System.out.printf("%d-bit long vectors (%d lanes)%n",
            LongVector.SPECIES_PREFERRED.vectorBitSize(), LongVector.SPECIES_PREFERRED.length());"""
    template = template.replace("VECTOR_INFO", "" if production else vector_info)
    if production:
        template = template.replace("import jdk.incubator.vector.LongVector;\n", "")
    variant_block = template.split("// BEGIN VARIANT\n")[1].split("// END VARIANT")[0]
    methods = "\n".join(variant_block.replace("VARIANT", name) for name in variants)
    first = template.index("// BEGIN VARIANT\n")
    last = template.index("// END VARIANT") + len("// END VARIANT")
    template = template[:first] + methods + template[last:]
    registrations = "\n".join(f"        register_{name}(tasks, rows);" for name in variants)
    validations = "\n".join(f"            validate_{name}();" for name in variants)
    save(source_dir / "Harness.java", template.replace("REGISTER_VARIANTS", registrations).replace("VALIDATE_VARIANTS", validations))
    return sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", type=Path, default=os.environ.get("JAVA_HOME"))
    parser.add_argument("--baseline", default=BASELINE, help="Git revision containing the original ByteBuffer")
    parser.add_argument("--production", action="store_true",
        help="Compare the actual working runtime against the baseline and an A/A repeat; no Vector API")
    parser.add_argument("--writers-only", action="store_true",
        help="With --production, compare the baseline TableBuilder using working ByteBuffer setters in all variants")
    parser.add_argument("--rounds", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=16)
    parser.add_argument("--millis", type=int, default=100, help="Approximate duration per sample")
    parser.add_argument("--label", default="results")
    parser.add_argument("--rows", type=int, nargs="+", choices=(100, 100_000), default=(100, 100_000))
    parser.add_argument("--operations", nargs="+",
        choices=("array_write", "array_read", "document_write", "document_read"),
        default=("array_write", "array_read", "document_write", "document_read"))
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--validation-in-worker", action="store_true",
        help="Reproduce the historical setup with validation inside each timed JVM")
    parser.add_argument("--jvm-arg", action="append", default=[], help="Extra JVM option; use --jvm-arg=-Dname=value")
    args = parser.parse_args()
    if args.writers_only and not args.production:
        parser.error("--writers-only requires --production")
    if args.java_home is None:
        parser.error("Set JAVA_HOME or pass --java-home (JDK 25, or 21+ with --production)")
    jars = sorted((Path.home() / ".gradle/caches/modules-2/files-2.1/org.jspecify/jspecify").glob("*/*/jspecify-*.jar"))
    jars = [jar for jar in jars if not jar.name.endswith(("-sources.jar", "-javadoc.jar"))]
    if not jars:
        parser.error("Run the regular Gradle build first to download JSpecify")
    classes = BUILD / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    sources = generate_sources(args.baseline, production=args.production, writers_only=args.writers_only)
    modules = [] if args.production else ["--add-modules", "jdk.incubator.vector"]
    subprocess.run([
        str(args.java_home / "bin/javac"), "--release", "21" if args.production else "25", *modules,
        "-cp", str(jars[-1]), "-d", str(classes), *sources,
    ], check=True)
    command = [
        str(args.java_home / "bin/java"), *modules,
        "-Xms2g", "-Xmx2g", *args.jvm_arg, "-cp", str(classes), "experiment.Harness",
        str(args.validate_only).lower(),
    ]
    if args.validate_only:
        subprocess.run(command, check=True)
    else:
        if not args.validation_in_worker:
            validation = command.copy()
            validation[-1] = "true"
            subprocess.run(validation, check=True)
            command.insert(command.index("-cp"), "-Dexperiment.skipValidation=true")
        variants = ("current", "repeat", "production") if args.production else VARIANTS
        benchmark(command, args, variants=variants)


def benchmark(command, args, *, variants=VARIANTS, output_dir=BUILD):
    """Keep one warmed JVM per variant, interleaving measurements over a pipe.

    The two A/A workers run the exact same current classes. Separate JVMs keep
    type profiles and compilation history from leaking between variants.
    """
    workers = {}
    try:
        for variant in variants:
            implementation = "current" if variant == "repeat" else variant
            worker_command = command.copy()
            worker_command.insert(worker_command.index("-cp"), f"-Dexperiment.worker={implementation}")
            worker = subprocess.Popen(worker_command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
            workers[variant] = worker
            while True:
                line = worker.stdout.readline().strip()
                if line == "READY":
                    break
                if not line and worker.poll() is not None:
                    raise RuntimeError(f"{variant} worker failed: {worker.returncode}")
                print(f"{variant}: {line}", flush=True)

        def sample(variant, rows, operation, iterations):
            worker = workers[variant]
            worker.stdin.write(f"{rows} {operation} {iterations}\n")
            worker.stdin.flush()
            line = worker.stdout.readline().strip()
            if not line.startswith("SAMPLE,"):
                raise RuntimeError(f"{variant} worker returned {line!r}")
            return tuple(int(value) for value in line.split(",")[1:])

        def order(round_number):
            shift = (round_number // 2) % len(variants)
            sequence = variants[shift:] + variants[:shift]
            return sequence[::-1] if round_number % 2 else sequence

        target = args.millis * 1_000_000

        def count(previous, elapsed):
            return max(1, min(20_000_000, int(previous * target / max(1, elapsed))))

        output_path = output_dir / f"{args.label}.csv"
        with output_path.open("w") as output:
            writer = csv.writer(output)
            writer.writerow(("rows", "operation", "variant", "round", "iterations", "ns_per_op", "bytes_per_op", "gc_count", "gc_ms"))
            for rows in args.rows:
                for operation in args.operations:
                    counts = {}
                    for variant in variants:
                        iterations = 1
                        while True:
                            elapsed, *_ = sample(variant, rows, operation, iterations)
                            if elapsed >= 10_000_000:
                                break
                            iterations *= 2
                        counts[variant] = count(iterations, elapsed)
                    print(f"Warming {operation}, {rows} rows", flush=True)
                    for round_number in range(args.warmup):
                        for variant in order(round_number):
                            elapsed, *_ = sample(variant, rows, operation, counts[variant])
                            counts[variant] = count(counts[variant], elapsed)
                    print(f"Measuring {operation}, {rows} rows: {args.rounds} rounds", flush=True)
                    for round_number in range(args.rounds):
                        for variant in order(round_number):
                            iterations = counts[variant]
                            elapsed, allocated, gc_count, gc_ms = sample(variant, rows, operation, iterations)
                            writer.writerow((rows, operation, variant, round_number, iterations,
                                elapsed / iterations, allocated / iterations, gc_count, gc_ms))
                        output.flush()
        print(f"Saved {output_path}", flush=True)
    finally:
        for worker in workers.values():
            if worker.stdin:
                worker.stdin.close()
            try:
                worker.wait(timeout=5)
            except subprocess.TimeoutExpired:
                worker.kill()
                worker.wait()


if __name__ == "__main__":
    main()
