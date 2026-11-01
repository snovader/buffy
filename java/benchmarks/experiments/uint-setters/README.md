# Unsigned array setters

Follow-up to the [uint32 SIMD experiment](../uint32/README.md): apply the
simpler uint32 setter, and measure whether uint8 and uint16 benefit from the
same change. Reads are unchanged. This experiment uses ordinary Java 21 APIs,
with no Vector API module.

The subsequent [allocation investigation](allocation.md) corrects the uint8
conclusion below and in the [local-cursor experiment](cursor.md). These older
runs validated inside each timed JVM, affecting JIT inlining and escape
analysis. The harness now validates in a separate JVM. Two new uint8 runs
retain the table gain without the extra direct-write allocation. Production
now uses a local cursor for uint8 and indexed loops for uint16 and uint32.
The [uint16/uint32 and full-document rerun](isolated-wide.md) now measures
those setters with isolated validation too. Use that report for current
performance conclusions; the historical results below remain unchanged.

## Change

The original setters advance the buffer's `position` field on every element.
The candidate saves the start in a local variable, indexes each write from
that start, then updates `position` once after the loop. It keeps the existing
up-front length/capacity checks and casts, including truncation to the low bits.

`run.py` generates isolated runtime copies, replacing only those three setter
methods in the candidate. It pins the baseline `ByteBuffer` to Git revision
`cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b`, so applying a setter to production
does not silently change the comparison. The other core files come from the
working tree. `--baseline` can select another revision.

## Workloads and method

- Direct array writes, including the buffer and output allocation, for each
  unsigned width at 100 and 100,000 elements.
- A three-field table built with `TableBuilder`: an int64, the unsigned array,
  and a float64. This measures the setter in the builder's writing callback.
- Inputs are deterministic random values over the unsigned type's full range.
  These are focused setter workloads, not replacements for the shared document
  benchmarks used to compare codecs or languages.
- Three separate warmed JVMs: original, an identical A/A repeat, and candidate.
  The earlier experiment's scheduler interleaves them in alternating/rotating
  order. Each operation/width has its own loop; every result reaches a volatile
  sink. Pipe communication is outside the timing.
- 16 warm-up rounds and 20 measured rounds per variant, approximately 100 ms
  per sample. A fixed 2 GiB heap per JVM, with normal JIT and GC settings.
- Main-thread allocated bytes and GC counts/times accompany every sample.
  They do not measure peak RSS or allocations on GC/compiler threads.

Before timing, a separate JVM verifies bytes against an independent little-endian oracle,
truncation of arbitrary values, empty arrays, all offset widths, unaligned
starts, a following scalar, and rejection of short output buffers without
writing. It also compares complete table bytes with an unchanged reference
and reads back all three fields at both measured lengths.

## Reproduce

From `java/`, with `JAVA_HOME` set to the desired JDK (21 or later):

```sh
./gradlew build
uptime
python3 benchmarks/experiments/uint-setters/run.py --label run1
python3 benchmarks/experiments/uint-setters/run.py --label run2
python3 benchmarks/experiments/uint32/summarize.py \
    benchmarks/build/uint-setters/run1.csv \
    benchmarks/build/uint-setters/run2.csv
```

The commands above use the corrected setup. Add `--validation-in-worker`
to reproduce the setup used for the historical results below.

`--java-home /path/to/jdk` overrides `JAVA_HOME`. Compilation uses `--release
21` and the normal Gradle cache for JSpecify. `--validate-only` skips timing.
For a narrower run, use `--rows 100000 --operations uint16_array_write
uint16_table_write`. The source tree and transient output are under the ignored
`benchmarks/build/uint-setters` directory. Recorded samples are kept under
`results/` beside this report.

## Historical results and decisions

Measured on 2026-11-01 on the 10-core Apple M2 Pro with 32 GiB RAM,
macOS 27.0.1 and Oracle JDK 25.0.1+8-27 (aarch64). Two independent sets of
three JVMs ran all workloads. The runtime changes remain compatible with
Java 21; no Java 21 performance measurements were made.

- **uint32: applied**, as requested. The candidate is the same indexed setter
  selected by the earlier full-document experiment. The focused tests here
  also showed faster direct and table writes at both sizes.
- **uint16: applied too.** At 100,000 elements, direct writes took 17.8–20.1 µs
  versus 25.7–26.2 µs for the faster unchanged worker in each run: a 24–31%
  reduction. Table writes took about 17.9 µs versus 25.5–25.8 µs, about 30%
  less time. At 100 elements, direct writes improved by 14–20%; table results
  fell inside the substantial A/A variation, so they do not establish a gain.
- **uint8: kept the original setter.** The candidate improved large table
  writes by about 38%, but it regressed direct writes by 49% at 100 elements
  and 14% at 100,000 in the first run, with an extra 104 B/op. In the second
  run it matched the original direct-write time and allocation. That is a
  workload/compiler tradeoff, not a consistent improvement worth applying
  along with the other setters.

Percentages above compare the candidate's fastest sample with the faster of
the two unchanged A/A workers in the same run. The table below retains both
baselines and medians, rather than concealing their differences.

### Timings

Each cell is **fastest / median µs per operation** over 20 measured samples.
The three-field table here differs from the earlier full-document workload;
absolute times should not be compared across those two experiments.

#### run1

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0213 / 0.0216 | 0.0214 / 0.0216 | 0.0318 / 0.0323 |
| 100 | uint8_table_write | 0.0816 / 0.0833 | 0.0661 / 0.0670 | 0.0730 / 0.0748 |
| 100 | uint16_array_write | 0.0369 / 0.0376 | 0.0368 / 0.0374 | 0.0317 / 0.0325 |
| 100 | uint16_table_write | 0.0763 / 0.0773 | 0.0651 / 0.0666 | 0.0682 / 0.0696 |
| 100 | uint32_array_write | 0.0427 / 0.0439 | 0.0424 / 0.0441 | 0.0305 / 0.0311 |
| 100 | uint32_table_write | 0.0830 / 0.0846 | 0.0685 / 0.0692 | 0.0665 / 0.0671 |
| 100000 | uint8_array_write | 17.2132 / 17.3287 | 17.1169 / 17.3153 | 19.6276 / 19.8656 |
| 100000 | uint8_table_write | 27.9118 / 28.1858 | 27.9189 / 28.0186 | 17.3023 / 17.4049 |
| 100000 | uint16_array_write | 26.9977 / 27.3552 | 26.2472 / 26.5696 | 20.0724 / 20.3366 |
| 100000 | uint16_table_write | 26.8709 / 27.1388 | 25.8295 / 26.1102 | 17.8806 / 18.0491 |
| 100000 | uint32_array_write | 36.5160 / 40.5694 | 33.9825 / 41.0317 | 19.3195 / 19.4449 |
| 100000 | uint32_table_write | 33.3762 / 34.6959 | 36.1530 / 38.4686 | 17.8425 / 18.1985 |

#### run2

| Rows | Operation | Current | A/A repeat | Scalar |
| ---: | --- | ---: | ---: | ---: |
| 100 | uint8_array_write | 0.0216 / 0.0218 | 0.0216 / 0.0218 | 0.0213 / 0.0217 |
| 100 | uint8_table_write | 0.0669 / 0.0678 | 0.0823 / 0.0837 | 0.0723 / 0.0734 |
| 100 | uint16_array_write | 0.0370 / 0.0375 | 0.0465 / 0.0469 | 0.0296 / 0.0298 |
| 100 | uint16_table_write | 0.0657 / 0.0680 | 0.0793 / 0.0804 | 0.0685 / 0.0692 |
| 100 | uint32_array_write | 0.0442 / 0.0449 | 0.0424 / 0.0428 | 0.0307 / 0.0311 |
| 100 | uint32_table_write | 0.0686 / 0.0698 | 0.0789 / 0.0800 | 0.0668 / 0.0671 |
| 100000 | uint8_array_write | 17.1997 / 17.3567 | 17.1738 / 17.3208 | 17.1771 / 17.2570 |
| 100000 | uint8_table_write | 27.9259 / 28.0993 | 27.9141 / 28.2002 | 17.1978 / 17.3610 |
| 100000 | uint16_array_write | 27.2940 / 27.6098 | 25.6625 / 25.9299 | 17.8302 / 18.1258 |
| 100000 | uint16_table_write | 26.8611 / 27.2774 | 25.5153 / 25.9535 | 17.9321 / 18.3340 |
| 100000 | uint32_array_write | 37.1183 / 44.5491 | 37.8426 / 42.2589 | 19.3174 / 19.4650 |
| 100000 | uint32_table_write | 33.5715 / 35.3716 | 32.7930 / 34.3572 | 17.5422 / 18.0594 |

### Allocation and limitations

For uint16 direct writes, all variants allocated 328 B/op at 100 elements
and 200,128 B/op at 100,000. The candidate's table writes allocated 600 and
200,400 B/op. The two unchanged workers varied between 512–600 and
200,312–200,400 B/op: an 88-byte difference even for identical code. The
candidate retained the higher allocation total. This is small for the large
arrays but matters more for small tables.

For uint8 direct writes, the unchanged workers allocated 120 and 100,024 B/op.
The candidate allocated 104 B/op more in run 1 and matched them in run 2.
These allocation differences and the A/A timing differences show that JIT
compilation/escape analysis affects the result. We did not disassemble the
scalar loops or establish which compiler transformation caused each change.

Background activity included other CPU-intensive processes; the one-minute
load averages before the runs were 2.69 and 3.37. Alternating order, A/A controls
and independent repeats help assess noise but do not make this a dedicated
benchmark machine. Inputs are reused; this measures steady workloads. The
custom harness includes a volatile publication per result, and its absolute
times should not be compared directly with JMH numbers. No x86 or big-endian
performance measurements were made.

### Runtime verification

At the time of these runs the production diff changed only `setUint16Array` and `setUint32Array`.
Neither allocates a new buffer or changes the bytes, truncation semantics,
length/capacity checks, or public API. `setUint8Array` was subsequently changed
to a local cursor after the [allocation investigation](allocation.md).
All readers remain as they were.

A new public-API property in `ByteBufferTest` checks arbitrary values' low
bits, exact little-endian bytes, all three offset widths, unaligned starts,
and a following field. The short-buffer example now covers all three unsigned
setters. The experiment validates both measured sizes and its boundary cases
before any timing.

Passed:

- `./gradlew build` (including Error Prone, NullAway, core/conformance tests,
  benchmark round trips and JMH compilation).
- `./gradlew test -PtestJavaVersion=21`.
- Both experiment validation suites; the original uint32 experiment now pins
  its old buffer revision so it still reproduces the original comparison.

Raw samples: [run 1](results/run1.csv), [run 2](results/run2.csv).
[Environment and source hashes](results/environment.json).
