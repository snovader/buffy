# @buffy/core

Binary tables of scalars, strings and typed arrays. `TableBuilder` writes a
table at its exact size, copying typed arrays in bulk, and `ByteBuffer` reads
fields back by offset, with arrays as views of the bytes where it can. Every
read checks that it stays within the bytes it was given.

```ts
import { ByteBuffer, TableBuilder, TableLayout } from '@buffy/core';

// A format's header, shared by its writer and its readers
const LAYOUT = TableLayout.create({ sized: true, versioned: true });
const FORMAT_VERSION = 1;

const series = new TableBuilder(2);
series.setUint32Array(timestamps); // a Uint32Array is copied as bytes
series.setFloat32Array(values); // a number[] would be converted first

const builder = new TableBuilder(3, LAYOUT, FORMAT_VERSION);
builder.setFloat64(59.3293);
builder.setString('Europe/Stockholm');
builder.setTable(series); // written straight into the table
const bytes = builder.bytes();

const buffer = new ByteBuffer(bytes, LAYOUT);
if (buffer.getVersion() !== FORMAT_VERSION) {
    throw new Error('Unsupported format version');
}

const latitude = buffer.getFloat64(buffer.rootOffset(0));
const timeZone = buffer.getString(buffer.rootOffset(1));
const table = buffer.rootOffset(2);
const read = buffer.getFloat32Array(buffer.fieldOffset(table, 1));
```

`StructWriter` and the `readUint8()`/`readInt32()`/`readFloat32()` helpers
write and read records at fixed offsets, such as stream frames, without a
header or offsets.

## Format

All values are little-endian. A table is its header, one offset per field
and then the fields in the order they were set:

- The header holds the table's byte length when the layout is `sized`, and
  then its version when it's `versioned`.
- Header slots, offsets and length prefixes are all `offsetSize` bytes wide:
  1, 2 or 4, which limits the table to 255 bytes, 64 KiB or 4 GiB.
- Offsets count from the table's start. A table nested with `setTable()` has
  no header, and its offsets count from its own start.
- Arrays and strings start with their element count. Typed arrays are padded
  so that their elements are aligned to their size from the table's start.
  `setTable()` aligns a nested table to the largest element size of its
  arrays when given its builder, and to 4 bytes by default when given bytes.
- A vector starts with its item count, and each item with its byte length.

The repository's `conformance` directory holds golden tables of every part of
the format, which every implementation of buffy reads and rebuilds byte for
byte; its README describes them. This package writes them, and
`yarn conformance:generate` regenerates them.

## Performance

- **Typed arrays of the field's element type are copied as bytes.** Any
  other array-like, such as a `number[]` or a typed array of another type, is
  converted as the typed array's constructor would convert it. Writing 27,028
  floats took 3.5–5 µs from a `Float32Array` and 14–20 µs from a `number[]`
  in Node and d8. Conversion is a built-in rather than a JS loop, so it
  doesn't slow down when a program passes many kinds of input.
- **Array reads return views when they can.** A view shares the bytes'
  storage, so it's free and reflects later changes to the bytes. An array
  that isn't aligned to its element size in the backing `ArrayBuffer` is
  copied instead, because typed arrays can't start there. Tables that
  `TableBuilder` writes are aligned; bytes received at an odd offset aren't.
  On a big-endian host, arrays of multi-byte elements are always copies, with
  each element's bytes swapped to the host's order.
- **Read arrays whole.** Each scalar getter is a call with a range check:
  summing a 27,028-element Float32 array with `getFloat32()` took about
  60 µs, against about 23 µs through the array's view.
- **Nest builders rather than bytes.** `setTable()` and `setVector()` write
  a `TableBuilder` straight into its parent when the parent is finalized, so
  the outermost table allocates and writes all the bytes once. A table nested
  as bytes was built into a buffer of its own and is copied. In Node, a
  vector of 100 tables that each nest another took 12 µs to build from
  builders and 49 µs from bytes, and a table nesting 100,000 rows of three
  columns took 30 µs and 77 µs.
- **Small tables cost about the allocation of their bytes.** A table of two
  float64s, a string and an int64 took 0.33 µs in Node, of which about 0.2 µs
  was allocating its bytes and their `DataView`. A nested builder allocates
  neither, so the vector of 100 nested tables took 12 µs against 7–13 µs in
  flatbuffers, protobufjs and msgpackr, and buffy read it 2–3 times as fast
  as they did.

## Validation and errors

Validation is always on:

- Every read checks its range against the bytes the `ByteBuffer` was given,
  even when the backing `ArrayBuffer` is larger, and throws a `DecodeError`
  if it would leave them. Strings must be valid UTF-8. A truncated or
  corrupted buffer therefore throws rather than returning values from outside
  it.
- `DecodeError` extends `RangeError`. Its message gives the read's position
  and length and the bytes available, and an invalid string's error has the
  `TypeError` from decoding as its `cause`.
- Checks can't tell a value that was corrupted within range from a real one,
  such as an offset that points at another field. A format that needs that
  guarantee can add a checksum.

Using the API wrongly throws other errors, whatever the bytes:

- Writes past the end of a `ByteBuffer` throw a `RangeError` and write
  nothing.
- `TableBuilder` throws a `RangeError` for more or fewer fields than it was
  created with, or a version that doesn't fit. A field that would make the
  table too long for its `offsetSize` throws a `RangeError` from its setter
  and isn't added. A version missing from or given for a layout, or a field
  set after `finalize()`, throws a `TypeError`.
- `getVersion()` throws a `TypeError` when the layout isn't versioned.
