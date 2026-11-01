#!/usr/bin/env python3
"""Trace uint32 table allocations in separate diagnostic JVMs, never timing runs.

Generate classes with run.py --include-fixed --validate-only first. --no-tlab --record enables
an allocation event for every allocation; use only for attribution, not speed.
"""

import argparse
import json
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
BUILD = HERE.parents[2] / "benchmarks/build/uint-setters"


def compiler_evidence(path, variant):
    root = ET.parse(path).getroot()
    compilations = []
    for task in root.iter("task"):
        if f"table_{variant}_32 " not in task.get("method", ""):
            continue
        classes = {node.get("id"): node.get("name") for node in task.iter("klass")}
        eliminated = [classes.get(node.get("type")) for node in task.iter("eliminate_allocation")]
        if not eliminated:
            continue
        methods = {}
        called = {}
        failures = []
        for node in task.iter():
            if node.tag == "method":
                methods[node.get("id")] = node.attrib
            elif node.tag == "call":
                called = methods.get(node.get("method"), {})
            elif node.tag == "inline_fail":
                failures.append({"holder": classes.get(called.get("holder")),
                    "method": called.get("name"), "reason": node.get("reason")})
        compilations.append({"compile_id": task.get("compile_id"), "method": task.get("method"),
            "eliminated": eliminated, "inlining_failures": failures})
    return compilations


def sample(worker, rows, operation, iterations):
    worker.stdin.write(f"{rows} {operation} {iterations}\n")
    worker.stdin.flush()
    line = worker.stdout.readline().strip()
    if not line.startswith("SAMPLE,"):
        raise RuntimeError(line)
    return int(line.split(",")[2]) / iterations


def probe(jdk, variant, name, flags, record):
    command = [str(jdk / "bin/java"), "-Xms2g", "-Xmx2g",
        "-Dexperiment.skipValidation=true", f"-Dexperiment.worker={variant}",
        "-XX:+UnlockDiagnosticVMOptions", "-XX:+LogCompilation",
        f"-XX:LogFile={BUILD / (name + '.xml')}", *flags,
        "-cp", str(BUILD / "classes"), "experiment.Harness", "false"]
    worker = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        text=True, bufsize=1)
    results = []
    try:
        while worker.stdout.readline().strip() != "READY":
            if worker.poll() is not None:
                raise RuntimeError(f"Worker exited: {worker.returncode}")
        # Preserve the earlier focused comparison's order and shared writer profiles.
        for rows in (100, 100_000):
            for width in (16, 32):
                for context in ("array", "table"):
                    operation = f"uint{width}_{context}_write"
                    iterations = 200_000 if rows == 100 else 1_000
                    for _ in range(24):
                        sample(worker, rows, operation, iterations)
                    allocated = [sample(worker, rows, operation, iterations) for _ in range(6)]
                    result = {"rows": rows, "operation": operation, "bytes_per_op": allocated}
                    results.append(result)
                    print(name, result, flush=True)
                    if record and rows == 100 and operation == "uint32_table_write":
                        jfc = BUILD / "allocation-events.jfc"
                        jfc.write_text('''<?xml version="1.0" encoding="UTF-8"?>
<configuration version="2.0" label="Allocations" description="Attribution only" provider="buffy experiment">
  <event name="jdk.ObjectAllocationOutsideTLAB"><setting name="enabled">true</setting><setting name="stackTrace">true</setting></event>
  <event name="jdk.ObjectAllocationInNewTLAB"><setting name="enabled">true</setting><setting name="stackTrace">true</setting></event>
</configuration>
''')
                        jfr = BUILD / f"{name}.jfr"
                        subprocess.run([str(jdk / "bin/jcmd"), str(worker.pid), "JFR.start",
                            "name=allocations", f"settings={jfc}"], check=True, stdout=subprocess.DEVNULL)
                        result["recorded_bytes_per_op"] = sample(worker, rows, operation, 10_000)
                        subprocess.run([str(jdk / "bin/jcmd"), str(worker.pid), "JFR.stop",
                            "name=allocations", f"filename={jfr}"], check=True, stdout=subprocess.DEVNULL)
        result = {"name": name, "variant": variant, "flags": flags, "samples": results}
        (BUILD / f"{name}.json").write_text(json.dumps(result, indent=2) + "\n")
        return result
    finally:
        worker.stdin.close()
        worker.wait(timeout=15)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java-home", type=Path, required=True)
    parser.add_argument("--variant", default="scalar")
    parser.add_argument("--label", default="table-allocation")
    parser.add_argument("--repeats", type=int, default=2)
    parser.add_argument("--no-tlab", action="store_true")
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--jvm-arg", action="append", default=[])
    args = parser.parse_args()
    flags = (["-XX:-UseTLAB"] if args.no_tlab else []) + args.jvm_arg
    for repeat in range(1, args.repeats + 1):
        name = f"{args.label}-{repeat}"
        result = probe(args.java_home, args.variant, name, flags, args.record)
        result["compilations"] = compiler_evidence(BUILD / f"{name}.xml", args.variant)
        (BUILD / f"{name}.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
