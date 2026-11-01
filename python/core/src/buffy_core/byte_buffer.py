from array import array
from collections.abc import Buffer, Iterable, Sequence
from struct import Struct, error as StructError
from typing import Any, overload

from .constants import SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64
from .elements import as_elements, from_little_endian, little_endian_bytes
from .errors import DecodeError, out_of_range
from .layout import TableLayout
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
from .types import AddressableSpace, FloatTypeCode, IntTypeCode, TypeCode

# The unsigned integer of each offset size, which lengths, offsets and header
# slots are written as
_SIZES: dict[AddressableSpace, Struct] = {
    SIZEOF_8: uint8,
    SIZEOF_16: uint16,
    SIZEOF_32: uint32,
}


class ByteBuffer:
    """Reads and writes within the supplied bytes, in the table layout it was
    created with, storing multi-byte values little-endian. Array reads return
    views of the bytes on a little-endian host, whatever their alignment, and
    copies in the host's byte order on a big-endian host.

    Every read checks that it stays within the bytes and raises ``DecodeError``
    otherwise. A write that doesn't fit in the bytes, or a value that doesn't
    fit its type, raises ``ValueError`` and writes nothing. That includes
    lengths, offsets and versions too large for the layout's ``offset_size``,
    which ``TableBuilder`` rejects before it writes a table.
    """

    __slots__ = [
        "__header_size",
        "__length",
        "__position",
        "__size",
        "__size_struct",
        "__sized",
        "__versioned",
        "__view",
    ]

    __size: AddressableSpace

    def __init__(
        self, bytes: Buffer, layout: TableLayout = TableLayout.DEFAULT
    ) -> None:
        view = memoryview(bytes)
        self.__view = view if view.format == "B" and view.ndim == 1 else view.cast("B")
        self.__length = len(self.__view)

        self.__sized = layout.sized()
        self.__versioned = layout.versioned()
        # The width of offsets, lengths and header slots
        self.__size = layout.offset_size()
        self.__size_struct = _SIZES[self.__size]
        # The byte length of the header slots before the root's field offsets
        self.__header_size = layout.header_size()

        self.__position = 0

    def reset(self) -> None:
        self.__position = 0

    def bytes(self) -> memoryview:
        return self.__view

    def get_size(self) -> int:
        return self.__get_size(0) if self.__sized else self.__length

    def set_size(self, size: int) -> None:
        self.__set_size(size)

    def get_version(self) -> int:
        """Returns the version that the table's header carries after its
        length. It's as wide as the header's other slots.

        Raises ``TypeError`` if the buffer's layout isn't versioned.
        """
        if not self.__versioned:
            raise TypeError("The table has no version")

        return self.__get_size(self.__size if self.__sized else 0)

    def set_version(self, version: int) -> None:
        self.__set_size(version)

    def root_offset(self, index: int) -> int:
        """Returns the position of field ``index`` of the root table."""
        return self.__get_size(self.__header_size + index * self.__size)

    def field_offset(self, table_position: int, index: int) -> int:
        """Returns the position of field ``index`` of a nested table, which has
        no header and stores its field offsets from its own start.
        ``table_position`` is the nested table's position, which is the offset
        of the field that holds it.
        """
        return table_position + self.__get_size(table_position + index * self.__size)

    def set_offset(self, offset: int) -> None:
        self.__set_size(offset)

    # The scalar getters are written out, since a shared helper's call made
    # each read a quarter slower. struct checks the end and raises struct.error,
    # which isn't a ValueError, and counts a negative position from the end,
    # so both fall through to a DecodeError.

    def get_int8(self, position: int) -> int:
        try:
            if position >= 0:
                return int8.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_8, self.__length)

    def get_uint8(self, position: int) -> int:
        try:
            if position >= 0:
                return uint8.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_8, self.__length)

    def get_int16(self, position: int) -> int:
        try:
            if position >= 0:
                return int16.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_16, self.__length)

    def get_uint16(self, position: int) -> int:
        try:
            if position >= 0:
                return uint16.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_16, self.__length)

    def get_int32(self, position: int) -> int:
        try:
            if position >= 0:
                return int32.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_32, self.__length)

    def get_uint32(self, position: int) -> int:
        try:
            if position >= 0:
                return uint32.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_32, self.__length)

    def get_int64(self, position: int) -> int:
        try:
            if position >= 0:
                return int64.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_64, self.__length)

    def get_uint64(self, position: int) -> int:
        try:
            if position >= 0:
                return uint64.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_64, self.__length)

    def get_float32(self, position: int) -> float:
        try:
            if position >= 0:
                return float32.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_32, self.__length)

    def get_float64(self, position: int) -> float:
        try:
            if position >= 0:
                return float64.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, SIZEOF_64, self.__length)

    def get_int8_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``, as a view of the buffer."""
        return self.__get_array("b", SIZEOF_8, position)

    def get_uint8_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``, as a view of the buffer."""
        length = self.__get_size(position)
        start = self.__check_range(position + self.__size, length)

        return self.__view[start : start + length]

    def get_int16_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("h", SIZEOF_16, position)

    def get_uint16_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("H", SIZEOF_16, position)

    def get_int32_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("i", SIZEOF_32, position)

    def get_uint32_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("I", SIZEOF_32, position)

    def get_int64_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("q", SIZEOF_64, position)

    def get_uint64_array(self, position: int) -> memoryview[int]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("Q", SIZEOF_64, position)

    def get_float32_array(self, position: int) -> memoryview[float]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("f", SIZEOF_32, position)

    def get_float64_array(self, position: int) -> memoryview[float]:
        """Reads the array at ``position``: a view of the buffer on a
        little-endian host, and a copy on a big-endian host."""
        return self.__get_array("d", SIZEOF_64, position)

    @overload
    def __get_array(
        self, type_code: IntTypeCode, item_size: int, position: int
    ) -> memoryview[int]: ...

    @overload
    def __get_array(
        self, type_code: FloatTypeCode, item_size: int, position: int
    ) -> memoryview[float]: ...

    def __get_array(self, type_code: TypeCode, item_size: int, position: int) -> Any:
        byte_length = self.__get_size(position) * item_size
        start = self.__check_range(position + self.__size, byte_length)

        return from_little_endian(type_code, self.__view[start : start + byte_length])

    def set_int8(self, value: int) -> None:
        self.__pack(int8, value)
        self.__position += SIZEOF_8

    def set_uint8(self, value: int) -> None:
        self.__pack(uint8, value)
        self.__position += SIZEOF_8

    def set_int16(self, value: int) -> None:
        self.__pack(int16, value)
        self.__position += SIZEOF_16

    def set_uint16(self, value: int) -> None:
        self.__pack(uint16, value)
        self.__position += SIZEOF_16

    def set_int32(self, value: int) -> None:
        self.__pack(int32, value)
        self.__position += SIZEOF_32

    def set_uint32(self, value: int) -> None:
        self.__pack(uint32, value)
        self.__position += SIZEOF_32

    def set_int64(self, value: int) -> None:
        self.__pack(int64, value)
        self.__position += SIZEOF_64

    def set_uint64(self, value: int) -> None:
        self.__pack(uint64, value)
        self.__position += SIZEOF_64

    def set_float32(self, value: float) -> None:
        self.__pack(float32, value)
        self.__position += SIZEOF_32

    def set_float64(self, value: float) -> None:
        self.__pack(float64, value)
        self.__position += SIZEOF_64

    def __pack(self, struct: Struct, value: float) -> None:
        """Writes ``value`` at the position. struct checks the range and the
        value before it writes, and raises struct.error, which isn't a
        ValueError, or OverflowError for a float too large for a float32."""
        try:
            struct.pack_into(self.__view, self.__position, value)
        except (StructError, OverflowError) as error:
            raise ValueError(
                f"Can't write {value!r} at {self.__position}: {error}"
            ) from error

    def set_int8_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        int8s, such as ``array("b")``, is copied as bytes; anything else is
        converted as ``array("b", values)`` would."""
        self.__set_array("b", SIZEOF_8, values)

    def set_uint8_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. Bytes, or any other
        contiguous buffer of uint8s, are copied as they are; anything else is
        converted as ``array("B", values)`` would."""
        self.__set_array("B", SIZEOF_8, values)

    def set_int16_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        int16s in the host's byte order, such as ``array("h")``, is copied as
        bytes; anything else is converted as ``array("h", values)`` would."""
        self.__set_array("h", SIZEOF_16, values)

    def set_uint16_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        uint16s in the host's byte order, such as ``array("H")``, is copied as
        bytes; anything else is converted as ``array("H", values)`` would."""
        self.__set_array("H", SIZEOF_16, values)

    def set_int32_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        int32s in the host's byte order, such as ``array("i")``, is copied as
        bytes; anything else is converted as ``array("i", values)`` would."""
        self.__set_array("i", SIZEOF_32, values)

    def set_uint32_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        uint32s in the host's byte order, such as ``array("I")``, is copied as
        bytes; anything else is converted as ``array("I", values)`` would."""
        self.__set_array("I", SIZEOF_32, values)

    def set_int64_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        int64s in the host's byte order, such as ``array("q")``, is copied as
        bytes; anything else is converted as ``array("q", values)`` would."""
        self.__set_array("q", SIZEOF_64, values)

    def set_uint64_array(self, values: Buffer | Iterable[int]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        uint64s in the host's byte order, such as ``array("Q")``, is copied as
        bytes; anything else is converted as ``array("Q", values)`` would."""
        self.__set_array("Q", SIZEOF_64, values)

    def set_float32_array(self, values: Buffer | Iterable[float]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        float32s in the host's byte order, such as ``array("f")``, is copied as
        bytes; anything else is converted as ``array("f", values)`` would."""
        self.__set_array("f", SIZEOF_32, values)

    def set_float64_array(self, values: Buffer | Iterable[float]) -> None:
        """Writes the length and then the elements. A contiguous buffer of
        float64s in the host's byte order, such as ``array("d")``, is copied as
        bytes; anything else is converted as ``array("d", values)`` would."""
        self.__set_array("d", SIZEOF_64, values)

    def __set_array(
        self, type_code: TypeCode, item_size: int, values: Buffer | Iterable[float]
    ) -> None:
        source = little_endian_bytes(type_code, as_elements(type_code, values))
        self.__check_writable(self.__size + len(source))

        self.__set_size(len(source) // item_size)
        self.__write(source)

    def set_bytes(self, bytes: Buffer) -> None:
        """Writes ``bytes`` as they are, without a length prefix."""
        # Checked and written inline, since TableBuilder writes every table
        # nested as bytes with it
        source = _byte_view(bytes)
        end = self.__position + len(source)
        if end > self.__length:
            raise self.__overflow(len(source))

        self.__view[self.__position : end] = source
        self.__position = end

    def set_padding(self, byte_length: int) -> None:
        """Writes ``byte_length`` zeros, such as to align the field after them."""
        self.__check_writable(byte_length)

        self.__write(memoryview(bytes(byte_length)))

    def get_string(self, position: int) -> str:
        """Raises ``DecodeError`` if the string isn't valid UTF-8."""
        length = self.__get_size(position)
        start = self.__check_range(position + self.__size, length)

        try:
            return str(self.__view[start : start + length], "utf-8")
        except UnicodeDecodeError as error:
            raise DecodeError(f"The string at {position} isn't valid UTF-8") from error

    def set_string(self, encoded: Buffer) -> None:
        """Writes a string encoded as UTF-8, after its length."""
        source = _byte_view(encoded)
        self.__check_writable(self.__size + len(source))

        self.__set_size(len(source))
        self.__write(source)

    def get_vector_offsets(self, position: int) -> array[int]:
        size = self.__size
        count = self.__get_size(position)
        # Every vector item needs at least its size prefix. Validate before
        # allocating the offset array from an untrusted count.
        self.__check_range(position + size, count * size)
        offsets = array("I", bytes(count * SIZEOF_32))

        offset = position + size
        for i in range(count):
            length = self.__get_size(offset)
            self.__check_range(offset + size, length)
            offsets[i] = offset + size
            offset += size + length

        return offsets

    def get_vector(self, position: int) -> list[memoryview[int]]:
        offsets = self.get_vector_offsets(position)
        return [self.get_uint8_array(offset - self.__size) for offset in offsets]

    def set_vector(self, vector: Sequence[Buffer]) -> None:
        """Writes the number of items and then each item after its length."""
        sources = [_byte_view(item) for item in vector]
        self.__check_writable(
            self.__size * (1 + len(sources)) + sum(len(source) for source in sources)
        )

        self.__set_size(len(sources))
        for source in sources:
            self.__set_size(len(source))
            self.__write(source)

    def __write(self, source: memoryview) -> None:
        end = self.__position + len(source)
        self.__view[self.__position : end] = source
        self.__position = end

    def __check_range(self, position: int, byte_length: int) -> int:
        """Returns ``position`` if ``byte_length`` bytes from it lie within the
        buffer, and raises ``DecodeError`` otherwise."""
        if position < 0 or position + byte_length > self.__length:
            raise out_of_range(position, byte_length, self.__length)

        return position

    def __check_writable(self, byte_length: int) -> None:
        if self.__position + byte_length > self.__length:
            raise self.__overflow(byte_length)

    def __overflow(self, byte_length: int) -> ValueError:
        return ValueError(
            f"Write exceeds the supplied buffer: {byte_length} bytes at "
            f"{self.__position} of {self.__length}"
        )

    def __get_size(self, position: int) -> int:
        try:
            if position >= 0:
                return self.__size_struct.unpack_from(self.__view, position)[0]
        except StructError:
            pass
        raise out_of_range(position, self.__size, self.__length)

    def __set_size(self, value: int) -> None:
        self.__pack(self.__size_struct, value)
        self.__position += self.__size


def _byte_view(buffer: Buffer) -> memoryview:
    """Returns the bytes of any buffer, as a one-dimensional view"""
    view = memoryview(buffer)
    return view if view.format == "B" and view.ndim == 1 else view.cast("B")
