# Java uint32 conversion experiment

Compares the current runtime, a simpler scalar loop, and explicit SIMD for
uint32 array reads and writes, both alone and in the complete document codec.
The benchmark uses generated runtime copies. This experiment requires JDK 25
and its `jdk.incubator.vector` module; production adoption is covered by the
[unsigned-setter follow-up](../uint-setters/README.md).

The [corrected setter and document comparison](../uint-setters/isolated-wide.md)
reruns the production setters with correctness validation in a separate JVM.
The historical results below ran validation inside each timed JVM. The
current harness isolates validation by default; use `--validation-in-worker`
to reproduce the old setup. `--production` compares the actual working runtime
with the pinned original and an identical A/A worker, without changing the
reader or requiring the Vector API. That mode compiles for Java 21.

## Variants

`run.py` copies the actual core sources and `BuffyCodec` into the ignored
`benchmarks/build/uint32/src` directory. Only the following methods change:

- **Current:** `getUint32Array` and `setUint32Array` from the pinned revision.
- **Scalar:** indexed addressing from a local start position; the setter
  updates its position once after the loop. Checks and allocation stay the same.
- **SIMD:** load bytes into an integer vector, widen to longs and mask with
  `0xffff_ffffL` on reads; narrow longs to integers and store their bytes on
  writes. Reinterpretation uses little-endian lane ordering. The long species
  is the preferred hardware width, and the integer species has the same lane
  count and half the bit width. Scalar code handles the tail.
- **A/A repeat:** another JVM running the exact same current implementation.

The comparison pins `ByteBuffer` to the source revision recorded below, so
applying the winning setter to the runtime does not change its baseline.
`--baseline <revision>` selects another Git revision for that buffer.

On the measured M2 Pro, the SIMD loop converts two elements per iteration
using 128-bit long vectors and 64-bit integer/byte vectors. The experiment does
not add intermediate numeric arrays. Generated variants preserve the original
builders, bounds checks, length prefixes, other field types and document codec.

## Run

From `java/`, with `JAVA_HOME` set to a JDK 25 installation:

```sh
./gradlew build
uptime
python3 benchmarks/experiments/uint32/run.py --label run1
python3 benchmarks/experiments/uint32/run.py --label run2
python3 benchmarks/experiments/uint32/summarize.py \
    benchmarks/build/uint32/run1.csv benchmarks/build/uint32/run2.csv
```

`--java-home /path/to/jdk` can replace `JAVA_HOME`. The script uses JSpecify
from the normal Gradle cache. `--validate-only` runs the correctness checks
without benchmarking. `--rounds`, `--warmup` and `--millis` control the sample
counts and approximate sample duration. `--rows 100000 --operations document_write`
runs just the large document write in fresh workers. Compilation uses JDK 25 independently
of the production Gradle build.

## Measurement

Each run starts four JVMs, one per variant, with fixed 2 GiB heaps and default
JIT/GC settings. Only one worker is asked to execute a timing loop at a time;
the others wait on a pipe. The parent alternates and rotates their order.
Timing happens inside each worker, excluding pipe traffic and result reporting.
Separate JVMs isolate type profiles and compilation history, and every
operation has a dedicated loop. Input values come from the existing
`Workloads.documentInput`, with 100 or 100,000 rows.

For each operation and size, calibration is followed by 16 warm-up rounds and
20 measurement rounds per variant, aiming for 100 ms per sample. Iteration
counts adapt during warm-up and stay fixed during measurement. Every result
is published to a volatile sink, so allocations and output writes remain
observable. The sink adds a small fixed cost, especially visible for small
arrays. This is a custom interleaved harness, not JMH.

Array writes include buffer construction, allocation, the length prefix and
conversion. Reads include buffer construction and the returned `long[]`.
Document measurements call the actual `BuffyCodec` document methods, including
all fields, builders, float columns and output allocations. The benchmark
retains the existing workload's uint32 values below 2^24; correctness checks
also cover the full unsigned range and discarded high bits.

CSV files record every sample's time, main-thread allocated bytes and GC count
and time. Allocation is measured with `ThreadMXBean`; it does not measure
peak RSS or allocations on GC/compiler threads. Fastest samples are reported
alongside medians, and separate runs are not pooled. GC remains enabled and
samples containing GC are retained.

## Correctness

A separate JVM checks every variant before timing (the historical runs below
performed these checks inside each timed worker):

- Lengths 0–65, including odd lengths and vector tails, for all three offset
  widths, with an unaligned array position.
- Zero, signed/unsigned integer boundaries, negative values, discarded high
  bits and deterministic random longs, against an independent little-endian
  byte oracle.
- A following scalar marker, checking that the setter advances correctly.
- Truncated prefixes and payloads, requiring the public decode exception.
- Both measured array sizes, and complete document bytes and values, against
  a separate unchanged reference runtime.

## Results

Measured on 2026-11-01, on a 10-core Apple M2 Pro with 32 GiB RAM,
macOS 27.0.1, Oracle JDK 25.0.1+8-27, aarch64. Source revision:
`cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b`.

**The simpler setter is the best first change.** It reduced full-document
write time by about 11–12% at 100 rows and 42% at 100,000 rows in both complete
runs. SIMD was about as fast as that scalar loop for large document writes.
A further run that timed only large document writes in fresh JVMs reproduced
the improvement, so preceding array benchmarks did not explain it.

Explicit SIMD did improve isolated large-array writes by 16–17%. Its large
array reads were 7–8% faster than the primary current worker, and complete
large-document reads about 5% faster. Treat the smaller read gains more
cautiously: the unchanged A/A workers differed by as much as 10% in the second
run's large-array read. SIMD small-array write times and scalar read times also
varied between fresh JVMs. There is no consistent reason to replace the
scalar reader with the proposed simpler loop.

The much larger setter benefit inside the document codec than in the isolated
array benchmark shows that call context matters. These results establish the
observed improvement; they do not identify which scalar JIT optimization caused
it. A separate HotSpot compilation diagnostic confirmed vector load, conversion
and store intrinsics in the SIMD methods, including the widening mask on reads.
It did not disassemble the generated instructions.

### Timings

Each cell is **fastest / median microseconds per operation**, over 20 measured
samples. Lower is better. Run 1 and run 2 are independent sets of four JVMs.

#### run1

| Rows | Operation | Current | A/A repeat | Scalar | SIMD |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | array_write | 0.0297 / 0.0299 | 0.0295 / 0.0296 | 0.0292 / 0.0294 | 0.0302 / 0.0305 |
| 100 | array_read | 0.0377 / 0.0381 | 0.0374 / 0.0381 | 0.0411 / 0.0417 | 0.0406 / 0.0413 |
| 100 | document_write | 0.2212 / 0.2240 | 0.2193 / 0.2231 | 0.1949 / 0.1970 | 0.1887 / 0.1930 |
| 100 | document_read | 0.1194 / 0.1210 | 0.1241 / 0.1250 | 0.1253 / 0.1260 | 0.1166 / 0.1174 |
| 100000 | array_write | 24.2282 / 24.5372 | 23.8069 / 24.5276 | 23.5152 / 23.9471 | 20.0815 / 20.2323 |
| 100000 | array_read | 25.5381 / 25.9182 | 25.7213 / 25.9935 | 28.2217 / 28.4971 | 23.4330 / 23.9310 |
| 100000 | document_write | 69.7232 / 70.7590 | 69.5761 / 70.3554 | 39.9805 / 41.1483 | 40.1438 / 40.9391 |
| 100000 | document_read | 46.9683 / 48.1357 | 47.1151 / 48.1301 | 49.1802 / 50.4919 | 44.5446 / 45.7140 |

#### run2

| Rows | Operation | Current | A/A repeat | Scalar | SIMD |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | array_write | 0.0299 / 0.0304 | 0.0314 / 0.0317 | 0.0314 / 0.0317 | 0.0245 / 0.0249 |
| 100 | array_read | 0.0372 / 0.0380 | 0.0406 / 0.0410 | 0.0380 / 0.0388 | 0.0404 / 0.0412 |
| 100 | document_write | 0.2182 / 0.2222 | 0.2172 / 0.2215 | 0.1939 / 0.1964 | 0.1896 / 0.1926 |
| 100 | document_read | 0.1191 / 0.1202 | 0.1289 / 0.1303 | 0.1168 / 0.1175 | 0.1152 / 0.1163 |
| 100000 | array_write | 24.0596 / 24.4945 | 24.0381 / 24.3510 | 24.1289 / 24.5500 | 20.3083 / 20.6872 |
| 100000 | array_read | 25.4209 / 26.0880 | 28.0348 / 28.3452 | 25.7508 / 25.9836 | 23.5793 / 23.8874 |
| 100000 | document_write | 70.5502 / 72.8765 | 70.3578 / 73.3663 | 41.0692 / 43.0375 | 41.3478 / 42.2541 |
| 100000 | document_read | 47.1135 / 48.0990 | 47.4887 / 48.8192 | 46.5185 / 48.0794 | 44.7513 / 45.7461 |

#### document-only

| Rows | Operation | Current | A/A repeat | Scalar | SIMD |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100000 | document_write | 69.9193 / 70.6826 | 69.0889 / 70.9017 | 40.2103 / 41.1259 | 40.1452 / 40.9112 |

### Allocation

Both complete runs allocated the same amount for full-document writes:
1,976 B/op for 100 rows and 1,200,776 B/op for 100,000 rows, across all
variants. The write speedup therefore did not come from allocating less.

SIMD isolated array writes allocated 56–104 B/op more than current, a fixed
per-call difference at both sizes. Current allocated 424 and 400,024 B/op for
the two sizes. Array-read allocation was unchanged at 920 and 800,120 B/op.
No per-element vector allocation remained in the measured SIMD implementation.
The scalar document reader allocated 112 B/op less in run 2 only, another
indication that JIT allocation elimination varied across compilations.

### Limits and recommendation

- Background activity was present, including another process using roughly one
  core. One-minute load averages at the full runs' starts were 3.81 and 3.11;
  the document-only check started at 4.69. A/A controls, interleaving, medians
  and the independent repeat help assess this noise, but do not make it a
  dedicated benchmark machine.
- These results apply to this ARM machine and JDK. No x86 or Java 21
  performance measurements were made. Inputs are reused, so this is a steady
  workload rather than a cold-cache test.
- The fixed volatile sink and custom harness mean absolute times should not
  be compared directly with the repository's JMH results.
- The initial exploratory harness shared a JVM across variants and exposed
  different JIT profiles in its A/A comparison. It was replaced before these
  recorded runs. An initial MemorySegment SIMD store path also allocated per
  element; the recorded variant uses byte-array vector loads and stores.

Recommend evaluating **only the simpler setter** for production first. It
retains Java 21 compatibility and captured the large document-write gain
without an incubating module. Keep the current reader unless a separate
change earns a consistent improvement. Explicit SIMD remains a candidate for
large-array workloads, but its extra platform requirements buy little beyond
the simpler setter on the complete document write tested here.

At the time of measurement, production sources were unchanged. The simpler
uint32 setter was subsequently applied; the [unsigned-setter follow-up](../uint-setters/README.md)
checks the same change for uint8 and uint16. All variant correctness
checks passed. `./gradlew build` passed using cached tasks; the core and
benchmark tests were also rerun explicitly with `./gradlew test --rerun-tasks`.

Raw samples: [run 1](results/run1.csv), [run 2](results/run2.csv),
[document-only check](results/document-only.csv).
[Environment and compilation evidence](results/environment.json) includes
source hashes and the vector intrinsics reported by HotSpot.
