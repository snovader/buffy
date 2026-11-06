from typing import ClassVar, Literal, cast, overload, override

from .types import OffsetSize


class TableLayout[Versioned: bool = bool]:
    """The header and offset size of a table. A format creates its layout once
    and passes it to both the ``TableBuilder`` that writes the table and the
    ``ByteBuffer`` that reads it, so that the two can't disagree. Nested tables
    use ``TableLayout.DEFAULT``.

    - ``sized``: whether the header starts with the table's total byte length.
    - ``versioned``: whether the header has a slot for a version, after the
      length when ``sized``. Readers can check it before reading any field.
    - ``offset_size``: the offset size, the width in bytes of the header slots,
      field offsets, counts, lengths and item ends, which limits the table's
      length: 1, of ``uint8`` offsets, for up to 255 bytes, 2, of ``uint16``,
      for 65,535 and 4, of ``uint32``, the default, for 4 GiB.

    ``Versioned`` lets type checkers require a version where a versioned table
    is built.

    Raises ``ValueError`` if ``offset_size`` isn't 1, 2 or 4.
    """

    __slots__ = ["__header_size", "__offset_size", "__sized", "__versioned"]

    DEFAULT: ClassVar[TableLayout[Literal[False]]]
    """No header slots, and offsets and lengths of the offset type ``uint32``"""

    __versioned: Versioned

    @overload
    def __init__(
        self: TableLayout[Literal[False]],
        *,
        sized: bool = False,
        versioned: Literal[False] = False,
        offset_size: OffsetSize = 4,
    ) -> None: ...

    @overload
    def __init__(
        self: TableLayout[Literal[True]],
        *,
        sized: bool = False,
        versioned: Literal[True],
        offset_size: OffsetSize = 4,
    ) -> None: ...

    @overload
    def __init__(
        self: TableLayout[bool],
        *,
        sized: bool = False,
        versioned: bool,
        offset_size: OffsetSize = 4,
    ) -> None: ...

    def __init__(
        self,
        *,
        sized: bool = False,
        versioned: bool = False,
        offset_size: OffsetSize = 4,
    ) -> None:
        if offset_size not in (1, 2, 4):
            raise ValueError(f"Offsets can't be {offset_size} bytes wide")

        self.__sized = sized
        self.__versioned = cast(Versioned, versioned)
        self.__offset_size: OffsetSize = offset_size
        header_slots = (1 if sized else 0) + (1 if versioned else 0)
        self.__header_size = header_slots * offset_size

    @override
    def __repr__(self) -> str:
        return (
            f"TableLayout(sized={self.__sized}, versioned={self.__versioned}, "
            f"offset_size={self.__offset_size})"
        )

    def sized(self) -> bool:
        return self.__sized

    def versioned(self) -> Versioned:
        return self.__versioned

    def offset_size(self) -> OffsetSize:
        return self.__offset_size

    def header_size(self) -> int:
        """The byte length of the header slots before the field offsets"""
        return self.__header_size


TableLayout.DEFAULT = TableLayout()


# The longest table that offsets and lengths of each size address
MAX_LENGTHS: dict[OffsetSize, int] = {
    1: 0xFF,
    2: 0xFFFF,
    4: 0xFFFF_FFFF,
}


def header_length(fields: int, layout: TableLayout, version: int | None) -> int:
    """Returns the byte length of the header of a table of ``fields`` fields in
    ``layout``, whose writer puts ``version`` in the header.

    Raises ``TypeError`` if ``version`` is given for a layout without a version
    slot, or missing for one with it. Raises ``ValueError`` if ``version``
    doesn't fit in the layout's offset size, or the header alone is longer than
    the layout addresses.
    """
    size = layout.offset_size()
    max_length = MAX_LENGTHS[size]
    versioned = layout.versioned()

    if versioned != (version is not None):
        raise TypeError(
            "A versioned layout needs a version"
            if versioned
            else "The layout has no version slot"
        )

    if version is not None and not 0 <= version <= max_length:
        raise ValueError(f"Version {version} doesn't fit in a {size}-byte slot")

    length = layout.header_size() + fields * size
    if length > max_length:
        raise ValueError(
            f"A header of {fields} fields doesn't fit in {size}-byte offsets"
        )

    return length
