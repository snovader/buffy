from collections.abc import Buffer, Iterable
from struct import Struct, error as StructError
from typing import Literal, overload

from .byte_buffer import struct_count
from .constants import SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64
from .elements import as_elements, little_endian_bytes
from .errors import describe_int
from .layout import (
    ALIGNMENTS,
    MAX_LENGTHS,
    OFFSET_TYPES,
    TableLayout,
    alignment_error,
    header_length,
)
from .structs import (
    float32,
    float64,
    int8,
    int16,
    int32,
    int64,
    uint8,
    uint16,
    uint32,
    uint64,
)
from .types import ElementSize, OffsetSize, TypeCode

# The unsigned integer of each offset size, which offsets, lengths, counts,
# item ends and header slots are written as
_LENGTHS: dict[OffsetSize, Struct] = {
    SIZEOF_8: uint8,
    SIZEOF_16: uint16,
    SIZEOF_32: uint32,
}

# The zeros that pad a field to its alignment, by their number
_PADDING = tuple(bytes(length) for length in range(SIZEOF_64))

# What struct raises for a value that it can't write: struct.error for an
# integer out of range, or an offset too large for the offset type, and
# OverflowError for a float too large for a float32. Slots lie within the
# bytes, since the writer checks each field's index and item end first.
_PACK_ERRORS = (StructError, OverflowError)


# The tables that TableWriter's table is nested in, innermost first: each
# one's number of fields, the index of its next field, and the tables that it
# is nested in
type _Parents = tuple[int, int | None, _Parents | None]

# A vector whose items TableWriter is writing: its position, its number of
# items and the index of the field after it, and the number of items ended of
# the vector whose item holds it, and that vector
type _Vector = tuple[int, int, int, int, _Vector | None]


class TableWriter:
    """Writes a table forward, appending each field to its bytes as it's set,
    for code that knows a table's fields as it writes them, such as the code
    that tafla-gen generates. Unlike ``TableBuilder``, it doesn't record the
    fields first, which takes most of a small table's time.

    Fields are set in order, each with its index in its table. A nested table
    follows its parent's ``set_table()``, between ``start_table()`` and
    ``end_table()``; the items of a vector follow its ``set_vector()`` the same
    way, each followed by ``set_item_end()``, which writes where it ends. Each
    field's offset is written as the writer reaches the field. Arrays and nested
    tables are padded as ``TableBuilder`` pads them, so the two write the same
    bytes for the same fields, and convert arrays, strings and structs the same
    way, with the same errors: a struct that isn't a C-contiguous buffer raises
    ``TypeError``.

    The writer has no sizer to check its fields against, so it checks their
    order: a field set out of order, twice, or at an index outside its table's
    fields, and an item end set out of order or outside its vector's items,
    raise ``ValueError`` without writing anything, and so do fields of a table
    whose vector has items left to end. ``end_table()`` raises ``ValueError``
    for a nested table with fields left to set, and ``bytes()`` for the root.

    The bytes grow as fields are appended, and ``bytes()`` returns a view of
    them, without copying them. A table too long for the layout's
    ``offset_type`` raises ``ValueError``, from the setter whose offset or count
    doesn't fit or from ``bytes()``. A scalar setter that raised wrote nothing,
    but another setter that raised may have written part of its field, after
    which the writer is of no further use.
    """

    __slots__ = [
        "__data",
        "__ended",
        "__fields",
        "__header_size",
        "__length",
        "__max_length",
        "__next",
        "__parents",
        "__size",
        "__sized",
        "__slots",
        "__table",
        "__vector",
    ]

    __size: OffsetSize

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
        """``fields`` is the number of fields of the table, and ``layout`` its
        header and offset type, which its readers must use too. ``version`` is
        written to the header, and required when the layout is versioned.

        Raises ``TypeError`` if ``version`` is given for a layout without a
        version slot, or missing for one with it. Raises ``ValueError`` if
        ``version`` doesn't fit in the layout's offset type, or the header alone
        is longer than the layout addresses.
        """
        size = layout.offset_size()
        data = bytearray(header_length(fields, layout, version))

        # The offset size, the width of offsets, lengths and header slots
        self.__size = size
        self.__length = _LENGTHS[size]
        self.__max_length = MAX_LENGTHS[size]
        self.__sized = layout.sized()
        self.__header_size = layout.header_size()
        self.__data = data
        # The start of the table being written, which its offsets count from,
        # and the position of its first offset, after the root's header
        self.__table = 0
        self.__slots = self.__header_size
        # The table's number of fields, and the index of the next field to
        # set, which is None while a vector of the table has items to end
        self.__fields = fields
        self.__next: int | None = 0
        # The tables that it's nested in, and the vector whose items are
        # being written and the number of them that ended. They're chains of
        # tuples rather than lists, since allocating two lists made writing a
        # record of four fields 3% slower.
        self.__parents: _Parents | None = None
        self.__vector: _Vector | None = None
        self.__ended = 0

        if version is not None:
            self.__length.pack_into(data, size if self.__sized else 0, version)

    def bytes(self) -> memoryview[int]:
        """Returns a view of the table's bytes, after which it takes no more
        fields.

        Raises ``ValueError`` if the table has fields left to set, or a table
        nested in it hasn't ended, or if it's longer than its layout addresses.
        """
        if self.__parents is not None or self.__next != self.__fields:
            raise self.__incomplete_error()

        data = self.__data
        if len(data) > self.__max_length:
            raise self.__too_long(len(data))

        if self.__sized:
            self.__length.pack_into(data, 0, len(data))

        return memoryview(data)

    def start_table(self, fields: int) -> int:
        """Starts the fields of a nested table or vector item of ``fields``
        fields, after its parent's ``set_table()`` or ``set_vector()``, and
        returns the parent's start, which ``end_table()`` takes back."""
        parent = self.__table
        data = self.__data
        start = len(data)
        data += bytes(fields * self.__size)

        self.__parents = (self.__fields, self.__next, self.__parents)
        self.__table = self.__slots = start
        self.__fields = fields
        self.__next = 0

        return parent

    def end_table(self, parent: int) -> None:
        """Ends the fields of the table that ``start_table()`` started.

        Raises ``ValueError`` if the table has fields left to set.
        """
        parents = self.__parents
        if self.__next != self.__fields or parents is None:
            raise self.__incomplete_error()

        self.__fields, self.__next, self.__parents = parents
        self.__table = parent
        # Only the root starts at 0, and only its offsets follow a header
        self.__slots = self.__header_size if parent == 0 else parent

    # The scalar setters are written out, with the offset's write, since a
    # shared helper's call made writing a Point a fifth slower. struct raises
    # _PACK_ERRORS, none of which is a ValueError, and __scalar_error() turns
    # them into ValueError, as ByteBuffer does. It's the offset that doesn't
    # fit when the table is longer than its offset type addresses.

    def set_int8(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = int8.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_uint8(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = uint8.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_int16(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = int16.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_uint16(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = uint16.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_int32(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = int32.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_uint32(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = uint32.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_int64(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = int64.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_uint64(self, index: int, value: int) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = uint64.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_float32(self, index: int, value: float) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = float32.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    def set_float64(self, index: int, value: float) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        data = self.__data
        offset = len(data) - self.__table
        try:
            packed = float64.pack(value)
            self.__length.pack_into(data, self.__slots + index * self.__size, offset)
        except _PACK_ERRORS as error:
            raise self.__scalar_error(value, offset, error) from error

        data += packed
        self.__next = index + 1

    # Each array is converted as TableBuilder converts it: a contiguous buffer
    # of the field's element type is copied as bytes, and anything else, such
    # as a list, converted as array() converts an iterable, which raises
    # OverflowError for an integer out of range and TypeError for a value of
    # another type, and converts a float past float32's range to an infinity.

    def set_int8_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "b", SIZEOF_8, values)

    def set_uint8_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "B", SIZEOF_8, values)

    def set_int16_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "h", SIZEOF_16, values)

    def set_uint16_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "H", SIZEOF_16, values)

    def set_int32_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "i", SIZEOF_32, values)

    def set_uint32_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "I", SIZEOF_32, values)

    def set_int64_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "q", SIZEOF_64, values)

    def set_uint64_array(self, index: int, values: Buffer | Iterable[int]) -> None:
        self.__set_array(index, "Q", SIZEOF_64, values)

    def set_float32_array(self, index: int, values: Buffer | Iterable[float]) -> None:
        self.__set_array(index, "f", SIZEOF_32, values)

    def set_float64_array(self, index: int, values: Buffer | Iterable[float]) -> None:
        self.__set_array(index, "d", SIZEOF_64, values)

    def set_string(self, index: int, string: str) -> None:
        """Writes ``string`` as UTF-8. Raises ``UnicodeEncodeError``, a
        ``ValueError``, if it has a lone surrogate, which UTF-8 can't encode."""
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        encoded = string.encode()

        self.__set_offset(index)
        self.__set_count(len(encoded), len(encoded))
        self.__data += encoded
        self.__next = index + 1

    def set_struct(self, index: int, data: Buffer) -> None:
        """Writes a struct field from its bytes: its scalars, little-endian, one
        after another, without padding. A ``struct.Struct`` with a ``<`` format
        packs them."""
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        self.__set_offset(index)
        self.__data += data
        self.__next = index + 1

    def set_struct_array(
        self, index: int, data: Buffer, size: int, alignment: ElementSize
    ) -> None:
        """Writes an array of structs of ``size`` bytes each, from their bytes
        one after another, padded so that they start at a multiple of
        ``alignment`` from the start of the table.

        Raises ``ValueError`` if ``size`` is less than 1, ``data`` isn't a
        whole number of structs, or ``alignment`` isn't 1, 2, 4 or 8.
        """
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        byte_length = memoryview(data).nbytes
        count = struct_count(byte_length, size)

        self.__start_array(index, count, byte_length, alignment)
        self.__data += data
        self.__next = index + 1

    def set_table(self, index: int, alignment: ElementSize) -> None:
        """Writes the offset of a nested table, padded to start at a multiple
        of ``alignment``, whose fields then follow between ``start_table()``
        and ``end_table()``.

        Raises ``ValueError`` if ``alignment`` isn't 1, 2, 4 or 8.
        """
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        self.__pad(len(self.__data), alignment)
        self.__set_offset(index)
        self.__next = index + 1

    def set_vector(self, index: int, count: int) -> int:
        """Writes the offset and count of a vector of ``count`` tables, whose
        items then follow, each between ``start_table()`` and ``end_table()``
        and then ``set_item_end()``. Returns the vector's position, which
        ``set_item_end()`` takes. The table's next field follows the last
        item's end."""
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        self.__set_offset(index)

        vector = len(self.__data)
        ends = count * self.__size
        self.__set_count(count, ends)
        self.__data += bytes(ends)

        if count == 0:
            self.__next = index + 1
        else:
            self.__vector = (vector, count, index + 1, self.__ended, self.__vector)
            self.__ended = 0
            # Until each item's end is set, no field index matches
            self.__next = None

        return vector

    def set_item_end(self, vector: int, item: int) -> None:
        """Writes the end of item ``item`` of the vector, which was just
        written.

        Raises ``ValueError`` if ``vector`` isn't the position of the vector
        whose items are being written, or ``item`` isn't the item after the
        last that ended.
        """
        # A vector is dropped with its last item's end, so the one being
        # written has items left to end
        current = self.__vector
        if current is None or vector != current[0] or item != self.__ended:
            raise self.__item_end_error(vector, item)

        data = self.__data
        try:
            self.__length.pack_into(
                data, vector + (item + 1) * self.__size, len(data) - vector
            )
        except _PACK_ERRORS as error:
            raise self.__too_long(len(data)) from error

        ended = item + 1
        if ended == current[1]:
            _, _, self.__next, self.__ended, self.__vector = current
        else:
            self.__ended = ended

    def __set_array(
        self,
        index: int,
        type_code: TypeCode,
        item_size: ElementSize,
        values: Buffer | Iterable[float],
    ) -> None:
        if index != self.__next or index >= self.__fields:
            raise self.__order_error(index)

        source = little_endian_bytes(type_code, as_elements(type_code, values))

        self.__start_array(index, len(source) // item_size, len(source), item_size)
        self.__data += source
        self.__next = index + 1

    def __start_array(
        self, index: int, count: int, byte_length: int, alignment: ElementSize
    ) -> None:
        """Pads the array so that its ``byte_length`` bytes of elements, after
        the count, are aligned, and writes its offset and count"""
        self.__pad(len(self.__data) + self.__size, alignment)
        self.__set_offset(index)
        self.__set_count(count, byte_length)

    def __pad(self, position: int, alignment: ElementSize) -> None:
        """Pads so that ``position`` lies at a multiple of ``alignment`` from
        the start of the table.

        Raises ``ValueError`` if ``alignment`` isn't 1, 2, 4 or 8.
        """
        # Checked inline rather than by padding_to(), whose call made writing
        # a vector of 100 tables that each nest another 3% slower
        if alignment not in ALIGNMENTS:
            raise alignment_error(alignment)

        padding = (self.__table - position) % alignment
        if padding:
            self.__data += _PADDING[padding]

    def __set_offset(self, index: int) -> None:
        """Writes the offset of field ``index``, which is the table's end, once
        its setter checked that it's the next"""
        data = self.__data
        try:
            self.__length.pack_into(
                data, self.__slots + index * self.__size, len(data) - self.__table
            )
        except _PACK_ERRORS as error:
            raise self.__too_long(len(data)) from error

    def __set_count(self, count: int, byte_length: int) -> None:
        """Appends a count or length, as wide as the offsets, of the
        ``byte_length`` bytes that follow it"""
        try:
            self.__data += self.__length.pack(count)
        except StructError as error:
            length = len(self.__data) + self.__size + byte_length
            raise self.__too_long(length) from error

    def __order_error(self, index: int) -> ValueError:
        """The error of field ``index`` set out of order, which a field of a
        table whose vector has items left to end is too"""
        next_index = self.__next
        fields = self.__fields

        if next_index is None:
            return self.__unended_error()

        if not 0 <= index < fields:
            return ValueError(
                f"Can't add field {describe_int(index + 1)} to a table of {fields}"
            )

        if index < next_index:
            return ValueError(f"Field {index} was already set")

        return ValueError(f"Field {next_index} wasn't set before field {index}")

    def __item_end_error(self, vector: int, item: int) -> ValueError:
        """The error of an item end that isn't the next of the vector whose
        items are being written"""
        current = self.__vector
        if current is None or vector != current[0]:
            return ValueError(
                f"No vector at {describe_int(vector)} has items left to end"
            )

        items = current[1]
        if not 0 <= item < items:
            return ValueError(
                f"Can't end item {describe_int(item)} of a vector of {items}"
            )

        if item < self.__ended:
            return ValueError(f"Item {item}'s end was already set")

        return ValueError(f"Item {self.__ended}'s end wasn't set before item {item}'s")

    def __incomplete_error(self) -> ValueError:
        """The error of a table that ended with fields left to set, or of the
        root with a table nested in it that hasn't ended"""
        next_index = self.__next

        if next_index is None:
            return self.__unended_error()

        if next_index != self.__fields:
            table = "table" if self.__parents is None else "nested table"
            return ValueError(
                f"Only {next_index} of the {table}'s {self.__fields} fields were set"
            )

        if self.__parents is None:
            return ValueError(
                "end_table() ended a table that start_table() didn't start"
            )

        return ValueError("A nested table hasn't ended")

    def __unended_error(self) -> ValueError:
        """The error of a field set, or a table ended, before the ends of its
        vector's items"""
        items = 0 if self.__vector is None else self.__vector[1]
        return ValueError(
            f"Only {self.__ended} of the vector's {items} item ends were set"
        )

    def __scalar_error(self, value: float, offset: int, error: Exception) -> ValueError:
        if offset > self.__max_length:
            return self.__too_long(len(self.__data))

        return ValueError(f"Can't write {value!r} at {len(self.__data)}: {error}")

    def __too_long(self, length: int) -> ValueError:
        """The error of a table whose fields make it ``length`` bytes, too long
        for its offset type, which an offset, count or item end then doesn't
        fit in"""
        offset_type = OFFSET_TYPES[self.__size]
        return ValueError(
            f"The table's fields make it {length} bytes, "
            f"too long for {offset_type} offsets"
        )
