#!/usr/bin/env python3
"""Compare unsigned array setters against a saved Git revision, on Java 21+."""

import argparse
import importlib.util
import os
from pathlib import Path
import subprocess


HERE = Path(__file__).resolve().parent
JAVA = HERE.parents[2]
BUILD = JAVA / "benchmarks/build/uint-setters"
VARIANTS = ("current", "repeat", "scalar")
BASELINE = "cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b"

# Reuse the earlier experiment's worker scheduling and CSV format.
spec = importlib.util.spec_from_file_location("uint32_experiment", HERE.parent / "uint32/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

SETTERS = {
    8: """    public void setUint8Array(short[] values) {
        startArray(values.length, Byte.BYTES);

        var start = position;
        for (int i = 0; i < values.length; i++) {
            bytes[start + i] = (byte) values[i];
        }

        position = start + values.length;
    }

""",
    16: """    public void setUint16Array(int[] values) {
        startArray(values.length, Short.BYTES);

        var start = position;
        for (int i = 0; i < values.length; i++) {
            view.putShort(start + i * Short.BYTES, (short) values[i]);
        }

        position = start + values.length * Short.BYTES;
    }

""",
    32: """    public void setUint32Array(long[] values) {
        startArray(values.length, Integer.BYTES);

        var start = position;
        for (int i = 0; i < values.length; i++) {
            view.putInt(start + i * Integer.BYTES, (int) values[i]);
        }

        position = start + values.length * Integer.BYTES;
    }

""",
}

CURSOR_UINT8 = """    public void setUint8Array(short[] values) {
        startArray(values.length, Byte.BYTES);

        var cursor = position;
        for (var value : values) {
            bytes[cursor++] = (byte) value;
        }

        position = cursor;
    }

"""

HELPER_UINT8 = """    public void setUint8Array(short[] values) {
        startArray(values.length, Byte.BYTES);
        position = writeUint8Elements(bytes, position, values);
    }

    private static int writeUint8Elements(byte[] bytes, int cursor, short[] values) {
        for (var value : values) {
            bytes[cursor++] = (byte) value;
        }

        return cursor;
    }

"""


def fixed_writers(source):
    """Use the known field count directly, without an ArrayList wrapper."""
    replacements = {
        "private static final List<Consumer<ByteBuffer>> NO_WRITERS = List.of();":
            "private static final Consumer<ByteBuffer>[] NO_WRITERS = newWriters(0);",
        "private final int fields;": "private int fieldsSet;",
        "private List<Consumer<ByteBuffer>> writers;": "private Consumer<ByteBuffer>[] writers;",
        "        this.fields = fields;\n": "",
        "this.writers = new ArrayList<>(fields);": "this.writers = newWriters(fields);",
        "writers.size()": "fieldsSet",
        "fieldsSet != fields": "fieldsSet != positions.length",
        "fieldsSet, fields)": "fieldsSet, positions.length)",
        "table.fieldsSet != table.fields": "table.fieldsSet != table.positions.length",
        "                    table.fields\n": "                    table.positions.length\n",
        "index == fields": "index == positions.length",
        "fields + 1, fields)": "positions.length + 1, positions.length)",
        "writers.add(writer);": "writers[index] = writer;\n        fieldsSet = index + 1;",
        "i < fields;": "i < positions.length;",
        "writers.get(i).accept(buffer);": "writers[i].accept(buffer);",
    }
    for old, new in replacements.items():
        assert old in source, old
        source = source.replace(old, new)
    helper = """    @SuppressWarnings("unchecked")
    private static Consumer<ByteBuffer>[] newWriters(int fields) {
        return (Consumer<ByteBuffer>[]) new Consumer<?>[fields];
    }

"""
    marker = "    /** Pads the array so that its elements, after the length, are aligned */"
    assert marker in source
    return source.replace(marker, helper + marker)


def generate_sources(baseline, *, include_cursor=False, include_helper=False, include_fixed=False):
    baseline_source = subprocess.check_output([
        "git", "show", f"{baseline}:java/core/src/main/java/io/github/snovader/buffy/ByteBuffer.java"
    ], cwd=JAVA, text=True)
    baseline_table = None
    if include_fixed:
        baseline_table = subprocess.check_output([
            "git", "show", f"{baseline}:java/core/src/main/java/io/github/snovader/buffy/TableBuilder.java"
        ], cwd=JAVA, text=True)
    source_dir = BUILD / "src"
    sources = []

    def save(path, source):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source)
        sources.append(str(path))

    implementations = ("current", "scalar", "cursor") if include_cursor or include_helper else ("current", "scalar")
    if include_helper:
        implementations += ("helper",)
    if include_fixed:
        implementations += ("fixed",)
    for variant in (*implementations, "reference"):
        for path in (JAVA / "core/src/main/java/io/github/snovader/buffy").glob("*.java"):
            if path.name == "package-info.java":
                continue
            source = baseline_source if path.name == "ByteBuffer.java" else path.read_text()
            if path.name == "TableBuilder.java" and baseline_table is not None:
                source = baseline_table
            if variant in ("scalar", "cursor", "helper", "fixed") and path.name == "ByteBuffer.java":
                for width, next_width in ((8, 16), (16, 32), (32, 64)):
                    setter = CURSOR_UINT8 if variant == "cursor" and width == 8 else SETTERS[width]
                    if variant == "helper" and width == 8:
                        setter = HELPER_UINT8
                    source = harness.replace_method(source,
                        f"    public void setUint{width}Array(",
                        f"    public void setInt{next_width}Array(", setter)
            if variant == "fixed" and path.name == "TableBuilder.java":
                source = fixed_writers(source)
            source = source.replace("package io.github.snovader.buffy;", f"package experiment.{variant};")
            save(source_dir / variant / path.name, source)

    template = (HERE / "Harness.java.template").read_text()
    start = template.index("// BEGIN KERNEL\n")
    end = template.index("// END KERNEL")
    kernel = template[start + len("// BEGIN KERNEL\n"):end]
    methods = []
    registrations = []
    validations = []
    for variant in implementations:
        for width, java_type in ((8, "short"), (16, "int"), (32, "long")):
            methods.append(kernel.replace("VARIANT", variant).replace("WIDTH", str(width)).replace("TYPE", java_type))
            registrations.append(f"            register_{variant}_{width}(tasks, rows);")
            validations.append(f"        validate_{variant}_{width}();")
    template = template[:start] + "\n".join(methods) + template[end + len("// END KERNEL"):]
    template = template.replace("REGISTER", "\n".join(registrations)).replace("VALIDATE", "\n".join(validations))
    save(source_dir / "Harness.java", template)
    return sources


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", type=Path, default=os.environ.get("JAVA_HOME"))
    parser.add_argument("--baseline", default=BASELINE)
    parser.add_argument("--include-cursor", action="store_true", help="Also compare a local cursor in the uint8 setter")
    parser.add_argument("--include-helper", action="store_true", help="Compare original, A/A, cursor and a static payload helper")
    parser.add_argument("--include-fixed", action="store_true", help="Also compare indexed setters with a fixed writer array")
    parser.add_argument("--jvm-arg", action="append", default=[], help="Diagnostic JVM option; use --jvm-arg=-Dname=value")
    parser.add_argument("--rounds", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=16)
    parser.add_argument("--millis", type=int, default=100)
    parser.add_argument("--label", default="results")
    parser.add_argument("--rows", type=int, nargs="+", choices=(100, 100_000), default=(100, 100_000))
    operations = tuple(f"uint{width}_{context}_write" for width in (8, 16, 32) for context in ("array", "table"))
    parser.add_argument("--operations", nargs="+", choices=operations, default=operations)
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--validation-in-worker", action="store_true",
        help="Reproduce the earlier diagnostic setup: validate in each timed JVM")
    args = parser.parse_args()
    if args.java_home is None:
        parser.error("Set JAVA_HOME or pass --java-home")
    classes = BUILD / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    sources = generate_sources(args.baseline, include_cursor=args.include_cursor,
        include_helper=args.include_helper, include_fixed=args.include_fixed)
    jars = sorted((Path.home() / ".gradle/caches/modules-2/files-2.1/org.jspecify/jspecify").glob("*/*/jspecify-*.jar"))
    jars = [jar for jar in jars if not jar.name.endswith(("-sources.jar", "-javadoc.jar"))]
    if not jars:
        parser.error("Run the regular Gradle build first to download JSpecify")
    subprocess.run([str(args.java_home / "bin/javac"), "--release", "21",
        "-cp", str(jars[-1]), "-d", str(classes), *sources], check=True)
    command = [str(args.java_home / "bin/java"), "-Xms2g", "-Xmx2g", *args.jvm_arg,
        "-cp", str(classes), "experiment.Harness", str(args.validate_only).lower()]
    if args.validate_only:
        subprocess.run(command, check=True)
    else:
        if not args.validation_in_worker:
            validation = command.copy()
            validation[-1] = "true"
            subprocess.run(validation, check=True)
            command.insert(command.index("-cp"), "-Dexperiment.skipValidation=true")
        variants = (*VARIANTS, "cursor") if args.include_cursor else VARIANTS
        if args.include_helper:
            variants = ("current", "repeat", "cursor", "helper")
        if args.include_fixed:
            variants += ("fixed",)
        harness.benchmark(command, args, variants=variants, output_dir=BUILD)


if __name__ == "__main__":
    main()
