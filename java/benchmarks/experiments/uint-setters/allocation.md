# uint8 allocation investigation

The extra **104 B per direct write** are the 48-byte buffy `ByteBuffer` and
its 56-byte `java.nio.HeapByteBuffer` view. They are created by
`ByteBuffer.allocate` and the constructor's `java.nio.ByteBuffer.wrap`.
HotSpot can eliminate both when they do not escape the compiled caller.
The returned byte array must remain allocated.

The earlier [cursor experiment](cursor.md) ran comprehensive correctness
validation inside every timed JVM. That changed compilation and inlining
decisions before warm-up. Moving validation into a separate JVM removes the
extra allocation in both repeated comparisons. The local cursor then keeps
the large-table improvement with approximately unchanged direct-write speed.
It is now applied to the production uint8 setter. The uint16 and uint32
setters retain their indexed loops.

## Evidence and cause

The diagnostic agent uses `Instrumentation.getObjectSize` to measure shallow
object sizes on this VM. For 100 elements, the 104-byte output array occupies
120 bytes including its header and alignment. Consequently:

- Output array alone: **120 B/op**.
- Output plus the NIO view: **176 B/op**.
- Output plus both wrappers: **224 B/op**.

Separate diagnostic JVMs execute 20 warm-up batches and 12 measured batches
of one million direct writes each. Every retained sample agrees with the
allocation shown below. These instrumented runs establish allocation and
compiler behavior; their timings are not used in performance comparisons.

| Cursor diagnostic | Allocated B/op |
| --- | ---: |
| Original validation setup, including expected failures | 224 |
| Successful validation only | 176 |
| Validation moved out of the timed JVM | 120 |
| Validation moved out; escape analysis disabled | 224 |
| Validation moved out; setter inlining disabled | 224 |

In the normal-entry C2 compilation of the direct loop, the original setup
rejects `setUint8Array` for inlining: **"already compiled into a big method"**.
With validation moved out, C2 inlines the setter, `startArray`,
`assertWritable`, `setUnsigned` and `assertFits`. Inlining allows escape
analysis to see through those calls and scalar-replace the wrappers.
Disabling escape analysis or setter inlining restores the exact 104 B.

Failure tests contribute to the compilation history, but are not the sole
cause: successful validation alone still retains the 56-byte view in this
diagnostic. The original setter can also retain both wrappers under this
setup. This explains the earlier A/A variability and corrects the earlier
inference that the cursor itself necessarily incurs 104 extra bytes.

A static helper accepting only `byte[]`, a cursor and the input array was
also tried. It still allocated 224 B/op under the original validation setup,
so it did not justify extra production code. The chosen implementation is
the simple local cursor already shown in the earlier report.

This is a JIT optimization, not a source-level allocation guarantee. The
runtime still constructs the wrappers; other callers, VM versions or profile
histories can retain them. Runtime validation, capacity checks, the length
prefix and truncation behavior are unchanged. Only the experiment's
correctness validation moved to another process, where it still runs before
timing and must pass.

## Corrected performance comparison

Two independent runs on a 10-core Apple M2 Pro, 32 GiB RAM, macOS 27.0.1,
Oracle JDK 25.0.1+8-27 aarch64. One-minute load averages before timing were
4.56 and 3.13; background applications were active. Compilation targets
Java 21, but these are Java 25 performance measurements.

Each run uses four separate JVMs: original, an identical A/A repeat, indexed
scalar, and local cursor. They run interleaved in alternating/rotating order,
with 16 warm-up rounds and 20 measured rounds of approximately 100 ms each.
Each variant has its own concrete timing loops and a fixed 2 GiB heap. Every
result is published to a volatile sink. No diagnostic agent or compiler flags
are present in these runs. Correctness validation runs first in a separate
JVM using the same generated classes and inputs.

The direct workload includes allocating and returning the output. The table
contains an int64, the uint8 array and a float64. It is a focused
`TableBuilder` workload, not the earlier full document benchmark.

The cursor reduces table time by approximately **14% at 100 elements** and
**38% at 100,000 elements**. Direct-write differences are small: approximately
1% at 100 elements, and 0.4–1.3% at 100,000, comparing fastest samples against
the faster unchanged worker in each run. This removes the earlier substantial
direct-write regression; it does not establish a direct-write speedup.

All variants allocate exactly the same bytes in every measured sample:

| Elements | Direct B/op | Table B/op |
| ---: | ---: | ---: |
| 100 | 120 | 440 |
| 100,000 | 100,024 | 100,336 |

Allocated bytes are measured on the calling thread, not peak RSS or compiler
and GC thread allocations. These results describe steady successful writes
with reused inputs; they are not evidence about exception-heavy application
profiles, other architectures, or all JIT compilations.

## Verification and reproduction

Production checks passed with the cursor applied:

- `./gradlew build` on Java 25, including core/conformance tests, Error Prone,
  NullAway, benchmark round trips and JMH compilation.
- `./gradlew test -PtestJavaVersion=21`.

Candidate validation covers lengths 0–65, all offset widths, unaligned starts,
low-bit truncation, following fields, short output buffers without partial
writes, and complete table bytes/readback at 100 and 100,000 elements.

From `java/`, with `JAVA_HOME` set to the desired JDK:

```sh
uptime
python3 benchmarks/experiments/uint-setters/run.py \
    --include-cursor --operations uint8_array_write uint8_table_write \
    --label isolated-run1
python3 benchmarks/experiments/uint-setters/run.py \
    --include-cursor --operations uint8_array_write uint8_table_write \
    --label isolated-run2
python3 benchmarks/experiments/uint32/summarize.py \
    benchmarks/build/uint-setters/isolated-run1.csv \
    benchmarks/build/uint-setters/isolated-run2.csv
```

To reproduce allocation controls and compiler logs separately:

```sh
python3 benchmarks/experiments/uint-setters/run.py --include-helper --validate-only
python3 benchmarks/experiments/uint-setters/diagnose.py
```

Use `--validation-in-worker` on timing runs only to reproduce the earlier
setup. Baseline `ByteBuffer` remains pinned to
`cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b`; generated candidates do not depend
on the production setter. Other runtime sources come from the working tree.

Evidence: [raw run 1](results/isolated-run1.csv),
[raw run 2](results/isolated-run2.csv),
[allocation controls](results/allocation-diagnostics.json),
[C2 inlining decisions](results/allocation-inlining.json), and
[environment/source hashes](results/allocation-environment.json).

## Detailed timings

Cells show **fastest / median µs per operation** over 20 measured samples.

### isolated-run1

| Elements | Operation | Original | A/A repeat | Indexed | Cursor |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0214 / 0.0218 | 0.0214 / 0.0217 | 0.0214 / 0.0217 | 0.0216 / 0.0220 |
| 100 | uint8_table_write | 0.0676 / 0.0684 | 0.0675 / 0.0684 | 0.0580 / 0.0589 | 0.0583 / 0.0590 |
| 100,000 | uint8_array_write | 17.2676 / 17.3928 | 17.2869 / 17.4087 | 17.2022 / 17.3527 | 17.3298 / 17.5826 |
| 100,000 | uint8_table_write | 27.9341 / 28.2157 | 27.8149 / 28.2160 | 17.2173 / 17.4398 | 17.1823 / 17.3721 |

### isolated-run2

| Elements | Operation | Original | A/A repeat | Indexed | Cursor |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0214 / 0.0217 | 0.0214 / 0.0218 | 0.0214 / 0.0216 | 0.0216 / 0.0219 |
| 100 | uint8_table_write | 0.0674 / 0.0683 | 0.0676 / 0.0685 | 0.0583 / 0.0590 | 0.0583 / 0.0590 |
| 100,000 | uint8_array_write | 17.1983 / 17.3608 | 17.0662 / 17.3606 | 17.2009 / 17.3722 | 17.2922 / 17.5109 |
| 100,000 | uint8_table_write | 27.8972 / 28.1559 | 28.0060 / 28.1881 | 17.1653 / 17.3737 | 17.1550 / 17.3238 |
