# Benchmarks

`cargo bench -p tafla-benchmarks` compares tafla's Rust runtime, `tafla`,
with other Rust crates that encode small tables and numeric columns, in
Criterion benchmarks. It runs the workloads of the TypeScript comparison in
`typescript/benchmarks/`, with the same values, so that the languages' numbers
compare. Run the commands below from `rust/`.

## Workloads

- **record**: one table of two float64s, an int64 and a 16-byte string.
- **document of 100 and 100,000 rows**: a sized, versioned table of two
  float64s and a string, with a nested table of an int64 and three columns, a
  uint32 and two float32.
- **vector of 100 nested tables**: a table holding a vector of tables, each a
  float32 and a nested table of two float32s.
- **fixed-size record**: a uint32 and two float32s at fixed offsets.

Each shape is serialized and deserialized into the types of `workloads.rs`,
whose strings and columns are `Cow`s: slices of the bytes where a crate reads
them in place as Rust's `str` and `[T]`, and copies otherwise. Column `a` of
each document is also summed through each crate's per-element accessor.
`workloads.rs` ports the TypeScript generator, so the inputs are the same
values, which `tests/workloads.rs` checks against values that the TypeScript
generator printed.

`cargo run -p tafla-benchmarks --bin encoding` prints each crate's encoded
sizes and what it does with a truncated document, as the TypeScript comparison
does.

### What differs from TypeScript

- There are no `number[]` inputs. Rust's columns are typed already, and a
  `Vec<f64>` that parsed JSON would hold isn't what a codec is given.
- Only tafla's columns are read as slices of the bytes. tafla's `ArrayView`
  returns one with `to_cow()` when the array is aligned on a little-endian
  host, as the TypeScript runtime's typed arrays view the bytes. The
  flatbuffers crate's `Vector` has no slice of its elements, unlike
  `aArray()` in TypeScript, and rkyv reads them in place as little-endian
  wrappers, `f32_le`, rather than `f32`s, so both copy their columns into
  `Vec`s. Summing a column shows them reading in place.
- tafla's Rust runtime has no `StructWriter` or `readUint32()`. tafla-gen
  generates a struct's writer with `to_le_bytes()` and its reader with
  `ByteBuffer`'s getters, which the tafla codec does for the fixed-size
  record, and tafla's README recommends the standard library's
  `to_le_bytes()` and `from_le_bytes()` for such records, which are compared
  too. tafla and `to_le_bytes` write the record with the same code, an A/A
  pair that shows the harness's noise.
- MessagePack's columns are arrays of float32s and uint32s, packed and
  unpacked element by element, as serde serializes a `[f32]`. msgpackr's
  typed-array extensions are msgpackr's own.
- rkyv, which has no counterpart in the other languages, is the closest Rust
  counterpart to tafla's reads: an archive that's read in place after it's
  validated.
- flatbuffers' latest Rust release is 25.12.19, where the TypeScript
  comparison uses flatbuffers 25.9.23.
- `yarn bench`'s benchmarks of each getter and setter aren't ported.
- Criterion doesn't interleave the codecs; see [Method](#method).

## Crates

| Crate                                  | Why it's comparable                                                                                                                     |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `tafla` (`core/`)                      | The reference that the others are compared with.                                                                                        |
| `flatbuffers` 25.12.19                 | Zero-copy tables with offsets and vectors, like tafla.                                                                                  |
| `prost` 0.14.4                         | The most used protobuf crate, with packed repeated floats.                                                                              |
| `rmp-serde` 1.3.1, `serde` 1.0.229     | MessagePack through serde: a schemaless format, whose tables are maps with their keys.                                                  |
| `rkyv` 0.8.18                          | Zero-copy archives of Rust types, read in place after bytecheck validates them, as tafla's reads check every range.                     |
| `to_le_bytes` (the standard library's) | For the fixed-size record only: fields at fixed offsets, which tafla's README recommends writing and reading with the standard library. |

Each crate is used as its documentation shows, and as the TypeScript
comparison uses its counterpart:

- **tafla** nests tables from their builders, which borrow the columns and
  write each array in bulk into a table of its exact size. Readers check the
  document's version, and read columns as `ArrayView`s, whose `to_cow()`
  borrows them. The vector's items are nested from builders, collected first
  as tafla-gen's code collects them, since creating a builder can fail.
- **FlatBuffers** builds each message with a new `FlatBufferBuilder`, which
  starts empty and doubles as it grows, and each table with flatc's
  `create()` and its arguments. Columns are written from slices with
  `create_vector()`, which writes them element by element. A message is the
  builder's buffer from where the builder finished it, taken with
  `collapse()` without a copy, as the TypeScript comparison takes
  `asUint8Array()`'s view. Readers verify the message with
  `flatbuffers::root()`, as flatc's `root_as_*()` functions do, and copy
  columns through their `Vector`'s iterator. The generated code in
  `src/codecs/flatbuffers/generated.rs` is written by hand in the shape that
  `flatc --rust` generates, with only what the benchmarks use, since the
  FlatBuffers project doesn't publish flatc on crates.io.
- **prost** builds messages from the input and writes them with
  `encode_to_vec()`, which sizes the bytes first, and reads them with
  `decode()`. Its messages own their strings and repeated fields, so building
  one from borrowed input copies its columns, as a caller that keeps its
  values would, and the decoded columns move into the output. The messages in
  `src/codecs/protobuf.rs` are derived with `#[derive(prost::Message)]` in the
  shape that prost-build generates, so no protoc is needed.
- **rmp-serde** packs the workloads' own types through their serde derives,
  with `to_vec_named()`, which packs a struct as a map with its field names,
  as msgpackr packs an object, and reads them with `from_slice()`, which
  borrows strings from the bytes.
- **rkyv** archives the workloads' own types through their derives, with
  `to_bytes()` into an `AlignedVec`, and reads them with `access()`, which
  validates the whole archive with bytecheck first, as untrusted bytes need.
  Numbers are read with `to_native()`, through which columns are copied into
  `Vec`s.
- **`to_le_bytes`** writes the fixed-size record into a 12-byte array with
  `to_le_bytes()`, and reads it with `from_le_bytes()` from the array of its
  first 12 bytes.

A field that FlatBuffers or protobuf bytes leave out reads as `None`, which
their codecs return as an error, since every workload has it.

## Method

Criterion 0.8.2 times each benchmark in a loop of its own, after a warm-up of
2 s, in 100 samples over about 4 s. Samples grow linearly, from about a
millisecond to 80 ms, so the fastest isn't one lucky call, nor limited by the
timer, which ticks every 42 ns on Apple silicon. The slowest benchmarks, which
take a millisecond per call, start from one call and take up to 7 s, as
Criterion warns. The `report` binary then prints each benchmark's fastest
sample, per call, and its ratio to tafla's, as the other languages'
comparisons report the fastest round, since load from other processes only
slows samples down. It prints the median sample too, which load moves away
from the fastest.

- Every codec is a type of its own, and each benchmark's loop is compiled for
  its codec, so nothing is shared between their loops; Rust has no JIT
  feedback to share.
- Inputs pass through `black_box()`, so that the compiler can't fold the
  constant record and fixed-size record into the code, and results are
  returned to Criterion, which passes them through `black_box()` and drops
  them within the timed loop, as a caller frees its bytes.
- Before timing, every benchmark checks that its codec reads back exactly what
  it wrote, with floats compared by their bits, and that it sums column `a` to
  the same bits. `cargo test` runs `tests/round_trip.rs`, which checks every
  codec on every workload.
- `cargo bench` builds with the workspace's release profile, with fat LTO and
  one codegen unit, so that calls inline across the crates.

Criterion doesn't interleave the codecs as the TypeScript harness does: it
runs one benchmark's samples after another's, so a burst of load can slow one
codec's samples and not another's. A benchmark's fastest sample is unaffected
unless the load lasts the whole 4 s. Check `uptime` before a run, and compare
the fastest samples with the medians, which the report summarizes.

The harness is Criterion rather than divan, although divan reports the
fastest sample itself: Criterion's samples last a millisecond or more, where
divan's last about 100 ticks of the timer, 4 µs on Apple silicon, and divan
keeps a sample's results alive until it ends, so that freeing them isn't
timed. Criterion's saved baselines also compare two versions of tafla, as a
performance change needs. It doesn't print the fastest sample, which the
report reads from the samples that it saves.

## Running

```sh
uptime
cargo bench -p tafla-benchmarks
cargo run -p tafla-benchmarks --bin report
```

The full run takes about 8 minutes. `-p tafla-benchmarks` keeps Cargo from
running the other crates' benchmark harnesses, which reject Criterion's
options. Criterion's options follow `--`: a regular expression
selects benchmarks, and `--save-baseline <name>` and `--baseline <name>`
compare two versions. `report --markdown` prints the table below.

```sh
cargo bench -p tafla-benchmarks -- 'document of 100,000 rows'
```

A quick run of 10 samples over 50 ms per benchmark, after a 50 ms warm-up,
checks that every benchmark works in about 9 seconds once it's built. CI runs
it after the round-trip test, with the report and the encoding report. Its
numbers mean little, and replace the last run's in the report.

```sh
cargo bench -p tafla-benchmarks -- --warm-up-time 0.05 --measurement-time 0.05 --sample-size 10 --nresamples 1000
```

`cargo test -p tafla-benchmarks --benches` runs every benchmark once, without
optimizations.

## Code checks

Clippy checks the crate with core's lints, all of `clippy::all`, `pedantic`
and `nursery`, and rustfmt formats it, with the workspace's other crates.
FlatBuffers' generated code keeps flatc's style, which flatc rather than this
crate would change, so its module allows the four lints that it raises: its
`_tab` and `_fbb` names, type names written in full rather than `Self` or
elided lifetimes, and tables that derive `PartialEq` without `Eq`. prost's
messages pass as they are.

The crate builds on Rust 1.88, the workspace's `rust-version`, so
`cargo +1.88 test` runs its tests too. Cargo's resolver picks versions of its
dependencies that support 1.88, which leaves the other crates' dependencies as
they were.

## Limitations

- Criterion doesn't interleave the codecs, as above.
- The Apple M2 Pro has 6 performance and 4 efficiency cores, and macOS can move
  the benchmark's thread to an efficiency core when others are busy.
- Only a little-endian host was timed. On a big-endian host, tafla's
  `to_cow()` copies its columns, swapping their bytes, and rkyv's archives
  are little-endian too.
- Only macOS's allocator was timed, which every codec's allocations depend on.
- FlatBuffers' builder starts empty and doubles by copying, and rmp-serde's and
  rkyv's buffers grow by doubling too. A `FlatBufferBuilder` with the
  message's capacity, or reused with `reset()`, would make writing cheaper,
  but the TypeScript comparison doesn't reuse its builder either.
- prost copies the input's columns into its message, which a caller that
  builds its values in prost's messages wouldn't.
- rmp-serde's `to_vec()` packs structs as arrays without their field names,
  which is smaller than `to_vec_named()`'s maps, but needs the reader to know
  the fields' order, as a schema would, so the other languages' MessagePack
  codecs pack maps.
- Summing floats one after another into an `f64` is bound by the latency of
  each addition, about 0.86 ns here, so tafla, FlatBuffers and rkyv sum the
  large document's column in the same time. The small document's 100
  additions take less than 100 latencies per call, since the processor
  overlaps a call's additions with the next call's.

## Results

One full run on 2026-11-01, on an Apple M2 Pro with 32 GB in macOS 27.0.1,
with rustc 1.99.1 and Criterion 0.8.2. It took 8 minutes. Other processes kept
the 1-minute load average between 1.3 and 1.9 on the 10 cores, with a median
of 1.5. The median sample was within 5% of the fastest for 59 of the 62
benchmarks, and tafla's read of the record within 7%. FlatBuffers' reads of
the record and of the fixed-size record changed speed in steps during the run:
the fixed-size record's first samples took 23 ns per call and the others 47 to
56 ns, which put its median 105% above its fastest sample, and the record's
took 42 to 90 ns, with a median 42% above. They did the same in an earlier
run under more load, with a 1-minute load average between 1.6 and 3.2, whose
fastest samples were all within 3.3% of these but rkyv's write of the record,
6.3% slower.

Fastest sample in µs per call, and its ratio to tafla's:

| Benchmark                                          |    tafla |    flatbuffers |          prost |       rmp-serde |             rkyv |      to_le_bytes |
| -------------------------------------------------- | -------: | -------------: | -------------: | --------------: | ---------------: | ---------------: |
| record: serialize                                  |   0.0785 |  0.277 (3.53×) | 0.0484 (0.62×) |   0.131 (1.67×) |   0.0620 (0.79×) |                  |
| record: deserialize                                |   0.0126 | 0.0415 (3.29×) | 0.0388 (3.08×) |  0.0454 (3.60×) |  0.00557 (0.44×) |                  |
| document of 100 rows: serialize                    |    0.164 |  0.591 (3.61×) |  0.565 (3.45×) |   1.227 (7.48×) |    0.191 (1.16×) |                  |
| document of 100 rows: deserialize                  |   0.0341 |  0.374 (11.0×) |  1.336 (39.2×) |   1.383 (40.6×) |   0.0842 (2.47×) |                  |
| document of 100 rows: sum a column per element     |   0.0356 |  0.166 (4.67×) |  1.360 (38.2×) |   1.428 (40.1×) |   0.0383 (1.08×) |                  |
| document of 100,000 rows: serialize                |     17.9 |  109.2 (6.11×) |  440.1 (24.6×) |   765.4 (42.8×) |     19.2 (1.07×) |                  |
| document of 100,000 rows: deserialize              |   0.0341 |  220.8 (6473×) | 757.5 (22203×) | 1249.3 (36620×) |      15.3 (448×) |                  |
| document of 100,000 rows: sum a column per element |     85.6 |   85.8 (1.00×) |  827.8 (9.67×) |  1341.9 (15.7×) |     85.6 (1.00×) |                  |
| vector of 100 nested tables: serialize             |    9.027 |  3.544 (0.39×) |  1.404 (0.16×) |   2.634 (0.29×) | 0.0382 (0.0042×) |                  |
| vector of 100 nested tables: deserialize           |    0.302 |  5.011 (16.6×) |  2.439 (8.07×) |   4.184 (13.8×) |   0.0306 (0.10×) |                  |
| fixed-size record: serialize                       | 0.000709 |   0.167 (236×) | 0.0232 (32.7×) |   0.0807 (114×) |   0.0198 (27.9×) | 0.000710 (1.00×) |
| fixed-size record: deserialize                     |  0.00117 | 0.0232 (19.7×) | 0.0107 (9.12×) |  0.0296 (25.2×) | 0.000992 (0.85×) | 0.000665 (0.57×) |

- tafla reads a document in 34 ns, at 100 rows and at 100,000 alike, since its
  columns are slices of the bytes. rkyv reads in place too, but copies its
  columns into `Vec`s, in 15 µs for 100,000 rows. FlatBuffers copies them
  through its iterator in 221 µs, and prost and rmp-serde parse every
  element, in 0.76 and 1.25 ms.
- tafla writes the large document in 18 µs and rkyv in 19 µs, both copying the
  columns whole. FlatBuffers, prost and rmp-serde write them element by
  element, 6 to 43 times slower. Copying the columns into prost's message took
  about 16 µs of its 440 µs, in a separate measurement.
- Summing the 100,000 floats takes 86 µs through tafla's, FlatBuffers' and
  rkyv's accessors alike, bound by the additions. Summing 100 takes tafla
  36 ns, rkyv 38 ns and FlatBuffers 166 ns. FlatBuffers' extra 130 ns doesn't
  grow with the rows, which suggests that it's opening the document, which
  `flatbuffers::root()` verifies. prost and rmp-serde decode the whole
  document first.
- rkyv reads the record in 5.6 ns, at fixed offsets once it's validated,
  against tafla's 12.6 ns and the others' 39 to 45 ns. prost writes it
  fastest, in 48 ns, then rkyv in 62 ns and tafla in 79 ns.
- The vector of 100 nested tables is the shape that tafla handles worst, as in
  the other languages: 9.0 µs to write, against prost's 1.4 µs, rmp-serde's
  2.6 µs and FlatBuffers' 3.5 µs, and each of its 200 tables is a builder with
  an allocation of its own. rkyv, whose items are 12 bytes each, inline,
  writes the vector in 38 ns and reads it in 31 ns, against tafla's 0.30 µs.
- tafla and `to_le_bytes` write the fixed-size record in 0.71 ns alike, with
  the same code. Reading it takes `from_le_bytes()` 0.67 ns, rkyv 0.99 ns and
  tafla's `ByteBuffer` getters 1.17 ns, which check each read's range. The
  other crates allocate a `Vec` for each record, and take 20 to 167 ns to
  write it and 11 to 30 ns to read it.

`cargo run -p tafla-benchmarks --bin encoding` printed:

```
Encoded size (bytes)
                        record     doc 100  doc 100000      vector       fixed
  tafla                     60        1296     1200096        3208          12
  flatbuffers               72        1328     1200128        2832          32
  prost                     47        1243     1187503        1900          16
  rmp-serde                 53        1573     1499239        3510          24
  rkyv                      48        1272     1200072        1208          12
  to_le_bytes                                                               12

Truncated document of 100000 rows: deserialize()
  tafla             600048 B  returned an error: Read exceeds the supplied buffer: 400000 bytes at 400092 of
  tafla                 24 B  returned an error: Read exceeds the supplied buffer: 8 bytes at 24 of 24
  flatbuffers       600064 B  returned an error: Range [800124, 800128) is out of bounds.
  flatbuffers           24 B  returned an error: Range [32, 40) is out of bounds.
  prost             593751 B  returned an error: failed to decode Protobuf message: Document.columns: buffer
  prost                 24 B  returned an error: failed to decode Protobuf message: Document.name: buffer und
  rmp-serde         749619 B  returned an error: IO error while reading data: failed to fill whole buffer
  rmp-serde             24 B  returned an error: IO error while reading data: unexpected end of file
  rkyv              600036 B  returned an error: unaligned pointer: ptr 0x0000007682bbe7ac unaligned for alig
  rkyv                  24 B  returned an error: subtree pointer overran range: ptr 0x0000000102ca12c0 size 5
```

Every crate rejects the truncated document with an error, and none panics.
rkyv finds an archive's root at the end of its bytes, so a truncated archive
has another root. Its errors describe the failure only in builds with debug
assertions, as this one is; in release builds, they say only that it failed.
The sizes of tafla, FlatBuffers and protobuf are the other languages', and
rmp-serde's are msgpack-core's in Java, which packs floats as float32s too.
