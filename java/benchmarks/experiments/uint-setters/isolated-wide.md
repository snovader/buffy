# uint16/uint32 setters and full documents, with isolated validation

Follow-up to the [uint8 allocation investigation](allocation.md). Correctness
checks run in a separate JVM before timing. Timed workers use the same
generated classes, but warm up only on the measured workloads. This avoids
priming their JIT profiles with the validation suite's boundary and failure
cases.

## Results and decision

**Keep the indexed uint16/uint32 setters.** Both retain substantial table-write
gains, and the actual production runtime serializes the full document faster
in both independent runs. The earlier large direct-write gains do not survive
the corrected setup; the benefits depend on calling context.

| Workload | 100 elements/rows | 100,000 elements/rows |
| --- | ---: | ---: |
| uint16 direct write | 3–4% less time | 2–4% less time |
| uint16 table write | 11% less time | 33–34% less time |
| uint32 direct write | −1% to +2% time; no consistent gain | ~2% more time in fastest samples |
| uint32 table write | 15–20% less time | 38–40% less time |
| Full document write | ~7% less time | 29–32% less time |

At 100,000 rows, full-document writes fall from **56.6–59.4 µs to
40.1–40.4 µs**, comparing the faster unchanged worker in each run with
production. At 100 rows, they fall from **0.204 µs to 0.190 µs**. Document
read timings differ by less than 1% from the faster A/A baseline in each run;
there is no reader change or consistent read speedup. The second run's two
unchanged large-document writers differ by about 5% in their fastest samples,
so the table uses the faster one when quoting the gain.

Large direct uint32 writes have similar medians: 18.22/18.28 µs for the faster
baseline by median versus 18.41/18.28 µs for the candidate. Their fastest
samples show a repeatable but small 1.7–2.0% slowdown. Direct uint16 improves
only modestly. Neither result supports the previously reported 24–31% uint16
direct-write reduction or a broad direct-write advantage for uint32.

The corrected full-document gain is also smaller than the historical
11–12%/42% result. Use the corrected ~7%/29–32% figures for this machine and
workload. No production source changes were needed for this rerun.

### Allocation

Direct writes allocate **only the output byte array** for every variant in
both runs: 224/200,024 B for uint16 and 424/400,024 B for uint32, at
100/100,000 elements. No 104-byte wrapper overhead remains on these paths.

Full-document writes allocate **1,976/1,200,776 B per operation** for every
variant in both runs. Reads allocate **1,760/1,600,160 B**. The document
speedup therefore comes without an allocation reduction or increase.

Uint16 table writes allocate 536/200,336 B for every variant. Uint32 tables
allocate 712/400,312 B for the originals in both runs and the candidate in
run 1. In run 2, the candidate retains an additional **24 B per table** at
both sizes (736/400,336 B), with a slightly smaller small-table timing gain.
That residual variation shows that isolating validation does not make all JIT
allocation decisions identical. The subsequent [allocation investigation](table-allocation.md)
identifies the object as the writer list wrapper and replaces it with a
fixed-size array. It is absent from the document and direct-write measurements
in this earlier comparison.

## Comparisons

The original `ByteBuffer` is pinned to
`cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b`. Each comparison includes an identical
A/A worker. Other core sources come from the working tree and are identical
between variants.

- **Focused setters:** original versus the indexed uint16/uint32 setters now
  used in production. Direct writes include allocating the buffer and output.
  Table writes build an int64, the unsigned array and a float64 through
  `TableBuilder`. Inputs are deterministic random values across each type's
  full unsigned range, at 100 and 100,000 elements.
- **Full documents:** original versus the actual working production runtime,
  with its original reader and the three adopted unsigned setters. Uses the
  actual `BuffyCodec` and shared `Workloads.documentInput` at 100 and 100,000
  rows. Measures serialization and deserialization in fresh JVMs with no
  preceding array microbenchmarks. The document contains a uint32 key column,
  two float32 columns, a base integer, coordinates and a name in nested tables.
  It contains no uint16 array, so this confirms only the uint32 setter's
  effect end to end. Uint16 is covered by the focused direct/table workloads.

The production comparison uses `uint32/run.py --production`, which copies
the working runtime without modifying its reader or adding SIMD. Both
comparisons compile for Java 21 and run without the Vector API module.

## Method and limits

Each comparison is repeated with an independent set of three JVMs. Workers
run interleaved in alternating/rotating order, with separate concrete timing
loops, 16 warm-up rounds and 20 measured rounds of approximately 100 ms each.
Every result is published to a volatile sink. Each JVM uses a fixed 2 GiB
heap with normal JIT and GC settings. Pipe traffic and result reporting are
outside timing. GC samples remain included.

Measured on 2026-11-01 on a 10-core Apple M2 Pro, 32 GiB RAM, macOS 27.0.1,
Oracle JDK 25.0.1+8-27 aarch64. Background applications were active, including
a CPU-intensive Biome process. A/A controls and repeated runs help assess
variation, but this is not a dedicated benchmark machine. Inputs are reused;
these are steady successful workloads, not cold-cache or exception-heavy
profiles. No Java 21, x86 or big-endian performance claims follow from them.

Initial one-minute load averages were 2.18 and 3.15 for the focused runs,
then 2.67 and 3.11 for the document runs. Comparisons ran sequentially.

Fastest samples and medians are reported separately for each run. Percentage
improvements compare the candidate's fastest sample with the faster original
A/A worker in that run. Allocation measurements cover the calling thread,
not peak RSS or compiler/GC thread allocation. Absolute times should not be
compared directly with JMH results.

## Reproduce

From `java/`, with `JAVA_HOME` set to the desired JDK:

```sh
uptime
python3 benchmarks/experiments/uint-setters/run.py \
    --operations uint16_array_write uint16_table_write uint32_array_write uint32_table_write \
    --label isolated-wide-run1
uptime
python3 benchmarks/experiments/uint-setters/run.py \
    --operations uint16_array_write uint16_table_write uint32_array_write uint32_table_write \
    --label isolated-wide-run2
uptime
python3 benchmarks/experiments/uint32/run.py --production \
    --operations document_write document_read --label production-document-run1
uptime
python3 benchmarks/experiments/uint32/run.py --production \
    --operations document_write document_read --label production-document-run2
python3 benchmarks/experiments/uint32/summarize.py \
    benchmarks/build/uint-setters/isolated-wide-run1.csv \
    benchmarks/build/uint-setters/isolated-wide-run2.csv \
    benchmarks/build/uint32/production-document-run1.csv \
    benchmarks/build/uint32/production-document-run2.csv
```

Setters are checked against an independent little-endian byte oracle,
including unsigned boundaries, discarded high bits, empty arrays, lengths
0–65, all offset widths, unaligned starts, following fields and short output
buffers without partial writes. Both measured table sizes are checked byte
for byte and read back. Documents are checked against an unchanged reference
for both complete bytes and decoded values at both sizes. The document
harness also checks malformed arrays fail with the public decode exception.

All four performance runs passed their separate validation process on Java
25. The focused setters and production document validation also passed on
Java 21. The modified uint32 harness's original scalar/SIMD validation mode
passed on Java 25. Production sources are unchanged from the preceding
successful Java 25 build and Java 21 test run.

Raw results: [focused run 1](results/isolated-wide-run1.csv),
[focused run 2](results/isolated-wide-run2.csv),
[document run 1](results/production-document-run1.csv),
[document run 2](results/production-document-run2.csv).
[Environment and source hashes](results/isolated-wide-environment.json).

## Detailed samples

Each timing cell is **fastest / median µs per operation**, over 20 measured
samples. Allocation tables report median calling-thread bytes per operation.
`Scalar` denotes the indexed setters; `Production` is the working runtime.

### isolated-wide-run1

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint16_array_write | 0.0227 / 0.0232 | 0.0226 / 0.0233 | 0.0218 / 0.0222 |
| 100 | uint16_table_write | 0.0614 / 0.0624 | 0.0616 / 0.0624 | 0.0549 / 0.0554 |
| 100 | uint32_array_write | 0.0195 / 0.0198 | 0.0193 / 0.0196 | 0.0191 / 0.0197 |
| 100 | uint32_table_write | 0.0634 / 0.0644 | 0.0622 / 0.0632 | 0.0496 / 0.0503 |
| 100000 | uint16_array_write | 18.0301 / 18.2595 | 17.9198 / 18.1792 | 17.5702 / 17.8376 |
| 100000 | uint16_table_write | 26.7786 / 27.3857 | 26.4619 / 26.8465 | 17.6608 / 17.9651 |
| 100000 | uint32_array_write | 18.0606 / 18.3459 | 17.9130 / 18.2208 | 18.2219 / 18.4088 |
| 100000 | uint32_table_write | 28.7473 / 29.5489 | 29.4932 / 30.0578 | 17.9471 / 18.1441 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint16_array_write | 224 | 224 | 224 |
| 100 | uint16_table_write | 536 | 536 | 536 |
| 100 | uint32_array_write | 424 | 424 | 424 |
| 100 | uint32_table_write | 712 | 712 | 712 |
| 100000 | uint16_array_write | 200024 | 200024 | 200024 |
| 100000 | uint16_table_write | 200336 | 200336 | 200336 |
| 100000 | uint32_array_write | 400024 | 400024 | 400024 |
| 100000 | uint32_table_write | 400312 | 400312 | 400312 |

### isolated-wide-run2

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint16_array_write | 0.0227 / 0.0232 | 0.0227 / 0.0231 | 0.0217 / 0.0224 |
| 100 | uint16_table_write | 0.0616 / 0.0623 | 0.0614 / 0.0622 | 0.0546 / 0.0552 |
| 100 | uint32_array_write | 0.0196 / 0.0197 | 0.0191 / 0.0194 | 0.0195 / 0.0197 |
| 100 | uint32_table_write | 0.0630 / 0.0639 | 0.0628 / 0.0640 | 0.0536 / 0.0540 |
| 100000 | uint16_array_write | 18.1401 / 18.3741 | 18.1333 / 18.3021 | 17.4983 / 17.7201 |
| 100000 | uint16_table_write | 26.8365 / 27.1444 | 26.7985 / 27.1494 | 17.5562 / 17.7639 |
| 100000 | uint32_array_write | 18.0009 / 18.3741 | 17.7937 / 18.2828 | 18.1423 / 18.2776 |
| 100000 | uint32_table_write | 29.9209 / 30.4354 | 29.6446 / 30.3210 | 17.7779 / 17.9997 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint16_array_write | 224 | 224 | 224 |
| 100 | uint16_table_write | 536 | 536 | 536 |
| 100 | uint32_array_write | 424 | 424 | 424 |
| 100 | uint32_table_write | 712 | 712 | 736 |
| 100000 | uint16_array_write | 200024 | 200024 | 200024 |
| 100000 | uint16_table_write | 200336 | 200336 | 200336 |
| 100000 | uint32_array_write | 400024 | 400024 | 400024 |
| 100000 | uint32_table_write | 400312 | 400312 | 400336 |

### production-document-run1

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 0.2044 / 0.2072 | 0.2037 / 0.2074 | 0.1896 / 0.1933 |
| 100 | document_read | 0.0753 / 0.0762 | 0.0750 / 0.0763 | 0.0750 / 0.0761 |
| 100000 | document_write | 59.4117 / 60.7038 | 59.4498 / 60.7821 | 40.4312 / 40.9885 |
| 100000 | document_read | 46.3600 / 47.6523 | 46.2235 / 47.5622 | 46.6406 / 47.1653 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 1976 | 1976 | 1976 |
| 100 | document_read | 1760 | 1760 | 1760 |
| 100000 | document_write | 1200776 | 1200776 | 1200776 |
| 100000 | document_read | 1600160 | 1600160 | 1600160 |

### production-document-run2

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 0.2058 / 0.2089 | 0.2037 / 0.2082 | 0.1897 / 0.1921 |
| 100 | document_read | 0.0756 / 0.0769 | 0.0771 / 0.0780 | 0.0750 / 0.0762 |
| 100000 | document_write | 56.5705 / 58.5244 | 59.5523 / 60.9711 | 40.0794 / 40.8511 |
| 100000 | document_read | 46.2261 / 47.2173 | 46.3440 / 47.2863 | 46.2847 / 47.1272 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 1976 | 1976 | 1976 |
| 100 | document_read | 1760 | 1760 | 1760 |
| 100000 | document_write | 1200776 | 1200776 | 1200776 |
| 100000 | document_read | 1600160 | 1600160 | 1600160 |
