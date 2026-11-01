# buffy-core

Binary tables of scalars, strings and arrays, in
[buffy](https://github.com/snovader/buffy)'s format.
`TableBuilder` writes a table at its exact size, copying arrays in bulk,
and `ByteBuffer` reads fields back by offset, borrowing arrays and strings
from the bytes. Every read checks that it stays within the bytes it was
given.

```rust
use buffy_core::{ByteBuffer, DecodeError, TableBuilder, TableLayout};

// A format's header, shared by its writer and its readers
const LAYOUT: TableLayout<true> = TableLayout::DEFAULT.sized().versioned();
const FORMAT_VERSION: u32 = 1;

let timestamps = vec![0_u32, 86_400];
let values = vec![21.75_f32, 18.5];

let mut series = TableBuilder::new(2, TableLayout::DEFAULT)?;
series.set_u32_array(&timestamps)?; // borrowed until the table is written
series.set_f32_array(values.clone())?; // or owned

let mut builder = TableBuilder::versioned(3, LAYOUT, FORMAT_VERSION)?;
builder.set_f64(59.3293)?;
builder.set_str("Europe/Stockholm")?;
builder.set_table(series)?; // written straight into the table
let bytes = builder.to_bytes()?;

let buffer = ByteBuffer::new(&bytes, LAYOUT);
let version = buffer.version()?;
if version != FORMAT_VERSION {
    let supported = FORMAT_VERSION;
    return Err(DecodeError::UnsupportedVersion { version, supported }.into());
}

let latitude = buffer.get_f64(buffer.root_offset(0)?)?;
let time_zone = buffer.get_str(buffer.root_offset(1)?)?;
let table = buffer.root_offset(2)?;
let read = buffer.get_f32_array(buffer.field_offset(table, 1)?)?;

assert_eq!((latitude, time_zone), (59.3293, "Europe/Stockholm"));
assert_eq!(read.to_vec(), values);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Records at fixed offsets without a header or offsets, such as stream frames,
are better written and read with the standard library's `to_le_bytes()` and
`from_le_bytes()`.

## Format

All values are little-endian. A table is its header, one offset per field
and then the fields in the order they were set:

- The header holds the table's byte length when the layout is `sized`, and
  then its version when it's `versioned`.
- Header slots, offsets and length prefixes are all `offset_size` bytes wide:
  1, 2 or 4, which limits the table to 255 bytes, 64 KiB or 4 GiB.
- Offsets count from the table's start. A table nested with `set_table()` has
  no header, and its offsets count from its own start.
- Arrays and strings start with their element count. Arrays are padded so
  that their elements are aligned to their size from the table's start.
  `set_table()` aligns a nested table to the largest element size of its
  arrays when given its builder, and to 4 bytes by default when given bytes.
- A vector starts with its item count, and each item with its byte length.

[SPEC.md](https://github.com/snovader/buffy/blob/main/SPEC.md) describes
the format in full. The crate writes the same bytes as buffy's TypeScript,
Python and Java runtimes for the same fields, and reads their tables: its
tests read and rebuild the
[conformance suite](https://github.com/snovader/buffy/tree/main/conformance)
in buffy's repository, golden tables that cover the whole format, byte for
byte.

## Borrowing and copying

- **Builders borrow or own their values.** Array setters take anything that
  converts into a `Cow` of the element type: a slice or `&Vec` is borrowed
  until the table is written, and a `Vec` is owned. `set_str()` takes a
  `&str` or a `String` the same way. The borrow checker keeps borrowed values
  from changing before the table is written.
- **Nest builders rather than bytes.** `set_table()` and `set_vector()` take
  a `TableBuilder`, owned or borrowed, and write it straight into its parent,
  so the outermost table allocates and writes all the bytes once. A table
  nested as bytes was written into an allocation of its own and is copied.
- **Arrays are read in place.** `ByteBuffer`'s array getters return an
  `ArrayView`, which reads elements from the bytes at any alignment and in
  any host byte order. `as_slice()` returns them as a slice when the host is
  little-endian and the elements are aligned to their type in memory, and
  `to_cow()` falls back to a copy otherwise. `TableBuilder` aligns arrays
  from the table's start, so they're aligned in memory when the table is, as
  a `Vec<u8>` from the system allocator typically is but isn't guaranteed to
  be. Byte arrays and strings are always slices.

## Performance

`benchmarks/` compares the runtime with the flatbuffers, prost, rmp-serde
and rkyv crates on the workloads that every language's benchmarks run, as
its README describes. Its fastest samples, on an Apple M2 Pro:

- **Arrays are written in bulk.** A document of 100,000 rows of three
  columns, 1.2 MB, took 18 µs to write, about as long as rkyv, which copies
  arrays whole too, and 6 to 43 times faster than flatbuffers, prost and
  rmp-serde, which write them element by element. The builder writes the
  table at its exact size, appending to reserved capacity rather than zeroing
  it first.
- **Arrays are read in place.** Reading the document, with its columns as
  slices from `ArrayView::to_cow()`, took 34 ns at 100 rows and at 100,000
  alike. rkyv took 15 µs to copy the large document's columns out of its
  archive, and the others 0.2 to 1.2 ms.
- **Read arrays whole.** An `ArrayView`'s iterator reads each element
  without a check of its own: summing the document's 100,000 floats through
  it took 86 µs, bound by the additions, as long as through flatbuffers' and
  rkyv's accessors. `get_f32()` at each position would check every read and
  return early from the loop on an error.
- **Small tables are quick to write and read.** A table of two `f64`s, an
  `i64` and a 16-byte string took 79 ns to write, including allocating the
  builder's record of fields and the table, and 13 ns to read.
- **Each nested builder allocates.** A vector of 100 tables that each nest
  another took 9.0 µs to write, 2.5 to 6.4 times as long as flatbuffers,
  rmp-serde and prost, which took 1.4 µs: each of its 200 tables is a
  builder, which allocates its record of fields. It took 0.30 µs to read.

## Validation and errors

Validation is always on:

- Every read checks its range against the bytes the `ByteBuffer` was given
  and returns a `DecodeError` if it would leave them. Strings must be valid
  UTF-8. A truncated or corrupted buffer therefore returns an error rather
  than values from outside it, and no bytes make a read panic.
- `DecodeError::OutOfRange` gives the read's position and length and the
  bytes available, and `DecodeError::InvalidUtf8` has the `Utf8Error` as its
  source.
- `ByteBuffer` doesn't compare versions. A reader returns
  `DecodeError::UnsupportedVersion` when `version()` isn't one it reads, as
  the example does, so that every decoding failure is a `DecodeError`.
- Checks can't tell a value that was corrupted within range from a real one,
  such as an offset that points at another field. A format that needs that
  guarantee can add a checksum.

`TableBuilder` returns a `BuildError`, whatever the values' bytes, rather
than panicking, since WebAssembly builds abort on panic:

- A setter returns an error for a field beyond the number the table was
  created with, or one that would make the table too long for its
  `offset_size`, and leaves the builder as it was. `to_bytes()`, and nesting
  a builder, return an error while fields are left to set. A version that
  doesn't fit, or a header too long for its `offset_size`, is an error from
  the constructor.
- The compiler requires a version exactly for a versioned layout, and offers
  `ByteBuffer::version()` only for one, through `TableLayout`'s `VERSIONED`
  parameter.
