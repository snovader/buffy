# tafla

Binary tables of scalars, strings and typed arrays. `TableBuilder` writes a
table at its exact size, copying typed arrays in bulk, and `ByteBuffer` reads
fields back by offset, with arrays as views of the bytes where it can. Every
read checks that it stays within the bytes it was given.

```ts
import { ByteBuffer, TableBuilder, TableLayout } from 'tafla';

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

`StructWriter`, and the read and write functions, one of each per scalar
type, such as `readUint32()` and `writeFloat64()`, read and write records at
fixed offsets, such as stream frames, without a header or offsets. The write
functions write into a `Uint8Array` at an offset, and throw a `RangeError` for
a value that doesn't lie within it, without writing any of it.

`TableSizer` and `TableWriter` write a table whose fields are known before
it's written, as the code that tafla-gen generates writes its tables. The
sizer counts the table's length from its fields, and its writer writes the
same fields, in the same order, into bytes of that length, without recording
them first as `TableBuilder` does. Both pad arrays and nested tables as
`TableBuilder` does, so they write the same bytes.

`TableWriter` is the target of the generated code, whose calls follow from
the schema, so it doesn't check them again. Its contract:

- Fields are set in index order, each once, as the sizer added them, and
  each vector item is followed by its `setItemEnd()`.
- A field index outside its table, or an item outside its vector, writes an
  offset or item end over another slot or field, so the table's bytes are
  undefined. Every write still stays within the writer's bytes: one whose
  slot lies past them writes nothing.
- Fields that differ from those the sizer counted, in their lengths or
  number, throw a `RangeError` rather than end past the bytes, and `bytes()`
  throws one when they didn't fill them.

Rust's writer also reports an index past its table's fields, since the parent
that the generated code hands back already carries the count. The others don't
keep that count for each nested table, so the contract doesn't promise it.
Hand-written tables use `TableBuilder`, which checks every field.

## Format

All values are little-endian. A table is its header, one offset per field
and then the fields in the order they were set:

- The header holds the table's byte length when the layout is `sized`, and
  then its version when it's `versioned`.
- Header slots, offsets, counts, lengths and item ends are all of the
  layout's offset type, `offsetType`: `'uint8'`, `'uint16'` or `'uint32'`,
  whose offset size is 1, 2 or 4 bytes. It limits the table to 255 bytes,
  64 KiB or 4 GiB.
- Offsets count from the table's start. A table nested with `setTable()` has
  no header, and its offsets count from its own start.
- A struct is its scalars one after another, without padding, and
  `setStruct()` writes its bytes in place, like a scalar.
- Arrays and strings start with their element count. Typed arrays are padded
  so that their elements are aligned to their size from the table's start,
  and arrays of structs, which `setStructArray()` writes, to their largest
  scalar's size. `setTable()` aligns a nested table to the largest alignment
  of its arrays when given its builder, and to 4 bytes by default when given
  bytes.
- A vector starts with its item count and the end of each item, counted from
  the vector's start, so `getVectorOffset()` finds any item from two ends,
  without reading the items before it.

The repository's `conformance` directory holds golden tables of every part of
the format, which every implementation of tafla reads and rebuilds byte for
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
- **Write records of fixed size as arrays of structs.** An array of 100
  structs of three float32s took about 1 µs to write and 1,208 bytes, against
  13 µs and 3,208 bytes for a vector of 100 tables that each nest a table of
  two of them, since it's one field of bytes rather than 200 builders.
- **Generated code writes tables directly.** `TableSizer` and `TableWriter`
  write each field once, where `TableBuilder` records it and every nested
  builder first. In Node 26, the generated code wrote a vector of 100 nested
  tables of scalars in 4.1 µs rather than 13.8 µs, 100 tables of a string
  and an array in 11 µs rather than 33 µs, and a table of two float64s, an
  int64 and a string in 0.16 µs rather than 0.47 µs. `TableWriter` writes
  scalars as bytes, since V8 keeps a `Uint8Array` of up to 64 bytes on its
  heap until a `DataView` or `subarray()` asks for its `ArrayBuffer`, which
  took 140 ns.
- **Small tables cost about the allocation of their bytes.** A table of two
  float64s, a string and an int64 took 0.33 µs in Node, of which about 0.2 µs
  was allocating its bytes and their `DataView`. A nested builder allocates
  neither, so the vector of 100 nested tables took 12 µs against 7–13 µs in
  flatbuffers, protobufjs and msgpackr, and tafla read it 2–3 times as fast
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

- `TableLayout.create()` throws a `RangeError` for an offset type other than
  `'uint8'`, `'uint16'` and `'uint32'`, and a `TypeError` for an option other
  than `sized`, `versioned` and `offsetType`.
- Writes past the end of a `ByteBuffer` throw a `RangeError` and write
  nothing.
- `TableBuilder` throws a `RangeError` for more or fewer fields than it was
  created with, or a version that doesn't fit. A field that would make the
  table too long for its offset type throws a `RangeError` from its setter
  and isn't added. A version missing from or given for a layout, or a field
  set after `finalize()`, throws a `TypeError`.
- `TableSizer` throws as `TableBuilder`'s constructor does for its header and
  version, and its `byteLength()` and `writer()` throw a `RangeError` for a
  table too long for its offset type. A `TableWriter` throws a `RangeError`
  from `bytes()` when its fields didn't fill the bytes that its sizer
  counted, and from `setString()` for a string longer than the one counted.
- `getVersion()` throws a `TypeError` when the layout isn't versioned.
- `getVectorOffset()` throws a `RangeError` for an index outside the vector,
  once its count has been checked against the bytes.
