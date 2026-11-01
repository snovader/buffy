"""Binary tables of scalars, strings and typed arrays. ``TableBuilder`` writes a
table at its exact size, copying typed arrays in bulk, and ``ByteBuffer`` reads
fields back by offset, with arrays as views of the bytes. Every read checks
that it stays within the bytes it was given.

>>> from array import array
>>> from tafla import ByteBuffer, TableBuilder, TableLayout

A format's header, shared by its writer and its readers:

>>> LAYOUT = TableLayout(sized=True, versioned=True)
>>> FORMAT_VERSION = 1

>>> series = TableBuilder(2)
>>> series.set_uint32_array(array("I", [0, 86400]))  # copied as bytes
>>> series.set_float32_array([21.75, 18.5])  # a list is converted first

>>> builder = TableBuilder(3, LAYOUT, FORMAT_VERSION)
>>> builder.set_float64(59.3293)
>>> builder.set_string("Europe/Stockholm")
>>> builder.set_table(series)  # written straight into the table
>>> data = builder.bytes()

>>> buffer = ByteBuffer(data, LAYOUT)
>>> buffer.get_version() == FORMAT_VERSION
True
>>> buffer.get_float64(buffer.root_offset(0))
59.3293
>>> buffer.get_string(buffer.root_offset(1))
'Europe/Stockholm'
>>> table = buffer.root_offset(2)
>>> buffer.get_float32_array(buffer.field_offset(table, 1)).tolist()
[21.75, 18.5]

Records at fixed offsets without a header or offsets, such as stream frames,
are better written and read with the standard library's ``struct``.

Format
------

All values are little-endian. A table is its header, one offset per field and
then the fields in the order they were set:

- The header holds the table's byte length when the layout is ``sized``, and
  then its version when it's ``versioned``.
- Header slots, offsets and length prefixes are all ``offset_size`` bytes wide:
  1, 2 or 4, which limits the table to 255 bytes, 64 KiB or 4 GiB.
- Offsets count from the table's start. A table nested with ``set_table()`` has
  no header, and its offsets count from its own start.
- Arrays and strings start with their element count. Typed arrays are padded so
  that their elements are aligned to their size from the table's start.
  ``set_table()`` aligns a nested table to the largest element size of its
  arrays when given its builder, and to 4 bytes by default when given bytes.
- A vector starts with its item count, and each item with its byte length.

Performance
-----------

Measured in CPython 3.14 on an Apple M2 Pro, with the garbage collector off:

- **Buffers of the field's element type are copied as bytes.** That includes
  an ``array`` of the type code and a numpy array of the type. Anything else,
  such as a list or an array of another type, is converted as ``array()``
  converts an iterable, at about 13 ns per element: a table of 27,028 floats
  took 5 µs to write from an ``array("f")`` and 0.36 ms from a list.
- **Array reads return views.** A view shares the bytes' storage, so it's free
  and reflects later changes to the bytes. Unlike typed arrays in JavaScript,
  a ``memoryview`` reads elements at any alignment, so arrays received at an
  odd offset are views too. On a big-endian host, arrays of multi-byte
  elements are copies, with each element's bytes swapped to the host's order.
- **Read arrays whole.** Each scalar getter is a call with a range check:
  summing a 27,028-element float32 array with ``get_float32()`` took 3.4 ms,
  against 0.24 ms through the array's view.
- **Nest builders rather than bytes.** ``set_table()`` and ``set_vector()``
  write a ``TableBuilder`` straight into its parent when the parent is
  finalized, so the outermost table allocates and writes all the bytes once.
  A table nested as bytes was built into a buffer of its own and is copied. A
  vector of 100 tables that each nest another took 0.27 ms to build from
  builders and 0.42 ms from bytes, and a table nesting 27,028 rows of three
  columns took 16 µs and 31 µs.
- **Small tables cost their calls.** Each field is a few Python calls to set
  and to write, so a table of two float64s, an int64 and a 16-byte string took
  2.6 µs, far more than allocating and copying its 60 bytes.

Validation and errors
---------------------

Validation is always on:

- Every read checks its range against the bytes the ``ByteBuffer`` was given
  and raises ``DecodeError`` if it would leave them. Strings must be valid
  UTF-8. A truncated or corrupted buffer therefore raises rather than returning
  values from outside it.
- ``DecodeError`` is a ``ValueError``. Its message gives the read's position
  and length and the bytes available, and an invalid string's error has the
  ``UnicodeDecodeError`` as its cause.
- Checks can't tell a value that was corrupted within range from a real one,
  such as an offset that points at another field. A format that needs that
  guarantee can add a checksum.

Using the API wrongly raises other errors, whatever the bytes:

- Writes past the end of a ``ByteBuffer``, and arrays, strings and vectors
  too long for its ``offset_size``, raise ``ValueError`` and write nothing. A
  scalar that doesn't fit its type raises ``ValueError`` and leaves the
  position where it was, though CPython zeroes the scalar's bytes first.
- ``TableBuilder`` raises ``ValueError`` for more or fewer fields than it was
  created with, a version that doesn't fit, or an array or table given as a
  buffer whose length changed before ``finalize()``. A field that would make
  the table too long for its ``offset_size`` raises ``ValueError`` from its
  setter and isn't added. A value that its type can't hold raises
  ``OverflowError`` or ``TypeError`` from an array setter, and ``ValueError``
  from ``finalize()`` for a scalar. A string with a lone surrogate raises
  ``UnicodeEncodeError`` from ``set_string()``. A version missing from or
  given for a layout, or a field set after ``finalize()``, raises
  ``TypeError``.
- ``get_version()`` raises ``TypeError`` when the layout isn't versioned.
"""

from .byte_buffer import ByteBuffer
from .constants import SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64
from .errors import DecodeError
from .layout import TableLayout
from .table_builder import TableBuilder

__all__ = [
    "ByteBuffer",
    "DecodeError",
    "TableBuilder",
    "TableLayout",
    "SIZEOF_8",
    "SIZEOF_16",
    "SIZEOF_32",
    "SIZEOF_64",
]
