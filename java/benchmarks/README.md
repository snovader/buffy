# Benchmarks

`./gradlew :benchmarks:jmh` compares buffy's Java runtime with other Java
libraries that encode small tables and numeric columns, in JMH benchmarks. It
runs the workloads of the TypeScript comparison in `typescript/benchmarks/`,
with the same values, so that the two languages' numbers compare. Run the
commands below from `java/`.

## Workloads

- **record**: one table of two float64s, an int64 and a 16-byte string.
- **document of 100 and 100,000 rows**: a sized, versioned table of two
  float64s and a string, with a nested table of an int64 and three columns, a
  uint32 and two float32.
- **vector of 100 nested tables**: a table holding a vector of tables, each a
  float32 and a nested table of two float32s.
- **fixed-size record**: a uint32 and two float32s at fixed offsets.

Each shape is serialized and deserialized, with columns read back as Java
arrays: the uint32 keys as a `long[]`, as buffy reads them, and the float32s
as `float[]`s. Column `a` of each document is also summed through each
library's per-element accessor. `Workloads` ports the TypeScript generator, so
the inputs are the same values, which `WorkloadsTest` checks against values
that the TypeScript generator printed.

The benchmark classes are named after the TypeScript code's workloads:
`RecordBenchmark`, `DocumentBenchmark`, `ListBenchmark` for the vector of
nested tables and `SampleBenchmark` for the fixed-size record.
`./gradlew :benchmarks:encodingReport` prints each library's encoded sizes and
what it does with a truncated document, as the TypeScript comparison does.

### What differs from TypeScript

- There are no `number[]` inputs. Java's arrays are typed, and its closest
  counterpart, a `List<Float>` of boxed values, isn't what a codec is given.
- Java's buffy has no `StructWriter`, so the fixed-size record is written with
  `ByteBuffer`'s setters into `ByteBuffer.allocate(12)` and read with its
  getters at fixed positions. buffy's package documentation suggests
  `MethodHandles.byteArrayViewVarHandle` for such records, which isn't buffy,
  so it isn't timed.
- msgpack-core has no typed arrays, and msgpackr's extension that copies their
  bytes is msgpackr's own, so MessagePack's columns are arrays of float32s and
  uint32s, packed and unpacked element by element.
- SBE, which has no JavaScript runtime here, encodes only the fixed-size
  record, the kind of message it's designed for.
- flatbuffers-java's latest release is 25.2.10, where the TypeScript
  comparison uses flatbuffers 25.9.23.
- `yarn bench`'s benchmarks of each getter and setter aren't ported.

## Libraries

| Library                                                       | Why it's comparable                                                                                                                                            |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| buffy (`:core`)                                               | The reference that the others are compared with.                                                                                                               |
| `com.google.flatbuffers:flatbuffers-java` 25.2.10             | Zero-copy tables with offsets and vectors, like buffy.                                                                                                         |
| `com.google.protobuf:protobuf-java` 4.36.2                    | The most used schema-based codec, with packed repeated floats.                                                                                                 |
| `org.msgpack:msgpack-core` 0.9.12                             | MessagePack's Java implementation: a schemaless format, whose tables are maps with their keys.                                                                 |
| `uk.co.real-logic:sbe-tool` 1.41.0, `org.agrona:agrona` 2.7.0 | Simple Binary Encoding, for the fixed-size record only: fields at fixed offsets, written and read in place through flyweights, like buffy's fixed-size record. |

Each library is used as its documentation shows, and as the TypeScript
comparison uses its counterpart:

- **buffy** nests tables from their builders. Arrays of the elements' own Java
  type are copied in bulk, and the uint32 keys are narrowed and widened element
  by element.
- **FlatBuffers** builds each message with a new `FlatBufferBuilder` of the
  default size, and creates vectors element by element with the generated
  methods. Float columns are copied out of `aAsByteBuffer()`'s little-endian
  view, and keys are widened through an `IntVector`. Its classes in
  `generated/flatbuffers/` are written by hand in the shape that
  `flatc --java` generates from `src/main/flatbuffers/workloads.fbs`, with only
  what the benchmarks use, since flatc isn't something Gradle can download.
- **protobuf** builds messages with their builders and parses them with their
  parsers. Repeated fields are added and read one element at a time, which
  keeps them unboxed, and parsed columns are copied into arrays. The protobuf
  Gradle plugin downloads protoc and generates the messages from
  `src/main/proto/messages.proto`.
- **MessagePack** packs a table as a map with its keys, and the reader matches
  the keys in whatever order they come. One packer and one unpacker serve every
  message, as msgpackr reuses its buffer: the packer is cleared before each
  message and `toByteArray()` copies the message out, and the unpacker is reset
  to each message's bytes. A new unpacker allocates a 16 KiB string decoder for
  its first string, which made reading the record 6 times slower in a quick
  run.
- **SBE** writes an 8-byte message header and the 12-byte record into a new
  array, and the decoder checks the header's template before reading. Its
  encoder and decoder flyweights are reused, as SBE intends. `generateSbe` runs
  sbe-tool on `src/main/sbe/sample.xml`.

## Method

JMH 1.37 times each benchmark in average-time mode. Every codec of every
benchmark runs in 3 forked JVMs, each of 5 warm-up and 5 measured iterations
of 1 s, with a fixed 2 GiB heap.

- A fork runs one codec, so the JIT compiles its call sites without the other
  codecs' type profiles, as the TypeScript harness's separate timer loops
  ensure.
- Inputs are read from the benchmark's state rather than constants, so that
  the JIT can't fold them into the code. Results are returned to JMH, which
  consumes them, so that the JIT can't eliminate the work.
- Before timing, every benchmark checks that its codec reads back exactly what
  it wrote. `./gradlew build` runs `RoundTripTest`, which checks every codec on
  every workload.
- After JMH's own summary, the mean with its 99.9% confidence interval, the
  `jmh` task prints each benchmark's fastest iteration and its ratio to
  buffy's, as the TypeScript comparison reports the fastest round, since load
  from other processes only slows iterations. JMH's results, with every
  iteration, are saved in `build/results/jmh/results.json`.

Unlike the TypeScript harness, JMH doesn't interleave the variants: it runs
one fork after another, so a burst of load can slow one codec's forks and not
another's. Three forks spread each codec over about half a minute, which makes
that less likely, and a fork's fastest iteration is unaffected unless the load
lasts the whole fork. Check `uptime` before a run, and compare the fastest
iterations with JMH's means.

## Running

```sh
uptime
./gradlew :benchmarks:jmh
```

The full run takes about 26 minutes. `--jmhArgs` passes JMH's own options,
which override those in `build.gradle.kts`: a regular expression selects
benchmarks, `-p` sets a parameter, and `-prof gc` adds the bytes allocated per
operation.

```sh
./gradlew :benchmarks:jmh --jmhArgs="Document -p codec=buffy"
```

A quick run of one fork, with one warm-up and one measured iteration of
100 ms, checks that every benchmark works in about 20 seconds. CI runs it after
the round-trip test, on Java 25. Its numbers mean little.

```sh
./gradlew :benchmarks:jmh --jmhArgs="-f 1 -wi 1 -w 100ms -i 1 -r 100ms"
```

## Code checks

The benchmarks are checked with core's Error Prone and NullAway settings,
except for:

- The `generated` packages: protoc's messages, sbe-tool's encoder and decoder,
  and the FlatBuffers classes in flatc's shape. Their style is their
  generators', and a generator's update can change it: protoc's messages raise
  51 Error Prone warnings, such as reference equality and unused variables,
  and flatc's shape raises 5, for its parenthesized returns and a class named
  `Record`, which `-Werror` would fail on. sbe-tool's code passes, but is
  excluded for the same reason. None of it is null-marked, so NullAway skips it
  too.
- JMH's generated harness, which is compiled without Error Prone.

NullAway is told that JMH sets `@Param` fields and calls `@Setup` methods
after constructing a benchmark's state.

## Limitations

- JMH doesn't interleave the codecs, as above.
- The Apple M2 Pro has 6 performance and 4 efficiency cores, and macOS can move
  the benchmark's thread to an efficiency core when others are busy.
- Only JDK 25 was timed, though buffy compiles for Java 21.
- msgpack-core uses `sun.misc.Unsafe`'s memory access, which JDK 24 and later
  warn about, and a later JDK will remove. The JMH forks allow it, so that the
  warning doesn't interrupt the output, and the tests print it.
- Agrona 2 uses the JDK's internal `Unsafe`, so the tests and JMH forks run
  with `--add-exports=java.base/jdk.internal.misc=ALL-UNNAMED`.
- FlatBuffers' builder starts at 1 KiB and doubles by copying, and protobuf's
  builders grow repeated fields as they're added, without a way to reserve
  their length. Reusing a `FlatBufferBuilder` with `clear()` would make small
  messages cheaper, but the TypeScript comparison doesn't reuse it either.
- Summing 100,000 floats one after another into a `double` is bound by the
  latency of each addition, about 0.86 ns here, so buffy's and FlatBuffers'
  accessors sum the large document in the same time, though not the small one.

## Results

One full run on 2026-11-01, on an Apple M2 Pro with 32 GB in macOS 27.0.1,
with Oracle's JDK 25.0.1 and JMH 1.37. It took 26 minutes. Other processes
kept the 1-minute load average between 1.3 and 3.8 on the 10 cores, with a
median of 2.2. JMH's mean was within 5% of the fastest iteration for 41 of the
50 benchmarks, and at most 39% above it, where one of protobuf's three forks
read the fixed-size record in 0.0135 µs and the others in 0.018–0.030 µs.

Fastest iteration in µs per operation, and its ratio to buffy's:

| Benchmark                                          | buffy  | flatbuffers    | protobuf        | msgpack         | sbe            |
| -------------------------------------------------- | -----: | -------------: | --------------: | --------------: | -------------: |
| record: serialize                                  | 0.0478 |  0.146 (3.05×) |  0.0297 (0.62×) |  0.0819 (1.71×) |                |
| record: deserialize                                | 0.0140 | 0.0195 (1.39×) |  0.0322 (2.29×) |   0.131 (9.32×) |                |
| document of 100 rows: serialize                    |  0.177 |  0.816 (4.62×) |   1.268 (7.18×) |   0.829 (4.69×) |                |
| document of 100 rows: deserialize                  | 0.0758 | 0.0797 (1.05×) |  0.816 (10.77×) |  1.457 (19.22×) |                |
| document of 100 rows: sum a column per element     | 0.0417 | 0.0555 (1.33×) |  0.826 (19.79×) |  0.730 (17.49×) |                |
| document of 100,000 rows: serialize                |   41.8 | 792.4 (18.94×) | 1757.8 (42.02×) |  695.6 (16.63×) |                |
| document of 100,000 rows: deserialize              |   48.7 |   49.5 (1.02×) | 1007.7 (20.70×) |  971.6 (19.95×) |                |
| document of 100,000 rows: sum a column per element |   86.2 |   86.2 (1.00×) | 1026.2 (11.91×) |   614.9 (7.14×) |                |
| vector of 100 nested tables: serialize             |  6.609 |  4.882 (0.74×) |   1.663 (0.25×) |   3.403 (0.51×) |                |
| vector of 100 nested tables: deserialize           |  0.566 |  0.870 (1.54×) |   3.535 (6.25×) |  7.377 (13.03×) |                |
| fixed-size record: serialize                       | 0.0021 | 0.129 (61.33×) |  0.0049 (2.31×) | 0.0759 (36.01×) | 0.0063 (2.97×) |
| fixed-size record: deserialize                     | 0.0025 | 0.0035 (1.42×) |  0.0135 (5.49×) | 0.0908 (37.04×) | 0.0059 (2.41×) |

- buffy reads every shape fastest. FlatBuffers reads documents within 5% of
  it, since both copy the float columns in bulk and widen the keys one by one.
- buffy writes documents 4.6 to 42 times faster than the others, whose
  columns are written element by element.
- The vector of 100 nested tables is the one shape that buffy writes slowest:
  6.6 µs, against protobuf's 1.7 µs. protobuf also writes the record faster.
- The sums of 100,000 floats take 86 µs through buffy's and FlatBuffers'
  accessors alike, bound by the additions; the 100-row sums differ.
- The fixed-size record takes 2 to 3 ns with buffy's `ByteBuffer`, and 6 ns
  with SBE, which writes and checks its 8-byte header too.

An iteration can also run faster than the rest when fewer garbage collections
fall into it. msgpack's fastest read of the large document took 972 µs,
against 1,030 to 1,107 µs for its other 14 iterations.

`./gradlew :benchmarks:encodingReport` printed:

```
Encoded size (bytes)
                        record     doc 100  doc 100000      vector       fixed
  buffy                     60        1296     1200096        3208          12
  flatbuffers               72        1328     1200128        2832          32
  protobuf                  47        1243     1187503        1900          16
  msgpack                   53        1573     1499239        3510          24
  sbe                                                                       20

Truncated document of 100000 rows: deserialize()
  buffy             600048 B  threw DecodeException: Read exceeds the supplied buffer: 400000 bytes at
  buffy                 24 B  threw DecodeException: Read exceeds the supplied buffer: 8 bytes at 24 of
  flatbuffers       600064 B  threw IndexOutOfBoundsException
  flatbuffers           24 B  threw IndexOutOfBoundsException
  protobuf          593751 B  threw InvalidProtocolBufferException: While parsing a protocol message, the input ended
  protobuf              24 B  threw InvalidProtocolBufferException: While parsing a protocol message, the input ended
  msgpack           749619 B  threw MessageInsufficientBufferException
  msgpack               24 B  threw MessageInsufficientBufferException
```

Every library rejects the truncated document. FlatBuffers' Java runtime reads
through a `java.nio.ByteBuffer`, whose range checks throw.

## Experiments

[uint32 conversion](experiments/uint32/README.md) compares the current runtime,
a simpler scalar loop and explicit SIMD on both document sizes and their
uint32 arrays. It includes an interleaved harness, A/A controls, allocation
measurements and raw results. The chosen scalar setter is now in production.

The [unsigned-setter follow-up](experiments/uint-setters/README.md) measures
the indexed loop for uint8, uint16 and uint32, both directly and through
`TableBuilder`. The runtime uses indexed loops for uint16 and uint32 and a
local cursor for uint8. The [uint8 allocation investigation](experiments/uint-setters/allocation.md)
identifies a validation-induced JIT effect in the earlier experiments and
records corrected uint8 comparisons: the cursor retains the table-write gain
without extra allocation in direct writes.

The [corrected uint16/uint32 and document comparison](experiments/uint-setters/isolated-wide.md)
reruns both indexed setters and the actual production document codec with
validation in a separate JVM. It includes fresh A/A controls, repeated runs,
allocation measurements and unchanged-reader checks.

The [table-writer allocation investigation](experiments/uint-setters/table-allocation.md)
traces the remaining 24 bytes to `ArrayList` escape analysis and replaces the
writer list with a fixed-size array. Powered repeats confirm the table-write
gain, unchanged large-document write speed, faster small-document writes,
and reduced allocation. Earlier timings under load and low battery are
retained in the report but excluded from its timing conclusions.
