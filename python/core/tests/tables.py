"""Builds tables from descriptions in the shape of the conformance suite's
``cases.json``, and reads tables back as descriptions. ``conformance/README.md``
at the root of the repository describes them.

The conformance test reads and rebuilds the suite's tables with them, and the
property tests build and read the descriptions that ``tests/descriptions.py``
generates, so that both test one model of tables.

Values are as JSON holds them: 64-bit integers as decimal strings, and the
floats that JSON has no number for as ``"NaN"``, ``"Infinity"``,
``"-Infinity"`` and ``"-0"``. A reader returns them as buffy reads them, as
ints and floats, and ``comparable_table()`` compares the two forms exactly.
"""

import struct
from collections.abc import Buffer, Callable
from dataclasses import dataclass
from math import copysign, isinf, isnan
from typing import Any, Literal

from buffy_core import ByteBuffer, TableBuilder, TableLayout

# A table, field, layout or vector item as cases.json describes it
type Description = dict[str, Any]

type OffsetSize = Literal[1, 2, 4]
type Alignment = Literal[1, 2, 4, 8]

# The struct format of each scalar type, which is also the type code of the
# array module and of the memoryviews that array getters return
FORMATS: dict[str, str] = {
    "int8": "b",
    "uint8": "B",
    "int16": "h",
    "uint16": "H",
    "int32": "i",
    "uint32": "I",
    "int64": "q",
    "uint64": "Q",
    "float32": "f",
    "float64": "d",
}

ELEMENT_SIZES: dict[str, int] = {
    kind: struct.calcsize(f"<{code}") for kind, code in FORMATS.items()
}

FLOAT_TYPES = ("float32", "float64")
INT64_TYPES = ("int64", "uint64")

# TableLayout.DEFAULT, as cases.json describes it
DEFAULT_LAYOUT: Description = {"sized": False, "versioned": False, "offsetSize": 4}

# The longest table that the offsets of each size address
LONGEST: dict[int, int] = {1: 0xFF, 2: 0xFFFF, 4: 0xFFFF_FFFF}

# The alignment that set_table() gives a table nested as bytes by default
BYTES_ALIGNMENT: Alignment = 4


def from_json(kind: str, value: Any) -> Any:
    """Returns a scalar as buffy takes it, from a description's value or a
    value that was read. ``float()`` parses the names of floats."""
    if kind in INT64_TYPES:
        return int(value)

    if kind in FLOAT_TYPES:
        return float(value)

    return value


def to_json_number(value: float) -> float | str:
    """Returns a float as JSON holds it, with the floats that JSON has no
    number for as their names"""
    if isnan(value):
        return "NaN"

    if isinf(value):
        return "Infinity" if value > 0 else "-Infinity"

    return "-0" if value == 0 and copysign(1, value) < 0 else value


def layout_of(layout: Description) -> TableLayout:
    return TableLayout(
        sized=layout["sized"],
        versioned=layout["versioned"],
        offset_size=layout["offsetSize"],
    )


def create_builder(table: Description) -> TableBuilder:
    """Returns the builder of a table's description, with none of its fields
    set"""
    layout = table["layout"]
    sized, offset_size = layout["sized"], layout["offsetSize"]
    fields = len(table["fields"])

    if layout["versioned"]:
        return TableBuilder(
            fields,
            TableLayout(sized=sized, versioned=True, offset_size=offset_size),
            table["version"],
        )

    return TableBuilder(fields, TableLayout(sized=sized, offset_size=offset_size))


def build_table(table: Description) -> TableBuilder:
    """Builds a table as its description sets it out"""
    builder = create_builder(table)
    for field in table["fields"]:
        set_field(builder, field)

    return builder


def set_field(builder: TableBuilder, field: Description) -> None:
    kind = field["type"]

    if kind == "table":
        builder.set_table(_build_nested(field), field.get("alignment"))
    elif kind == "vector":
        builder.set_vector([_build_nested(item) for item in field["items"]])
    elif kind.endswith("[]"):
        element = kind.removesuffix("[]")
        getattr(builder, f"set_{element}_array")(_array_values(element, field))
    else:
        getattr(builder, f"set_{kind}")(from_json(kind, field["value"]))


def _build_nested(nested: Description) -> TableBuilder | memoryview[int]:
    builder = build_table(nested["table"])
    return builder if nested["from"] == "builder" else builder.bytes()


def _array_values(element: str, field: Description) -> list[Any]:
    values = field["value"]
    if element in INT64_TYPES or element in FLOAT_TYPES:
        return [from_json(element, value) for value in values]

    return values


def alignment_of(field: Description) -> int:
    """The alignment that ``set_table()`` pads a nested table's field to: the
    one passed, or by default 4 for bytes, and for a builder the largest
    element size of its arrays and alignment of its nested tables, or 1 if it
    has neither"""
    if "alignment" in field:
        return field["alignment"]

    if field["from"] == "bytes":
        return BYTES_ALIGNMENT

    alignments = [1]
    for nested in field["table"]["fields"]:
        kind = nested["type"]
        if kind == "table":
            alignments.append(alignment_of(nested))
        elif kind.endswith("[]"):
            alignments.append(ELEMENT_SIZES[kind.removesuffix("[]")])

    return max(alignments)


def comparable_table(table: Description) -> Description:
    """Returns a table's description, or one that a reader returned, with
    every value in a form that compares exactly: 64-bit integers as ints, and
    floats as the bits of their float64, which a float32 widens to exactly, so
    that -0.0 differs from 0.0 and NaN equals the same NaN. Other values are
    ints and strings in both forms already."""
    return {**table, "fields": [comparable_field(field) for field in table["fields"]]}


def comparable_field(field: Description) -> Description:
    """Returns a field as ``comparable_table()`` returns its fields"""
    kind = field["type"]

    if kind == "table":
        return {**field, "table": comparable_table(field["table"])}

    if kind == "vector":
        items = [
            {**item, "table": comparable_table(item["table"])}
            for item in field["items"]
        ]
        return {**field, "items": items}

    if kind.endswith("[]"):
        element = kind.removesuffix("[]")
        convert = _COMPARABLE.get(element)
        if convert is None:
            return field

        return {**field, "value": [convert(value) for value in field["value"]]}

    convert = _COMPARABLE.get(kind)
    return field if convert is None else {**field, "value": convert(field["value"])}


def _float_bits(value: Any) -> str:
    """The bits of a float, or of the float that JSON names, as a float64 in
    hex, such as 7ff8000000000000 for NaN"""
    return struct.pack(">d", float(value)).hex()


# The conversions of the values that a description and a reader hold in
# different forms
_COMPARABLE: dict[str, Callable[[Any], int | str]] = {
    "int64": int,
    "uint64": int,
    "float32": _float_bits,
    "float64": _float_bits,
}


class MismatchError(AssertionError):
    """Raised by ``TableReader`` when the bytes hold a table of another shape
    than its description: a size slot that doesn't hold the table's length, or
    a vector of another number of items. It's the description's mismatch,
    rather than buffy's, which raises ``DecodeError``."""


@dataclass(frozen=True, slots=True)
class Placement:
    """Where a reader found a field: at ``position`` in ``data``, in a table
    that starts at ``table`` in ``data`` and whose offsets, counts and lengths
    are ``offset_size`` bytes wide"""

    field: Description
    data: memoryview[int]
    table: int
    position: int
    offset_size: OffsetSize


type Visit = Callable[[Placement], None]


class TableReader:
    """Reads the bytes of a table as descriptions of its fields set them out,
    and returns those descriptions with the values that were read in place of
    theirs, so that they compare equal with ``comparable_table()`` when the
    bytes hold the values.

    A nested table, from ``set_table()`` or in a vector, is read in place when
    it has no header and its offsets are as wide as its parent's. Any other is
    read as a table of its own: a vector item from its bytes, and a field from
    its position to the end of its parent, since its length isn't written.

    ``visit`` is called with the placement of every field before it's read,
    nested ones included.
    """

    __slots__ = ["__buffer", "__offset_size", "__visit"]

    __offset_size: OffsetSize

    def __init__(
        self, data: Buffer, layout: Description, visit: Visit | None = None
    ) -> None:
        self.__buffer = ByteBuffer(data, layout_of(layout))
        self.__offset_size = layout["offsetSize"]
        self.__visit = visit

    def read_table(self, table: Description) -> Description:
        """Reads the header and every field of a table from its own bytes,
        which hold that table alone. Raises ``MismatchError`` if its size slot
        doesn't hold its length."""
        buffer = self.__buffer
        if table["layout"]["sized"] and buffer.get_size() != len(buffer.bytes()):
            raise MismatchError(
                f"The size slot holds {buffer.get_size()}, "
                f"but the table has {len(buffer.bytes())} bytes"
            )

        return self.__read_header_and_fields(table)

    def read_root_field(self, index: int, field: Description) -> Description:
        return self.__read_field(0, self.__buffer.root_offset(index), field)

    def __read_header_and_fields(self, table: Description) -> Description:
        layout = table["layout"]

        read: Description = {"layout": layout}
        if layout["versioned"]:
            read["version"] = self.__buffer.get_version()

        read["fields"] = [
            self.read_root_field(index, field)
            for index, field in enumerate(table["fields"])
        ]

        return read

    def __read_field(
        self, table: int, position: int, field: Description
    ) -> Description:
        if self.__visit is not None:
            placement = Placement(
                field, self.__buffer.bytes(), table, position, self.__offset_size
            )
            self.__visit(placement)

        kind = field["type"]

        if kind == "table":
            return {**field, "table": self.__read_nested(position, field["table"])}

        if kind == "vector":
            return {**field, "items": self.__read_vector(position, field["items"])}

        if kind.endswith("[]"):
            element = kind.removesuffix("[]")
            values = getattr(self.__buffer, f"get_{element}_array")(position)
            return {**field, "value": values.tolist()}

        return {**field, "value": getattr(self.__buffer, f"get_{kind}")(position)}

    def __read_vector(
        self, position: int, items: list[Description]
    ) -> list[Description]:
        """Reads the items that the vector and its description both hold, and
        then raises ``MismatchError`` if they hold different numbers of
        items"""
        buffer = self.__buffer
        offsets = buffer.get_vector_offsets(position)
        vector = buffer.get_vector(position)

        read = [
            {**item, "table": self.__read_item(offset, item_bytes, item["table"])}
            for item, offset, item_bytes in zip(items, offsets, vector, strict=False)
        ]

        if len(vector) != len(items):
            raise MismatchError(
                f"The vector holds {len(vector)} items rather than {len(items)}"
            )

        return read

    def __read_item(
        self, position: int, item_bytes: memoryview[int], table: Description
    ) -> Description:
        if self.__reads_in_place(table):
            return self.__read_in_place(position, table)

        return TableReader(item_bytes, table["layout"], self.__visit).read_table(table)

    def __read_nested(self, position: int, table: Description) -> Description:
        """Reads a table nested with ``set_table()``"""
        if self.__reads_in_place(table):
            return self.__read_in_place(position, table)

        rest = self.__buffer.bytes()[position:]
        reader = TableReader(rest, table["layout"], self.__visit)

        return reader.__read_header_and_fields(table)

    def __reads_in_place(self, table: Description) -> bool:
        layout = table["layout"]
        return (
            not layout["sized"]
            and not layout["versioned"]
            and layout["offsetSize"] == self.__offset_size
        )

    def __read_in_place(self, position: int, table: Description) -> Description:
        buffer = self.__buffer
        return {
            "layout": table["layout"],
            "fields": [
                self.__read_field(position, buffer.field_offset(position, index), field)
                for index, field in enumerate(table["fields"])
            ],
        }


def read_table(
    data: Buffer, table: Description, visit: Visit | None = None
) -> Description:
    """Reads a table from its own bytes, as ``TableReader.read_table()`` does"""
    return TableReader(data, table["layout"], visit).read_table(table)
