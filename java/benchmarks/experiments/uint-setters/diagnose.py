#!/usr/bin/env python3
"""Identify buffer sizes and test escape analysis in separate diagnostic JVMs.

First generate classes with run.py --include-helper --validate-only. Diagnostic
timings are not performance results; the output records allocated bytes only.
"""

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import xml.etree.ElementTree as ET


HERE = Path(__file__).resolve().parent
BUILD = HERE.parents[2] / "benchmarks/build/uint-setters"


def build_agent(jdk):
    directory = BUILD / "allocation-probe"
    directory.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(jdk / "bin/javac"), "--release", "21", "-d", str(directory),
        str(HERE / "AllocationProbe.java")], check=True)
    manifest = directory / "MANIFEST.MF"
    manifest.write_text("Premain-Class: experiment.diagnostics.AllocationProbe\n\n")
    jar = directory / "probe.jar"
    subprocess.run([str(jdk / "bin/jar"), "--create", "--file", str(jar),
        "--manifest", str(manifest), "-C", str(directory), "experiment"], check=True)
    return jar


def probe(jdk, agent, variant, name, flags):
    log = BUILD / f"allocation-{name}.xml"
    command = [str(jdk / "bin/java"), "-Xms2g", "-Xmx2g",
        f"-javaagent:{agent}={variant}", "-XX:+UnlockDiagnosticVMOptions",
        "-XX:+LogCompilation", f"-XX:LogFile={log}", *flags,
        f"-Dexperiment.worker={variant}", "-cp", str(BUILD / "classes"),
        "experiment.Harness", "false"]
    worker = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        text=True, bufsize=1)
    startup = []
    try:
        while True:
            line = worker.stdout.readline().strip()
            if line == "READY":
                break
            if not line and worker.poll() is not None:
                raise RuntimeError(f"Worker failed: {worker.returncode}")
            startup.append(line)

        allocated = []
        for iteration in range(32):
            worker.stdin.write("100 uint8_array_write 1000000\n")
            worker.stdin.flush()
            line = worker.stdout.readline().strip()
            if not line.startswith("SAMPLE,"):
                raise RuntimeError(line)
            fields = [int(value) for value in line.split(",")[1:]]
            if iteration >= 20:
                allocated.append(fields[1] / 1_000_000)
        return {"name": name, "variant": variant, "flags": flags,
            "object_sizes": [line for line in startup if line.startswith("SIZE ")],
            "allocated_bytes_per_op": allocated,
            "median_allocated_bytes_per_op": statistics.median(allocated)}
    finally:
        worker.stdin.close()
        worker.wait(timeout=15)


def compiler_decisions(name, variant):
    """Keep C2's normal-entry direct-loop inlining decisions, including recompiles."""
    root = ET.parse(BUILD / f"allocation-{name}.xml").getroot()
    compiled = {node.get("compile_id"): node.attrib for node in root.iter("nmethod")}
    decisions = []
    for task in root.iter("task"):
        compilation = task.get("compile_id")
        if (f"arrayWrite_{variant}_8 " not in task.get("method", "")
                or compiled.get(compilation, {}).get("compiler") != "c2"
                or task.get("compile_kind") == "osr"):
            continue
        classes = {}
        methods = {}
        called = {}
        for node in task.iter():
            if node.tag == "klass":
                classes[node.get("id")] = node.get("name")
            elif node.tag == "method":
                methods[node.get("id")] = node.attrib
            elif node.tag == "call":
                called = methods.get(node.get("method"), {})
            elif node.tag in ("inline_success", "inline_fail"):
                if classes.get(called.get("holder")) == f"experiment.{variant}.ByteBuffer":
                    decisions.append({"compilation": compilation, "method": called.get("name"),
                        "decision": node.tag, "reason": node.get("reason")})
    return {"name": name, "variant": variant, "decisions": decisions}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", type=Path, default=os.environ.get("JAVA_HOME"))
    args = parser.parse_args()
    if args.java_home is None:
        parser.error("Set JAVA_HOME or --java-home")
    agent = build_agent(args.java_home)
    cases = [
        ("current", "original", []),
        ("cursor", "cursor", []),
        ("helper", "helper", []),
        ("cursor", "cursor-no-validation", ["-Dexperiment.skipValidation=true"]),
        ("cursor", "cursor-success-validation", ["-Dexperiment.skipFailureChecks=true"]),
        ("current", "original-no-validation", ["-Dexperiment.skipValidation=true"]),
        ("cursor", "cursor-clean-no-ea", ["-Dexperiment.skipValidation=true", "-XX:-DoEscapeAnalysis"]),
        ("cursor", "cursor-clean-no-inline", ["-Dexperiment.skipValidation=true", "-XX:CompileCommand=dontinline,experiment.cursor.ByteBuffer::setUint8Array"]),
        ("helper", "helper-no-ea", ["-XX:-DoEscapeAnalysis"]),
        ("cursor", "cursor-no-inline", ["-XX:CompileCommand=dontinline,experiment.cursor.ByteBuffer::setUint8Array"]),
        ("helper", "helper-payload-no-inline", ["-XX:CompileCommand=dontinline,experiment.helper.ByteBuffer::writeUint8Elements"]),
    ]
    results = []
    inlining = []
    for variant, name, flags in cases:
        result = probe(args.java_home, agent, variant, name, flags)
        results.append(result)
        print(json.dumps(result), flush=True)
        (BUILD / "allocation-diagnostics.json").write_text(json.dumps(results, indent=2) + "\n")
        inlining.append(compiler_decisions(name, variant))
        (BUILD / "allocation-inlining.json").write_text(json.dumps(inlining, indent=2) + "\n")


if __name__ == "__main__":
    main()
