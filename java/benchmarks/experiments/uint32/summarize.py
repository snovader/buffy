#!/usr/bin/env python3
"""Summarize one run at a time; do not pool independent JVMs' minima."""

import csv
from pathlib import Path
import statistics
import sys


def summarize(path):
    groups = {}
    with path.open() as source:
        for row in csv.DictReader(source):
            key = (int(row["rows"]), row["operation"], row["variant"])
            groups.setdefault(key, []).append(row)

    print(f"\n{path.name}: fastest / median µs per operation\n")
    variants = tuple(dict.fromkeys(key[2] for key in groups))
    labels = {"current": "Current", "repeat": "A/A repeat", "scalar": "Scalar", "simd": "SIMD", "cursor": "Cursor", "helper": "Helper", "production": "Production"}
    labels["fixed"] = "Fixed writers"
    header = "| Rows | Operation | " + " | ".join(labels[v] for v in variants) + " |"
    separator = "| ---: | --- | " + " | ".join("---:" for _ in variants) + " |"
    print(header)
    print(separator)
    for rows, operation in dict.fromkeys((key[0], key[1]) for key in groups):
        cells = []
        for variant in variants:
            values = [float(row["ns_per_op"]) / 1000 for row in groups[rows, operation, variant]]
            cells.append(f"{min(values):.4f} / {statistics.median(values):.4f}")
        print(f"| {rows} | {operation} | " + " | ".join(cells) + " |")

    print("\nMedian allocated bytes per operation\n")
    print(header)
    print(separator)
    for rows, operation in dict.fromkeys((key[0], key[1]) for key in groups):
        cells = []
        for variant in variants:
            samples = groups[rows, operation, variant]
            allocated = statistics.median(float(row["bytes_per_op"]) for row in samples)
            cells.append(f"{allocated:.0f}")
        print(f"| {rows} | {operation} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    for argument in sys.argv[1:]:
        summarize(Path(argument))
