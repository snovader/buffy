# buffy-core

Binary tables of scalars, strings and typed arrays, in
[buffy](https://github.com/snovader/buffy)'s format. `TableBuilder` writes a
table at its exact size, copying typed arrays in bulk, and `ByteBuffer` reads
fields back by offset, with arrays as views of the bytes. Every read checks
that it stays within the bytes it was given, so tables can come from untrusted
sources.

```sh
pip install buffy-core
```

The package is `buffy_core`. It has no dependencies, and needs Python 3.14 or
later.

```python
from array import array

from buffy_core import ByteBuffer, TableBuilder, TableLayout

# A format's header, shared by its writer and its readers
LAYOUT = TableLayout(sized=True, versioned=True)
FORMAT_VERSION = 1

series = TableBuilder(2)
series.set_uint32_array(array("I", [0, 86400]))  # copied as bytes
series.set_float32_array([21.75, 18.5])  # a list is converted first

builder = TableBuilder(3, LAYOUT, FORMAT_VERSION)
builder.set_float64(59.3293)
builder.set_string("Europe/Stockholm")
builder.set_table(series)  # written straight into the table
data = builder.bytes()

buffer = ByteBuffer(data, LAYOUT)
if buffer.get_version() != FORMAT_VERSION:
    raise ValueError("Unsupported format version")

latitude = buffer.get_float64(buffer.root_offset(0))
time_zone = buffer.get_string(buffer.root_offset(1))
table = buffer.root_offset(2)
values = buffer.get_float32_array(buffer.field_offset(table, 1))  # a memoryview
```

Records at fixed offsets without a header or offsets, such as stream frames,
are better written and read with the standard library's `struct`.

## Validation and errors

Validation is always on. Every read checks its range against the bytes the
`ByteBuffer` was given and raises `DecodeError`, a `ValueError`, if it would
leave them, and strings must be valid UTF-8. A truncated or corrupted buffer
therefore raises rather than returning values from outside it. Using the API
wrongly, such as setting more fields than a `TableBuilder` was created with,
raises other errors, whatever the bytes.

## Documentation

The package's docstring, `help(buffy_core)`, describes the format, what is
copied and what is a view, measured performance, and every error.
[SPEC.md](https://github.com/snovader/buffy/blob/main/SPEC.md) describes the
format in full.

buffy has implementations in TypeScript, Python, Java and Rust, which write
the same bytes for the same fields and read each other's tables. Each reads
and rebuilds the
[conformance suite](https://github.com/snovader/buffy/tree/main/conformance)
in buffy's repository, golden tables that cover the whole format, byte for
byte.
