# Benchmarks

`python -m tafla_benchmarks` compares `tafla` with other Python libraries
that encode small tables and numeric columns, in pyperf benchmarks. It runs the
workloads of the TypeScript comparison in `typescript/benchmarks/`, with the
same values, so that the languages' numbers compare. Run the commands below
from `python/`.

## Workloads

- **record**: one table of two float64s, an int64 and a 16-byte string.
- **document of 100 and 100,000 rows**: a sized, versioned table of two
  float64s and a string, with a nested table of an int64 and three columns, a
  uint32 and two float32.
- **vector of 100 nested tables**: a table holding a vector of tables, each a
  float32 and a nested table of two float32s.
- **fixed-size record**: a uint32 and two float32s at fixed offsets.

Each shape is serialized and deserialized, with columns read back as each
library returns them: tafla's as memoryviews of the bytes, FlatBuffers' as
numpy arrays that view them, protobuf's as the message's repeated fields, and
MessagePack's as memoryviews of the bytes that it unpacks. The documents'
columns are `array`s, and documents are also serialized from lists, as parsed
JSON would have them. Column `a` of each document is summed through each
library's per-element accessor. `workloads.py` ports the TypeScript generator,
so the inputs are the same values, which `tests/test_workloads.py` checks
against values that the TypeScript generator printed.

`python -m tafla_benchmarks.encoding` prints each library's encoded sizes and
what it does with a truncated document, as the TypeScript comparison does.

### What differs from TypeScript

- Columns are read back as each library returns them, rather than converted to
  one type. Python has no typed array that every library returns, as
  JavaScript's typed arrays are, and converting would time the conversion
  rather than the library.
- Lists stand in for `number[]`: the groups that serialize documents from
  `number[]` serialize them from lists here.
- tafla's Python runtime has no `StructWriter` or `readUint32()`, so the
  fixed-size record is written with `ByteBuffer`'s setters into a new 12-byte
  `bytearray` and read with its getters at fixed positions, as the Java
  benchmarks do. tafla's documentation recommends the standard library's
  `struct` for such records, which is compared too.
- FlatBuffers' Python runtime copies vectors in bulk only from and to numpy
  arrays, so its columns are written and read through numpy. Without numpy, it
  has only a call per element.
- msgpack's columns are extension types of their arrays' bytes, which this
  suite defines, as msgpackr's typed arrays are msgpackr's own.
- flatbuffers' latest Python release is 25.12.19, where the TypeScript
  comparison uses flatbuffers 25.9.23.
- `yarn bench`'s benchmarks of each getter and setter aren't ported.
- The variants run in worker processes of their own rather than interleaved
  in one process; see [Method](#method).

## Libraries

| Library                                    | Why it's comparable                                                                                                                        |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `tafla` (`core/`)                          | The reference that the others are compared with.                                                                                           |
| `flatbuffers` 25.12.19, with `numpy` 2.5.4 | Zero-copy tables with offsets and vectors, like tafla.                                                                                     |
| `protobuf` 7.36.2                          | The most used schema-based codec, with packed repeated floats. Its wheels parse and serialize in C, with upb.                              |
| `msgpack` 1.2.4                            | MessagePack's Python implementation, with a C extension: a schemaless format, whose tables are maps with their keys.                       |
| `struct` (the standard library's)          | For the fixed-size record only: fields at fixed offsets, packed and unpacked in one call, which tafla's documentation recommends for them. |

Each library is used as its documentation shows, and as the TypeScript
comparison uses its counterpart:

- **tafla** nests tables from their builders and reads columns as views of the
  bytes. Arrays of the columns' element types are copied as bytes, and lists
  are converted as `array()` converts them. Summing a column indexes its view,
  as tafla's documentation advises over a getter call per element.
- **FlatBuffers** builds each message with a new `Builder` of the default
  size, adding fields one by one with the generated functions, and reads them
  with the generated accessors. Columns are written with
  `CreateNumpyVector()`, from numpy arrays that view the `array`s or convert
  the lists, and read with the generated `AsNumpy()` accessors, which view the
  bytes. The modules in `generated/flatbuffers/` are written by hand in the
  shape that `flatc --python` generates from `generated/workloads.fbs`, with
  only what the benchmarks use, since the FlatBuffers project doesn't publish
  flatc on PyPI, where uv installs tools from. As in flatc's, an accessor of a
  nested table imports the table's class on every call.
- **protobuf** builds messages from keyword arguments and parses them with
  `FromString()`. Nested messages are built in place, as protobuf's tutorial
  builds them: passing a nested message to its parent's constructor copies it,
  which made writing the vector of nested tables 2.6 times slower and the
  large document 13% slower. Repeated fields are extended from the columns,
  which converts them element by element.
- **MessagePack** packs a table as a map with its keys, and a document as one
  map without a nested one for its columns, as msgpackr packs them. Floats pack
  as float64s, as msgpack packs Python's floats and msgpackr packs numbers. One
  `Packer` serves every message, as msgpackr reuses its packer, and
  `unpackb()` reads each. Columns pack as extensions of their arrays' bytes,
  as msgpack's documentation packs an `array`, and lists are converted to
  arrays first. The extension hook returns a memoryview of the bytes that the
  unpacker copied out, which took half the time of copying them again into an
  `array` for 100,000 rows.
- **struct** packs and unpacks the fixed-size record with one precompiled
  `Struct("<Iff")`.

## Method

pyperf 2.10.0 runs each benchmark in worker processes of its own. The first
calibrates the number of calls that one value times, so that it takes at least
100 ms, and each of the others runs a warm-up and then 3 values. A full run
runs the suite 4 times, with 5 workers per benchmark, and appends their values
to one file: 20 workers and 60 values per benchmark. The runs alternate the
order of the codecs in each group, so that each benchmark's workers are spread
over the whole run. `tafla_benchmarks.report` then prints each benchmark's
fastest value, and its ratio to tafla's, as the TypeScript comparison reports
the fastest round, since load from other processes only slows values down.

- A worker runs one benchmark, so CPython's specializing interpreter
  specializes its timing loop for one operation, as the TypeScript harness's
  separate timer loops ensure for V8. CPython doesn't eliminate unused results,
  so the loop drops each result, which frees it before the next call, as a
  caller's would be.
- A worker prepares its benchmark's input when it first times it, after
  checking that the codec reads back exactly what it wrote, with floats
  compared by their bits. `tests/test_round_trips.py` checks every codec on
  every workload.
- Every deserialization creates the same dataclasses from the values it read,
  from positional arguments, which took 65 ns for the fixed-size record,
  against 190 ns from keywords.
- The garbage collector stays enabled, as it is for callers.

pyperf doesn't interleave the codecs as the TypeScript harness does: it runs
one benchmark's workers after another's. Within a run, a burst of load can slow
one codec's workers and not another's, but the 4 runs spread each benchmark
over the whole run, and its fastest value is unaffected unless the load lasts
for all of them. Check `uptime` before a run, and compare the fastest values
with pyperf's means, which the report summarizes.

## Running

```sh
uptime
for order in "" --reverse "" --reverse; do
    uv run --python 3.14+gil python -m tafla_benchmarks --processes 5 --append results.json $order
done
uv run --python 3.14+gil python -m tafla_benchmarks.report results.json
```

The full run takes about 18 minutes. `--append` adds to an existing file, so
remove the last run's `results.json` first. `--filter <regex>` runs only the
groups whose name matches, and `--reverse` runs each group's codecs in reverse
order. pyperf's own options apply too, and its commands read the results:
`python -m pyperf stats results.json` describes each benchmark's values, for
example.

A quick run of one worker per benchmark, which calls each operation once,
checks that every benchmark works in about 10 seconds, most of which its 58
workers spend starting and importing numpy. CI runs it after the round-trip
test, on Python 3.14. Its numbers mean little.

```sh
uv run python -m tafla_benchmarks --debug-single-value --quiet
```

## Generated code

`generated/messages_pb2.py` and its stub `messages_pb2.pyi` are generated from
`generated/messages.proto` by the protoc of grpcio-tools 1.85.1, protoc 35.1,
which the `protoc` dependency group installs:

```sh
uv run --package tafla-benchmarks --group protoc python -m grpc_tools.protoc -I benchmarks/src --python_out=benchmarks/src --pyi_out=benchmarks/src benchmarks/src/tafla_benchmarks/generated/messages.proto
```

They're checked in, so that the tests and checks run without protoc, although
protoc marks its output as not meant to be: protobuf's runtime accepts
generated code of its own major version that isn't newer than it, so they need
regenerating when protobuf's major version changes.

## Code checks

Ruff and ty check the benchmarks with the workspace's settings, except for
generated code:

- Ruff skips protoc's `messages_pb2.py` and `messages_pb2.pyi`, which are its
  verbatim output, and ty skips the stub, which leaves `Mapping`'s type
  arguments out.
- The FlatBuffers modules keep the shape of flatc's code, which Ruff formats:
  modules named after their tables, classes that derive from `object` and no
  annotations, which Ruff's rules for them ignore. ty ignores their use of the
  `flatbuffers` package's submodules, which the package imports itself.

## Limitations

- pyperf doesn't interleave the codecs, as above.
- The Apple M2 Pro has 6 performance and 4 efficiency cores, and macOS can move
  a worker to an efficiency core when others are busy.
- Only CPython 3.14's build with the GIL was timed. CI runs the tests on 3.14
  and 3.15, and uv picks free-threaded builds on some machines, which these
  numbers don't describe.
- protobuf's wheels use upb. Its pure-Python implementation, which
  `PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION=python` selects, would be much slower,
  and isn't timed.
- msgpack's column extensions are in the host's byte order, as msgpackr's are,
  and are this suite's own, so another MessagePack reader would see them as
  unknown extensions.
- FlatBuffers' builder starts at 1 KiB and doubles by copying. Reusing a
  `Builder` with `Clear()` would make small messages cheaper, but the
  TypeScript comparison doesn't reuse it either.

## Results

One full run on 2026-11-01, on an Apple M2 Pro with 32 GB in macOS 27.0.1,
with Homebrew's CPython 3.14.10, built with the GIL, PGO, LTO and the
tail-calling interpreter, without the JIT. It took 18 minutes. Other processes
kept the 1-minute load average between 1.1 and 2.9 on the 10 cores, with a
median of 1.4. pyperf's mean was within 5% of the fastest value for 53 of the
58 benchmarks. The others serialize documents: for tafla's, msgpack's and
FlatBuffers' large documents, and msgpack's small one from lists, the medians
were 9% to 14% above the fastest values, as the means were, so most values were
slower than the fastest, rather than a few of them slowed by load. msgpack's
small document had a few slow values, which put its mean 8% above its fastest
value and its median 1%.

Fastest value in µs per call, and its ratio to tafla's:

| Benchmark                                          |  tafla |       flatbuffers |         protobuf |        msgpack |        struct |
| -------------------------------------------------- | -----: | ----------------: | ---------------: | -------------: | ------------: |
| record: serialize                                  |  2.444 |     8.299 (3.40×) |    0.497 (0.20×) |  0.284 (0.12×) |               |
| record: deserialize                                |  1.225 |     4.771 (3.89×) |    0.436 (0.36×) |  0.393 (0.32×) |               |
| document of 100 rows: serialize                    |  7.316 |      20.5 (2.80×) |    8.571 (1.17×) |  1.847 (0.25×) |               |
| document of 100 rows: deserialize                  |  2.646 |      12.7 (4.79×) |    1.721 (0.65×) |  1.175 (0.44×) |               |
| document of 100 rows: sum a column per element     |  3.536 |    142.9 (40.40×) |    4.791 (1.35×) |  3.709 (1.05×) |               |
| document of 100,000 rows: serialize                |   31.7 |     176.9 (5.57×) | 7577.7 (238.75×) |   59.7 (1.88×) |               |
| document of 100,000 rows: deserialize              |  2.660 |      12.7 (4.79×) |  618.3 (232.48×) |   20.3 (7.64×) |               |
| document of 100,000 rows: sum a column per element | 3056.8 | 140796.6 (46.06×) |   5051.1 (1.65×) | 3091.2 (1.01×) |               |
| vector of 100 nested tables: serialize             |  259.7 |     527.9 (2.03×) |     41.3 (0.16×) |   20.8 (0.08×) |               |
| vector of 100 nested tables: deserialize           |   93.3 |     636.5 (6.82×) |     39.3 (0.42×) |   30.2 (0.32×) |               |
| fixed-size record: serialize                       |  0.637 |    6.520 (10.24×) |    0.453 (0.71×) |  0.226 (0.36×) | 0.074 (0.12×) |
| fixed-size record: deserialize                     |  0.636 |     3.515 (5.53×) |    0.385 (0.61×) |  0.318 (0.50×) | 0.142 (0.22×) |
| document of 100 rows: serialize lists              |   11.1 |      25.2 (2.27×) |    5.625 (0.51×) |  4.868 (0.44×) |               |
| document of 100,000 rows: serialize lists          | 2960.5 |    4908.1 (1.66×) |   4309.9 (1.46×) | 2990.8 (1.01×) |               |

- tafla reads both documents in 2.7 µs, since its columns are views, whatever
  their length. FlatBuffers' numpy views take 12.7 µs at both sizes, so its
  pure-Python accessors' calls cost that, not the columns. msgpack and
  protobuf read the small document faster, in C, but copy the large one's
  columns: msgpack in 20 µs, and protobuf, which parses them into its
  containers, in 618 µs.
- tafla writes the large document fastest, in 32 µs, copying its arrays as
  bytes; msgpack copies them twice, into extensions and then into the message.
  protobuf converts every element, which took 7.6 ms from arrays and 4.3 ms
  from lists, whose elements are Python floats already.
- Small tables cost tafla its Python calls, a few per field, as its
  documentation says: protobuf and msgpack, in C, write the record 5 to 9 times
  faster and read it about 3 times faster. Only FlatBuffers, also pure Python,
  is slower.
- The vector of 100 nested tables is where tafla is furthest behind: 260 µs to
  write, against msgpack's 21 µs and protobuf's 41 µs, and 93 µs to read,
  against 30 µs and 39 µs.
- Summing a column through a memoryview takes about 31 ns per element, for
  tafla and msgpack alike, and through protobuf's container 51 ns.
  FlatBuffers' generated accessor takes 1.4 µs per element.
- Converting lists dominates writing the large document from them, which took
  every library 3 to 5 ms.
- `struct` packs the fixed-size record in 74 ns and unpacks it in 142 ns, about
  half of which is creating the result. tafla's `ByteBuffer` takes 637 ns and
  636 ns.

Every deserialization includes creating the workload's dataclasses, about
65 ns each, which the codecs share: 6.5 µs of reading the vector of 100 nested
tables, for example.

`python -m tafla_benchmarks.encoding` printed:

```
Encoded size (bytes)
                        record     doc 100  doc 100000      vector       fixed
  tafla                     60        1296     1200096        3208          12
  flatbuffers               72        1328     1200128        2832          32
  protobuf                  47        1243     1187503        1900          16
  msgpack                   53        1276     1200082        4710          32
  struct                                                                    12

Truncated document of 100000 rows: deserialize()
  tafla             600048 B  raised tafla.errors.DecodeError: Read exceeds the supplied buffer: 400000 bytes at
  tafla                 24 B  raised tafla.errors.DecodeError: Read exceeds the supplied buffer: 8 bytes at 24 of
  flatbuffers       600064 B  raised struct.error: unpack_from requires a buffer of at least 800128 b
  flatbuffers           24 B  raised struct.error: unpack_from requires a buffer of at least 28 bytes
  protobuf          593751 B  raised google.protobuf.message.DecodeError: Error parsing message with type 'tafla.benchmarks.
  protobuf              24 B  raised google.protobuf.message.DecodeError: Error parsing message with type 'tafla.benchmarks.
  msgpack           600041 B  raised ValueError: Unpack failed: incomplete input
  msgpack               24 B  raised ValueError: Unpack failed: incomplete input
```

Every library rejects the truncated document. FlatBuffers' Python runtime reads
through `struct`, whose range checks raise `struct.error`. msgpack packs the
floats of the vector and the fixed-size record as float64s, so its vector is
the largest.
