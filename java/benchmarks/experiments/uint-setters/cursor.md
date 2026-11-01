# uint8 local-cursor experiment

> **Superseded by the [allocation investigation](allocation.md).** These runs
> performed correctness validation inside the timed JVMs, affecting JIT
> inlining and escape analysis. With validation in a separate JVM, the cursor
> allocates only the output array during direct writes and retains the table
> improvement. The cursor is now applied to production. The original results
> and interpretation below are retained as historical evidence.

A local cursor did **not** remove the direct-write regression in these runs.
It performed much like the indexed candidate: about 38% faster for a large
table, but about 14–15% slower for direct writes of 100,000 elements and
46–48% slower at 100 elements, relative to the faster unchanged worker in
each run. The production uint8 setter remains unchanged; the previously
applied uint16 and uint32 setters remain in place.

## Candidate

```java
public void setUint8Array(short[] values) {
    startArray(values.length, Byte.BYTES);

    var cursor = position;
    for (var value : values) {
        bytes[cursor++] = (byte) value;
    }

    position = cursor;
}
```

This keeps the original enhanced-for loop and increments a local variable,
then commits the position once. Unlike the indexed candidate, it does not
calculate each destination from `start + i`. Bounds checking, the length
prefix, casts and output bytes are unchanged.

## Method

Same [harness, inputs and correctness checks](README.md) as the preceding
unsigned-setter experiment, with a fourth JVM for the cursor candidate.
The `Scalar` column is the previous indexed candidate. Current and A/A repeat
run identical original code. All variants execute separate concrete timing
loops, interleaved in alternating/rotating order, with 16 warm-up rounds and
20 measurement rounds of about 100 ms each. Each JVM has a fixed 2 GiB heap.
Two complete runs used fresh sets of four JVMs.

Measured on 2026-11-01 on a 10-core Apple M2 Pro with 32 GiB RAM,
macOS 27.0.1, Oracle JDK 25.0.1+8-27, aarch64. Background applications were
active; initial one-minute load averages were 5.94 and 4.57. Timing and
allocation results are specific to these workloads and compilations.

## Timing

Cells show **fastest / median µs per operation** over 20 measured samples.
The table workload is an int64, the uint8 array and a float64 built through
`TableBuilder`; it is not the earlier full-document workload.

### cursor-run1

| Rows | Operation | Current | A/A repeat | Scalar | Cursor |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0218 / 0.0221 | 0.0217 / 0.0221 | 0.0316 / 0.0323 | 0.0321 / 0.0327 |
| 100 | uint8_table_write | 0.0820 / 0.0836 | 0.0671 / 0.0682 | 0.0737 / 0.0752 | 0.0737 / 0.0754 |
| 100000 | uint8_array_write | 17.2357 / 17.5638 | 17.2751 / 17.5625 | 19.7385 / 20.1221 | 19.5846 / 19.9351 |
| 100000 | uint8_table_write | 27.7887 / 28.2682 | 27.7389 / 28.0849 | 17.1189 / 17.5324 | 17.0324 / 17.3736 |

### cursor-run2

| Rows | Operation | Current | A/A repeat | Scalar | Cursor |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0215 / 0.0218 | 0.0400 / 0.0405 | 0.0313 / 0.0318 | 0.0314 / 0.0322 |
| 100 | uint8_table_write | 0.0661 / 0.0672 | 0.0671 / 0.0686 | 0.0726 / 0.0737 | 0.0730 / 0.0742 |
| 100000 | uint8_array_write | 17.3008 / 17.7304 | 27.6934 / 28.0496 | 19.7051 / 20.0763 | 19.8469 / 20.1802 |
| 100000 | uint8_table_write | 27.7214 / 28.1394 | 27.8636 / 28.2852 | 17.1177 / 17.5231 | 17.1576 / 17.5232 |

## Allocation and JIT variability

The indexed and cursor variants both allocated 224 B/op for direct writes
of 100 elements and 100,128 B/op at 100,000 in both runs. The fastest original
worker allocated 120 and 100,024 B/op: **104 fewer bytes per call**. This is a
fixed overhead, not an allocation per element. Cursor table writes allocated
504 and 100,400 B/op, matching the indexed candidate. Original table writes
varied by 88 B/op between compilations.

The second run's A/A worker was substantially slower than its identical
counterpart and also retained the extra 104 B/op. That makes it inappropriate
to claim a universal slowdown from the cursor syntax alone. The supported
conclusion is narrower: it did not deliver a reliable improvement over the
original's fast direct-write path in this experiment.

A separate `-XX:+LogCompilation` diagnostic showed both original and cursor
setters rejected for inlining in some compilations and successfully inlined
in others. The cursor is not inherently non-inlineable. This diagnostic does
not establish which compiled version ran in each benchmark sample, or the
precise compiler transformation responsible for the extra allocation. The
[extracted inlining decisions](results/cursor-inlining.json) record that
limited evidence; diagnostic timings are not included in the result tables.

## Reproduce

From `java/`, using JDK 25 to reproduce these timings:

```sh
uptime
python3 benchmarks/experiments/uint-setters/run.py \
    --include-cursor --operations uint8_array_write uint8_table_write \
    --validation-in-worker --label cursor-run1
python3 benchmarks/experiments/uint-setters/run.py \
    --include-cursor --operations uint8_array_write uint8_table_write \
    --validation-in-worker --label cursor-run2
python3 benchmarks/experiments/uint32/summarize.py \
    benchmarks/build/uint-setters/cursor-run1.csv \
    benchmarks/build/uint-setters/cursor-run2.csv
```

Set `JAVA_HOME` or pass `--java-home`. The source and class files are generated
under `benchmarks/build/uint-setters`. The original buffer is pinned to
`cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b`; the cursor is an optional generated
variant. At the time of these runs it had not yet been applied to production.

All candidate correctness checks passed before timing on JDK 25. The same
validation suite also passed on JDK 21: lengths 0–65, all offset widths,
unaligned starts, truncation/high bits, empty arrays, following fields, short
output buffers, and complete table bytes/readback at both measured sizes.

Raw samples: [run 1](results/cursor-run1.csv), [run 2](results/cursor-run2.csv).
[Environment and candidate source hashes](results/cursor-environment.json).
