from collections.abc import Buffer, Callable, Iterable, Sequence
from typing import Any, Literal, overload

from .byte_buffer import ByteBuffer
from .constants import SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64
from .elements import as_elements
from .layout import TableLayout
from .types import AddressableSpace, ElementSize, TypeCode

type _Write = Callable[[ByteBuffer, Any], None]

# The longest table that offsets and lengths of each size address
_MAX_LENGTHS: dict[AddressableSpace, int] = {
    SIZEOF_8: 0xFF,
    SIZEOF_16: 0xFFFF,
    SIZEOF_32: 0xFFFF_FFFF,
}


class TableBuilder:
    """Builds a table: a header with one offset per field, after the table's
    total length and version when its layout has them, followed by the fields
    in the order they're set. Arrays, strings and vectors are length-prefixed,
    and typed arrays are padded so their elements are aligned from the start of
    the table. Fields are read back by index with ``ByteBuffer.root_offset()``,
    or ``ByteBuffer.field_offset()`` for a table nested in another.

    Setting a field records it and its position. ``finalize()`` then allocates
    the table at its exact size and writes every field in one pass, so arrays
    and tables given as buffers must keep their length in between; a changed
    length raises ``ValueError``. Tables nested from their builders are written
    in the same pass, straight into this one. A setter raises ``ValueError``,
    and leaves the builder as it was, when its field would make the table
    longer than the layout's ``offset_size`` addresses: 255 bytes, 64 KiB or 4
    GiB.
    """

    __slots__ = [
        "__alignment",
        "__buffer",
        "__fields",
        "__layout",
        "__max_length",
        "__offset",
        "__records",
        "__size",
        "__version",
    ]

    __buffer: ByteBuffer | None
    __size: AddressableSpace
    __alignment: ElementSize
    # Per field: the padding before it, its position from the table's start,
    # the function that writes it and its value. The functions aren't bound
    # to the builder, so that it isn't in a reference cycle with its values.
    __records: list[tuple[int, int, _Write, Any]]

    @overload
    def __init__(
        self, fields: int, layout: TableLayout[Literal[False]] = TableLayout.DEFAULT
    ) -> None: ...

    @overload
    def __init__(
        self, fields: int, layout: TableLayout[Literal[True]], version: int
    ) -> None: ...

    def __init__(
        self,
        fields: int,
        layout: TableLayout = TableLayout.DEFAULT,
        version: int | None = None,
    ) -> None:
        """``fields`` is the number of fields that the table will hold, and
        ``layout`` its header and offset width, which its readers must use too.
        ``version`` is written to the header, and required when the layout is
        versioned. Readers can check it with ``ByteBuffer.get_version()``
        before reading any field.

        Raises ``TypeError`` if ``version`` is given for a layout without a
        version slot, or missing for one with it. Raises ``ValueError`` if
        ``version`` doesn't fit in the layout's offset size, or the header
        alone is longer than the layout addresses.
        """
        size = layout.offset_size()
        max_length = _MAX_LENGTHS[size]
        versioned = layout.versioned()

        if versioned != (version is not None):
            raise TypeError(
                "A versioned layout needs a version"
                if versioned
                else "The layout has no version slot"
            )

        if version is not None and not 0 <= version <= max_length:
            raise ValueError(f"Version {version} doesn't fit in a {size}-byte slot")

        self.__buffer = None
        self.__layout = layout
        self.__version = version
        self.__size = size
        self.__fields = fields
        self.__max_length = max_length
        # The table's length so far, which is where the next field goes
        self.__offset = layout.header_size() + fields * size
        # The largest alignment that a field needs from the table's start,
        # which a parent pads this table to when it nests it
        self.__alignment = SIZEOF_8
        self.__records = []

        if self.__offset > max_length:
            raise ValueError(
                f"A header of {fields} fields doesn't fit in {size}-byte offsets"
            )

    def finalize(self) -> ByteBuffer:
        """Writes the table, once; later calls return the same buffer.

        Raises ``ValueError`` if fewer fields were set than the table holds, or
        if the length of an array or table given as a buffer changed since it
        was set. A failed write leaves the builder unfinalized, with its fields
        set.
        """
        if self.__buffer is not None:
            return self.__buffer

        self.__assert_complete()

        buffer = ByteBuffer(bytearray(self.__offset), self.__layout)
        self.__write(buffer)

        # Release the values, which may be large arrays
        self.__records = []
        self.__buffer = buffer

        return buffer

    def bytes(self) -> memoryview[int]:
        return self.finalize().bytes()

    def set_int8(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_int8, value, SIZEOF_8, 0)

    def set_uint8(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_uint8, value, SIZEOF_8, 0)

    def set_int16(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_int16, value, SIZEOF_16, 0)

    def set_uint16(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_uint16, value, SIZEOF_16, 0)

    def set_int32(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_int32, value, SIZEOF_32, 0)

    def set_uint32(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_uint32, value, SIZEOF_32, 0)

    def set_int64(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_int64, value, SIZEOF_64, 0)

    def set_uint64(self, value: int) -> None:
        self.__add_field(ByteBuffer.set_uint64, value, SIZEOF_64, 0)

    def set_float32(self, value: float) -> None:
        self.__add_field(ByteBuffer.set_float32, value, SIZEOF_32, 0)

    def set_float64(self, value: float) -> None:
        self.__add_field(ByteBuffer.set_float64, value, SIZEOF_64, 0)

    # A contiguous buffer of the field's element type is copied when the table
    # is written. Anything else is converted when it's set, so that a value
    # that doesn't fit raises from its setter, and the builder owns the copy.
    # Writing a table of 27,028 floats took 0.36 ms from a list, against 5 µs
    # from an array("f").

    def set_int8_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of int8s, such as
        ``array("b")``, is copied as bytes; anything else, such as a list, is
        converted as ``array("b", values)`` would, which takes several times as
        long."""
        self.__add_array("b", SIZEOF_8, ByteBuffer.set_int8_array, values)

    def set_uint8_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. Bytes, or any other contiguous buffer of
        uint8s, are copied as they are; anything else, such as a list, is
        converted as ``array("B", values)`` would, which takes several times as
        long."""
        self.__add_array("B", SIZEOF_8, ByteBuffer.set_uint8_array, values)

    def set_int16_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of int16s in the host's
        byte order, such as ``array("h")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("h", values)`` would, which
        takes several times as long."""
        self.__add_array("h", SIZEOF_16, ByteBuffer.set_int16_array, values)

    def set_uint16_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of uint16s in the host's
        byte order, such as ``array("H")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("H", values)`` would, which
        takes several times as long."""
        self.__add_array("H", SIZEOF_16, ByteBuffer.set_uint16_array, values)

    def set_int32_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of int32s in the host's
        byte order, such as ``array("i")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("i", values)`` would, which
        takes several times as long."""
        self.__add_array("i", SIZEOF_32, ByteBuffer.set_int32_array, values)

    def set_uint32_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of uint32s in the host's
        byte order, such as ``array("I")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("I", values)`` would, which
        takes several times as long."""
        self.__add_array("I", SIZEOF_32, ByteBuffer.set_uint32_array, values)

    def set_int64_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of int64s in the host's
        byte order, such as ``array("q")`` or a numpy int64 array, is copied as
        bytes; anything else, such as a list, is converted as
        ``array("q", values)`` would, which takes several times as long."""
        self.__add_array("q", SIZEOF_64, ByteBuffer.set_int64_array, values)

    def set_uint64_array(self, values: Buffer | Iterable[int]) -> None:
        """Sets an array field. A contiguous buffer of uint64s in the host's
        byte order, such as ``array("Q")`` or a numpy uint64 array, is copied
        as bytes; anything else, such as a list, is converted as
        ``array("Q", values)`` would, which takes several times as long."""
        self.__add_array("Q", SIZEOF_64, ByteBuffer.set_uint64_array, values)

    def set_float32_array(self, values: Buffer | Iterable[float]) -> None:
        """Sets an array field. A contiguous buffer of float32s in the host's
        byte order, such as ``array("f")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("f", values)`` would, which
        takes several times as long."""
        self.__add_array("f", SIZEOF_32, ByteBuffer.set_float32_array, values)

    def set_float64_array(self, values: Buffer | Iterable[float]) -> None:
        """Sets an array field. A contiguous buffer of float64s in the host's
        byte order, such as ``array("d")``, is copied as bytes; anything else,
        such as a list, is converted as ``array("d", values)`` would, which
        takes several times as long."""
        self.__add_array("d", SIZEOF_64, ByteBuffer.set_float64_array, values)

    def set_string(self, string: str) -> None:
        """Writes ``string`` as UTF-8. Raises ``UnicodeEncodeError``, a
        ``ValueError``, if it has a lone surrogate, which UTF-8 can't encode."""
        encoded = string.encode()
        self.__add_field(ByteBuffer.set_string, encoded, self.__size + len(encoded), 0)

    def set_table(
        self, table: TableBuilder | Buffer, alignment: ElementSize | None = None
    ) -> None:
        """Embeds a table, from its builder or its bytes, without a length
        prefix. Readers find its fields with ``ByteBuffer.field_offset()`` from
        the field's offset, so its layout must be ``TableLayout.DEFAULT``.

        A builder must have all its fields set. ``finalize()`` writes it
        straight into this table, without bytes of its own. It can still be
        finalized or nested elsewhere too, since a complete builder doesn't
        change.

        The nested table's typed arrays are aligned from its own start, so it's
        padded to start at a multiple of ``alignment``. For a builder, the
        default is the largest element size of its arrays and the alignment of
        its nested tables, or 1 if it has neither. For bytes, the default of 4
        suits tables of 8-, 16- and 32-bit elements; pass 8 for a table with
        64-bit arrays.

        Raises ``ValueError`` if the builder has fields left to set.
        """
        write: _Write
        if isinstance(table, TableBuilder):
            byte_length = TableBuilder.__length_of(table)
            write, value = TableBuilder.__write_table, (table, self.__size)
            default_alignment = table.__alignment
        else:
            byte_length = memoryview(table).nbytes
            write, value = _write_bytes, (table, byte_length)
            default_alignment = SIZEOF_32

        if alignment is None:
            alignment = default_alignment

        padding = _padding(self.__offset, alignment)
        self.__add_field(write, value, byte_length, padding)
        self.__align_to(alignment)

    def set_vector(self, vector: Sequence[TableBuilder | Buffer]) -> None:
        """Writes a vector of tables, from their builders or their bytes, each
        with a length prefix. Builders must have all their fields set, and are
        written straight into this table as ``set_table()`` writes them, except
        for those whose layout has another ``offset_size``, which are finalized
        and copied. Items aren't padded.

        Raises ``ValueError`` if a builder has fields left to set.
        """
        items = tuple(vector)
        byte_length = self.__size * (1 + len(items))
        for item in items:
            byte_length += TableBuilder.__length_of(item)

        self.__add_field(
            TableBuilder.__write_vector,
            (items, byte_length, self.__size),
            byte_length,
            0,
        )

    def __add_array(
        self,
        type_code: TypeCode,
        item_size: ElementSize,
        set_array: _Write,
        values: Buffer | Iterable[float],
    ) -> None:
        """Pads the array so that its elements, after the length, are aligned"""
        elements = as_elements(type_code, values)
        byte_length = memoryview(elements).nbytes
        padding = _padding(self.__offset + self.__size, item_size)

        self.__add_field(
            _write_array,
            (set_array, elements, byte_length),
            self.__size + byte_length,
            padding,
        )
        self.__align_to(item_size)

    def __align_to(self, alignment: ElementSize) -> None:
        self.__alignment = max(self.__alignment, alignment)

    def __add_field(
        self, write: _Write, value: Any, byte_length: int, padding: int
    ) -> None:
        """Records a field of ``byte_length`` bytes after ``padding`` bytes.
        Checking the table's length here, rather than in finalize(), reports a
        field too long for the layout from its own setter and leaves the
        builder as it was. Every offset and length is shorter than the table,
        so the table's length is the one value to check.

        Raises ``ValueError`` if the table already holds all its fields, or the
        field would make it longer than the layout addresses. Raises
        ``TypeError`` if the table is already written.
        """
        if self.__buffer is not None:
            raise TypeError("The table was already finalized")

        records = self.__records
        index = len(records)
        if index == self.__fields:
            raise ValueError(f"Can't add field {index + 1} to a table of {index}")

        position = self.__offset + padding
        length = position + byte_length
        if length > self.__max_length:
            raise ValueError(
                f"Field {index + 1} makes the table {length} bytes, "
                f"too long for {self.__size}-byte offsets"
            )

        records.append((padding, position, write, value))
        self.__offset = length

    def __assert_complete(self) -> None:
        if self.__buffer is None and len(self.__records) != self.__fields:
            raise ValueError(
                f"Only {len(self.__records)} of the table's {self.__fields} "
                "fields were set"
            )

    @staticmethod
    def __length_of(table: TableBuilder | Buffer) -> int:
        """The byte length of a table to nest, which a builder knows once all
        its fields are set. Raises ``ValueError`` if it has fields left to set.
        """
        if not isinstance(table, TableBuilder):
            return memoryview(table).nbytes

        table.__assert_complete()
        return table.__offset

    def __write(self, buffer: ByteBuffer) -> None:
        """Writes the table from the buffer's position, which counts as its
        start. The buffer's offsets must be as wide as the table's."""
        if self.__layout.sized():
            buffer.set_size(self.__offset)

        if self.__version is not None:
            buffer.set_version(self.__version)

        records = self.__records
        for record in records:
            buffer.set_offset(record[1])

        for padding, _, write, value in records:
            if padding:
                buffer.set_padding(padding)

            write(buffer, value)

    @staticmethod
    def __write_table(
        buffer: ByteBuffer, field: tuple[TableBuilder, AddressableSpace]
    ) -> None:
        table, size = field
        TableBuilder.__write_item(buffer, table, size)

    @staticmethod
    def __write_vector(
        buffer: ByteBuffer,
        field: tuple[tuple[TableBuilder | Buffer, ...], int, AddressableSpace],
    ) -> None:
        """Writes a vector into a buffer whose offsets are ``size`` bytes wide,
        like the vector's count and lengths"""
        items, byte_length, size = field
        lengths = [TableBuilder.__length_of(item) for item in items]
        _check_length(byte_length, size * (1 + len(items)) + sum(lengths))

        buffer.set_size(len(items))
        for item, length in zip(items, lengths, strict=True):
            buffer.set_size(length)
            TableBuilder.__write_item(buffer, item, size)

    @staticmethod
    def __write_item(
        buffer: ByteBuffer, table: TableBuilder | Buffer, size: AddressableSpace
    ) -> None:
        """Writes a builder straight into a buffer whose offsets are ``size``
        bytes wide, unless it was finalized already or has offsets of another
        width, since the buffer writes every offset and length at its own
        width. Those tables, and bytes, are copied."""
        if not isinstance(table, TableBuilder):
            buffer.set_bytes(table)
        elif table.__buffer is None and table.__size == size:
            table.__write(buffer)
        else:
            buffer.set_bytes(table.bytes())


def _write_bytes(buffer: ByteBuffer, field: tuple[Buffer, int]) -> None:
    table, byte_length = field
    _check_length(byte_length, memoryview(table).nbytes)

    buffer.set_bytes(table)


def _write_array(buffer: ByteBuffer, field: tuple[_Write, Buffer, int]) -> None:
    set_array, elements, byte_length = field
    _check_length(byte_length, memoryview(elements).nbytes)

    set_array(buffer, elements)


def _check_length(recorded: int, current: int) -> None:
    """Rejects a field whose length changed after it was set, which would shift
    every later field away from its written offset."""
    if current != recorded:
        raise ValueError(
            f"Field length changed from {recorded} to {current} bytes after it was set"
        )


def _padding(offset: int, alignment: ElementSize) -> int:
    remainder = offset % alignment
    return 0 if remainder == 0 else alignment - remainder
