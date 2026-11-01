# The extra 24 bytes in uint32 table writes

The extra object is the **24-byte `ArrayList` holding `TableBuilder`'s field
writers**. It is unrelated to per-element uint32 conversion. Depending on
compilation history, HotSpot either eliminates the list wrapper or retains it.

The runtime now stores the writers in a fixed-size array, using the field
count already known at construction. This removes the list wrapper and its
growth path from the source. The uint32 setter remains the indexed loop.

## Results after connecting AC power

The powered focused repeat confirms the uint32 table-write gain:

| uint32 workload | Indexed setter, list writers | Indexed setter, fixed writers |
| --- | ---: | ---: |
| 100-element table, fastest µs | 0.0530 | 0.0477 |
| 100,000-element table, fastest µs | 17.7974 | 17.7348 |
| 100-element table, B/op | 736 | 680 |
| 100,000-element table, B/op | 400,336 | 400,280 |

Large tables remain about **38% faster than the original setter/list builder**
(28.7900 µs for the faster original A/A worker versus 17.7348 µs). The
fixed-array version differs from the adopted indexed setter by only 0.4% on
large tables and improves the small table by about 10%. Direct writes retain
their output-only allocation and similar timing. The uint16 table comparison
likewise retains its large-table improvement and allocates 56 B less. This
agrees with the first focused run, which measured 17.4599 → 17.5303 µs for
the same builder change and a 40% gain over its faster original A/A worker.

Two independent powered full-document runs confirm that removing the writer
list preserves the adopted setter's end-to-end performance:

| Document write | Faster old-builder A/A, µs | Fixed writers, µs | Change |
| --- | ---: | ---: | --- |
| 100 rows, powered run 1 | 0.1909 | 0.1682 | 11.9% less time |
| 100 rows, powered run 2 | 0.1903 | 0.1699 | 10.7% less time |
| 100,000 rows, powered run 1 | 40.8446 | 40.9348 | 0.2% more time |
| 100,000 rows, powered run 2 | 41.2354 | 41.0861 | 0.4% less time |

Every document write allocates **48 fewer bytes** in both runs:
1,976 → 1,928 B at 100 rows and 1,200,776 → 1,200,728 B at 100,000 rows.
Reads allocate identically, with fastest timings within 2% at 100 rows and
1% at 100,000 rows; no reader change or consistent read gain is claimed.
The runtime and benchmark source hashes are unchanged from the allocation
investigation. No further runtime changes were needed after the powered
verification.

## Earlier runs with unstable load and low battery

The second focused run again allocates **680/400,280 B** for uint32 tables,
versus **712/400,312 B** for the list builder in that compilation. It confirms
the memory result, including against a compilation that already eliminated
the list wrapper. However, system load rose from 4.36 to 16.05, and all
variants slowed roughly threefold. Its timings are retained below for
transparency but are excluded from the main timing conclusion.

The first full-document comparison also allocated **48 fewer bytes per write**:
1,976 → 1,928 B at 100 rows and 1,200,776 → 1,200,728 B at 100,000 rows.
Reads allocate identically. This run also experienced unstable conditions;
the laptop was observed at **3% battery**, and both unchanged read and write
loops were much slower than in earlier runs. The large-document fastest
sample regressed while its median was essentially unchanged, so it does
**not** establish a reliable document timing result. Timing was paused after
observing the battery state and resumed after AC power was connected. The
powered results above replace this run for timing conclusions. All original
samples are retained below.

## Evidence

Two fresh diagnostic JVMs reproduced 736 B per 100-element table and
400,336 B per 100,000-element table. The lower-allocation compilations from
the [preceding comparison](isolated-wide.md) used 712 and 400,312 B.

C2 compilation logs identify the difference. In the higher-allocation
compilation of `table_scalar_32`, `ArrayList.grow` is rejected for inlining
as **"too big"** at the three field additions. HotSpot eliminates the
`TableBuilder` object but retains its `ArrayList`. A later compilation
inlines the growth path and explicitly records elimination of both objects;
the allocation total drops by exactly 24 bytes. Although a successful builder
never needs to grow its writer list, the un-inlined growth path prevents
scalar replacement of the list in the first compilation.

Separate controls reproduce the effect without relying on the later
recompilation:

| Diagnostic, indexed uint32 setter | 100 elements, B/op | 100,000 elements, B/op |
| --- | ---: | ---: |
| Force `ArrayList.grow` inline | 712 | 400,312 |
| Prevent `ArrayList.grow` inlining | 736 | 400,336 |
| Fixed writer array, no inline override | 680 | 400,280 |

These controls measure allocation only; their timings are not performance
evidence. Each diagnostic first warms the focused operations in the same
order as the earlier comparison, then records six allocation samples. All
six agree for each cell above.

A separate JFR recording with TLABs disabled records one 24-byte
`java.util.ArrayList` allocation per table, originating in the builder
constructor. Disabling TLABs also changes compilation behavior: that trace
retains the 64-byte builder and allocates 800 B/op. It establishes object size
and allocation site, not the ordinary benchmark's allocation total. The
normal-TLAB compilation logs and inline controls establish the 24-byte delta.

The fixed-array diagnostic also records elimination of a 32-byte builder
`int[]`. This explains the measured 56-byte reduction from the 736-byte case:
24 bytes from removing the wrapper, plus 32 bytes from another JIT
optimization. Only removal of the wrapper is guaranteed by the source change;
other allocation elimination remains compiler-dependent.

## Runtime change and behavior

`TableBuilder` now owns a `Consumer<ByteBuffer>[]` of exactly the requested
field count and tracks how many fields have been set. Its existing positions
array supplies the capacity, replacing the redundant stored total field count
without increasing the builder's object size. A private generic array
factory confines the unchecked cast; only consumers of
`ByteBuffer` are stored, and the array never escapes the builder API.

The builder still rejects extra fields and oversized fields before changing
its state, rejects incomplete writes, writes fields in order, releases writer
references after writing, and returns the cached bytes on subsequent calls.
Nested builders, deferred array contents and the wire format are unchanged.
The existing public-API properties and conformance cases cover these contracts.

## Measurement

Three focused runs compare four separate JVMs: original setters/list builder,
an identical original A/A worker, indexed setters/list builder, and indexed
setters/fixed writer array. Both `ByteBuffer` and the old `TableBuilder` are
pinned to `cfec1f8f07820fa9896831a1d7b5633f1ffbfc7b` before generating variants.
The indexed uint16/uint32 methods and fixed-array builder match production.

Three full-document runs compare the old list builder, an identical A/A worker,
and the new array builder, **all with the same adopted production setters
and unchanged readers**. This isolates the writer-storage change in the
actual document codec, including nested tables and the uint32 column.

All timing runs validate in a separate JVM first. They use 16 warm-up rounds
and 20 measured rounds of approximately 100 ms, interleaved in
alternating/rotating order. Each variant has concrete loops, a volatile result
sink, a fixed 2 GiB heap and normal JIT/GC settings. No allocation diagnostic,
JFR, TLAB override or inline override is present in the timing runs.

Machine: 10-core Apple M2 Pro, 32 GiB RAM, macOS 27.0.1, Oracle JDK
25.0.1+8-27 aarch64, on 2026-11-01. Background applications were active.
This is not a dedicated benchmark machine. Main-thread allocated bytes do
not measure peak RSS or compiler/GC-thread allocation. Inputs are reused;
results apply to these steady workloads, not all JIT profiles or platforms.
Initial one-minute loads were 3.86 and 4.36 for the focused runs, and 10.03
for the document run. The last two runs' timing limitations are described above.
The powered focused run began at load 3.85 and 47% battery, charging; its two
document repeats began at loads 4.22 and 5.43, with 49% and 50% battery,
charging. All three reported AC power. They ran sequentially, with a fresh
set of JVMs for each comparison and separate correctness validation before
each run. The unchanged A/A controls are retained in every result table.

## Verification and reproduction

With the production change applied, `./gradlew build` on Java 25 and
`./gradlew test -PtestJavaVersion=21` passed, including public-API properties,
conformance tests, benchmark round trips, Error Prone and NullAway. All timing
runs also validate their generated candidates before timing.

From `java/`, with `JAVA_HOME` set to JDK 25 for comparable performance:

```sh
uptime
python3 benchmarks/experiments/uint-setters/run.py --include-fixed \
    --operations uint16_array_write uint16_table_write uint32_array_write uint32_table_write \
    --label fixed-writers-powered
uptime
python3 benchmarks/experiments/uint32/run.py --production --writers-only \
    --operations document_write document_read --label fixed-writers-powered-document1
uptime
python3 benchmarks/experiments/uint32/run.py --production --writers-only \
    --operations document_write document_read --label fixed-writers-powered-document2
```

Allocation controls, separately from timing:

```sh
python3 benchmarks/experiments/uint-setters/run.py --include-fixed --validate-only
python3 benchmarks/experiments/uint-setters/table-allocation-probe.py \
    --java-home "$JAVA_HOME" --label table-grow-inline --repeats 1 \
    --jvm-arg=-XX:CompileCommand=inline,java.util.ArrayList::grow
python3 benchmarks/experiments/uint-setters/table-allocation-probe.py \
    --java-home "$JAVA_HOME" --label table-grow-no-inline --repeats 1 \
    --jvm-arg=-XX:CompileCommand=dontinline,java.util.ArrayList::grow
python3 benchmarks/experiments/uint-setters/table-allocation-probe.py \
    --java-home "$JAVA_HOME" --variant fixed --label table-fixed --repeats 1
```

For the optional JFR allocation-site check, use `--no-tlab --record` in a
separate diagnostic run. `AllocationEvents.java` streams its recording and
prints allocation class, bytes, count and stack for the measured table loop:

```sh
javac --release 21 -d benchmarks/build/uint-setters/allocation-probe \
    benchmarks/experiments/uint-setters/AllocationEvents.java
java -cp benchmarks/build/uint-setters/allocation-probe experiment.diagnostics.AllocationEvents \
    benchmarks/build/uint-setters/table-scalar-trace-1.jfr scalar
```

Evidence: [allocation and compiler controls](results/table-allocation-diagnostics.json),
[JFR allocation sites](results/table-allocation-events.json),
[environment/source hashes](results/table-allocation-environment.json),
[focused run 1](results/fixed-writers-run1.csv),
[focused run 2 under load](results/fixed-writers-run2.csv), and
[document run under load](results/fixed-writers-document1.csv),
[powered focused repeat](results/fixed-writers-powered.csv),
[powered document run 1](results/fixed-writers-powered-document1.csv), and
[powered document run 2](results/fixed-writers-powered-document2.csv).

## Recorded samples

Timing cells are **fastest / median µs per operation**. Each has 20 samples.
Allocation tables show median calling-thread bytes per operation. In the
document table, `Current` and `A/A repeat` use the old list builder with the
adopted setters; `Production` uses the new fixed-array builder with the same
setters. Focused run 1 and the three powered repeats support the timing
conclusions above; the low-battery/load runs are retained but excluded.

### fixed-writers-run1

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 0.0224 / 0.0231 | 0.0223 / 0.0230 | 0.0218 / 0.0222 | 0.0217 / 0.0223 |
| 100 | uint16_table_write | 0.0607 / 0.0615 | 0.0610 / 0.0623 | 0.0541 / 0.0547 | 0.0479 / 0.0486 |
| 100 | uint32_array_write | 0.0192 / 0.0197 | 0.0192 / 0.0199 | 0.0193 / 0.0197 | 0.0190 / 0.0193 |
| 100 | uint32_table_write | 0.0659 / 0.0669 | 0.0631 / 0.0637 | 0.0528 / 0.0537 | 0.0477 / 0.0486 |
| 100000 | uint16_array_write | 18.1285 / 18.3039 | 17.8548 / 18.3559 | 17.6924 / 17.9397 | 17.8383 / 18.0737 |
| 100000 | uint16_table_write | 26.8008 / 27.1609 | 26.9551 / 27.1308 | 17.6910 / 17.8839 | 17.7493 / 18.0065 |
| 100000 | uint32_array_write | 17.9715 / 18.1746 | 18.0298 / 18.2901 | 17.7805 / 18.1444 | 17.8767 / 18.0800 |
| 100000 | uint32_table_write | 31.9805 / 34.0350 | 29.3841 / 30.1282 | 17.4599 / 17.7330 | 17.5303 / 17.7998 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 224 | 224 | 224 | 224 |
| 100 | uint16_table_write | 536 | 536 | 536 | 480 |
| 100 | uint32_array_write | 424 | 424 | 424 | 424 |
| 100 | uint32_table_write | 736 | 712 | 736 | 680 |
| 100000 | uint16_array_write | 200024 | 200024 | 200024 | 200024 |
| 100000 | uint16_table_write | 200336 | 200336 | 200336 | 200280 |
| 100000 | uint32_array_write | 400024 | 400024 | 400024 | 400024 |
| 100000 | uint32_table_write | 400336 | 400312 | 400336 | 400280 |

### fixed-writers-run2

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 0.0448 / 0.0909 | 0.0448 / 0.0990 | 0.0430 / 0.1172 | 0.0427 / 0.1070 |
| 100 | uint16_table_write | 0.1881 / 0.2503 | 0.1853 / 0.2497 | 0.1641 / 0.2131 | 0.1450 / 0.1940 |
| 100 | uint32_array_write | 0.0742 / 0.0836 | 0.0698 / 0.0799 | 0.0714 / 0.0810 | 0.0709 / 0.0806 |
| 100 | uint32_table_write | 0.1904 / 0.2480 | 0.1904 / 0.2511 | 0.1504 / 0.1967 | 0.1450 / 0.1795 |
| 100000 | uint16_array_write | 55.2686 / 69.5536 | 53.0137 / 69.2242 | 53.2533 / 68.2124 | 51.2398 / 66.5052 |
| 100000 | uint16_table_write | 77.2419 / 93.9988 | 76.2295 / 96.1894 | 49.1748 / 67.1089 | 48.7296 / 67.2506 |
| 100000 | uint32_array_write | 58.5727 / 63.0890 | 56.1824 / 64.9551 | 57.6352 / 62.6800 | 56.4796 / 61.8314 |
| 100000 | uint32_table_write | 92.3330 / 112.0674 | 95.3240 / 122.0365 | 56.1640 / 66.7205 | 52.4558 / 65.9641 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 224 | 224 | 224 | 224 |
| 100 | uint16_table_write | 536 | 536 | 536 | 480 |
| 100 | uint32_array_write | 424 | 424 | 424 | 424 |
| 100 | uint32_table_write | 712 | 712 | 712 | 680 |
| 100000 | uint16_array_write | 200024 | 200024 | 200024 | 200024 |
| 100000 | uint16_table_write | 200336 | 200336 | 200336 | 200280 |
| 100000 | uint32_array_write | 400024 | 400024 | 400024 | 400024 |
| 100000 | uint32_table_write | 400312 | 400312 | 400312 | 400280 |

### fixed-writers-document1

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 0.5782 / 0.7175 | 0.5749 / 0.7290 | 0.5078 / 0.6300 |
| 100 | document_read | 0.2947 / 0.4764 | 0.3029 / 0.4393 | 0.2981 / 0.4251 |
| 100000 | document_write | 149.8132 / 180.6570 | 142.4971 / 174.7118 | 155.2478 / 174.2745 |
| 100000 | document_read | 165.0817 / 199.3266 | 179.1939 / 193.2509 | 167.9564 / 190.5509 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 1976 | 1976 | 1928 |
| 100 | document_read | 1760 | 1760 | 1760 |
| 100000 | document_write | 1200776 | 1200776 | 1200728 |
| 100000 | document_read | 1600160 | 1600160 | 1600160 |

### fixed-writers-powered

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 0.0228 / 0.0235 | 0.0227 / 0.0234 | 0.0218 / 0.0226 | 0.0218 / 0.0225 |
| 100 | uint16_table_write | 0.0606 / 0.0622 | 0.0610 / 0.0625 | 0.0543 / 0.0550 | 0.0519 / 0.0535 |
| 100 | uint32_array_write | 0.0193 / 0.0198 | 0.0189 / 0.0193 | 0.0194 / 0.0197 | 0.0193 / 0.0198 |
| 100 | uint32_table_write | 0.0659 / 0.0666 | 0.0626 / 0.0632 | 0.0530 / 0.0536 | 0.0477 / 0.0480 |
| 100000 | uint16_array_write | 18.1543 / 18.4478 | 18.1886 / 18.5906 | 17.6818 / 17.8787 | 17.7430 / 17.8742 |
| 100000 | uint16_table_write | 26.8546 / 27.2687 | 26.9194 / 27.3096 | 17.6853 / 17.9584 | 17.6343 / 17.8759 |
| 100000 | uint32_array_write | 18.2395 / 18.3758 | 18.1719 / 18.4133 | 18.1980 / 18.3582 | 18.2168 / 18.3610 |
| 100000 | uint32_table_write | 32.1626 / 33.5656 | 28.7900 / 29.5151 | 17.7974 / 18.0414 | 17.7348 / 18.0215 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Scalar | Fixed writers |
| ---: | --- | ---: | ---: | ---: | ---: |
| 100 | uint16_array_write | 224 | 224 | 224 | 224 |
| 100 | uint16_table_write | 536 | 536 | 536 | 480 |
| 100 | uint32_array_write | 424 | 424 | 424 | 424 |
| 100 | uint32_table_write | 736 | 712 | 736 | 680 |
| 100000 | uint16_array_write | 200024 | 200024 | 200024 | 200024 |
| 100000 | uint16_table_write | 200336 | 200336 | 200336 | 200280 |
| 100000 | uint32_array_write | 400024 | 400024 | 400024 | 400024 |
| 100000 | uint32_table_write | 400336 | 400312 | 400336 | 400280 |

### fixed-writers-powered-document1

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 0.1913 / 0.1937 | 0.1909 / 0.1941 | 0.1682 / 0.1706 |
| 100 | document_read | 0.0766 / 0.0777 | 0.0769 / 0.0776 | 0.0753 / 0.0769 |
| 100000 | document_write | 41.1050 / 41.5924 | 40.8446 / 41.6395 | 40.9348 / 41.4809 |
| 100000 | document_read | 47.0201 / 47.8625 | 46.9407 / 47.4992 | 47.0345 / 47.5965 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 1976 | 1976 | 1928 |
| 100 | document_read | 1760 | 1760 | 1760 |
| 100000 | document_write | 1200776 | 1200776 | 1200728 |
| 100000 | document_read | 1600160 | 1600160 | 1600160 |

### fixed-writers-powered-document2

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 0.1928 / 0.1962 | 0.1903 / 0.1946 | 0.1699 / 0.1725 |
| 100 | document_read | 0.0775 / 0.0784 | 0.0752 / 0.0763 | 0.0756 / 0.0769 |
| 100000 | document_write | 41.3475 / 41.7175 | 41.2354 / 41.7959 | 41.0861 / 41.5799 |
| 100000 | document_read | 47.0650 / 48.3205 | 46.8313 / 47.7700 | 47.1761 / 47.6645 |

Median allocated bytes per operation

| Rows | Operation | Current | A/A repeat | Production |
| ---: | --- | ---: | ---: | ---: |
| 100 | document_write | 1976 | 1976 | 1928 |
| 100 | document_read | 1760 | 1760 | 1760 |
| 100000 | document_write | 1200776 | 1200776 | 1200728 |
| 100000 | document_read | 1600160 | 1600160 | 1600160 |
