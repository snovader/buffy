# tafla format

This document describes tafla's binary format. Every implementation writes
and reads it, and the conformance suite in `conformance/` checks them against
it byte for byte. Until the first release, the format may still change.

## Overview

A tafla table holds a fixed number of fields, read by their index. The bytes
don't describe themselves: the reader knows each field's type from the format
it reads, such as code generated from a schema. A table has no magic number,
schema identifier or checksum; a format that needs one can add it as a field
or around the table.

All multi-byte values are little-endian. Integers are two's complement and
floats are IEEE 754 binary32 and binary64.

## Layout

A table's layout is fixed by its format, and the reader must know it:

- **Offset size**: 1, 2 or 4 bytes. Header slots, field offsets and every
  length or count prefix in the table are unsigned integers of this width.
  A table can't be longer than the largest of them: 255 bytes, 65,535 bytes
  or 4,294,967,295 bytes.
- **Sized**: whether the header holds the table's length.
- **Versioned**: whether the header holds a version.

The default layout has 4-byte offsets and no header slots.

## Table

A table is its header, then one offset per field, then the fields' values:

```text
[length]? [version]? offset[0] … offset[n-1] value … value
```

- **Length**, present when the layout is sized, is the byte length of the
  table.
- **Version**, present when the layout is versioned, is an unsigned integer
  chosen by the format, so that readers can reject versions they don't read.
- **Offset `i`** is the position of field `i`'s value, counted from the start
  of the table. The offsets of a table of `n` fields start right after the
  header, so field `i`'s offset is at `headerSize + i × offsetSize`, where the
  header is 0, 1 or 2 slots of `offsetSize` bytes.

Writers write the values in field order, each after the previous one, with
only the padding described below between them. Padding bytes are zero, and
readers ignore them.

## Values

| Type                                    | Bytes at the field's offset                                    |
| --------------------------------------- | -------------------------------------------------------------- |
| `int8` … `uint64`, `float32`, `float64` | The value, in 1, 2, 4 or 8 bytes                               |
| Array of one of those types             | A count `n`, then `n` elements                                 |
| String                                  | A count `n`, then `n` bytes of UTF-8                           |
| Struct                                  | Its scalars, one after another                                 |
| Array of structs                        | A count `n`, then `n` structs                                  |
| Nested table                            | The table's bytes, with no length prefix                       |
| Vector of tables                        | A count `n`, then the end of each item, then the `n` items     |

Counts, lengths and item ends are `offsetSize` bytes wide, as offsets are.

- **Scalars** aren't aligned.
- **Structs** are a fixed sequence of scalars, which the format defines, as
  it defines a table's fields; a struct within a struct adds its scalars in
  its place. A struct has no header, offsets or padding, so its scalars lie
  at fixed offsets from its start. Its size is the sum of its scalars' sizes,
  and its alignment the largest of them. A struct field isn't aligned, like a
  scalar.
- **Arrays** are aligned: writers pad before an array's count so that its
  first element starts at a multiple of its alignment from the start of the
  table, which is the element's size for scalars, and the struct's alignment
  for structs. Readers must accept unaligned arrays too, since tables can
  arrive at any position in memory.
- **Strings** must be valid UTF-8, without a terminator. Readers reject
  invalid UTF-8.
- **Nested tables** are padded to their alignment. A nested builder is
  aligned to the largest alignment of its arrays and of its own nested
  tables, or 1 if it has neither; a nested table given as bytes is aligned to
  4 unless the writer passes another alignment. A nested table has no length
  prefix, so its own offsets lead to its fields.
- **Vectors** hold tables of any layout. Each item's end is its offset from
  the vector's position, which is that of its count. Item `i` starts where
  item `i - 1` ends, and item 0 right after the last end, so readers find any
  item from at most two ends, without reading the items before it. Items
  aren't padded, so their arrays may be unaligned.

### Reading nested tables

A nested table can be read in place, with offsets counted from its own start,
when it has no header and offsets as wide as its parent's. Any other nested
table is read as a table of its own, with its own layout: a vector item from
its bytes, and a table field from its offset to the end of its parent, since
its length isn't written.

## Validation

Readers decode untrusted bytes. An implementation:

- Checks every read against the bytes it was given, including offsets,
  counts, lengths and item ends, and fails with its decode error when a read
  would leave them, rather than reading other memory or allocating for a
  count that the bytes can't hold. A vector item must start no later than it
  ends.
- Rejects strings that aren't valid UTF-8 with its decode error.
- Reports every failure to decode as its decode error, and doesn't crash,
  panic or hang on any input.

Checks can't tell a value corrupted within range from a real one, such as an
offset that points at another field.

## Writers

Writers produce the same bytes for the same fields in every implementation:
header, offsets, values in field order, the padding above and nothing else. A
writer rejects a table longer than its offset size addresses, and a version
that doesn't fit in it, rather than writing a table that readers would misread.

## Conformance

`conformance/cases.json` describes valid tables of every part of the format
and corrupted ones, and `conformance/README.md` describes how implementations
check themselves against them. A change to the bytes of an existing case is a
change to this format.
