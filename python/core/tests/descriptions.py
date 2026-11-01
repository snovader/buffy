"""Hypothesis strategies of table descriptions in the shape of the conformance
suite's ``cases.json``, which ``tests/tables.py`` builds and reads.

They cover every layout, element type and kind of field: integers at their
limits, 64-bit ones included; floats of both widths with -0.0, NaN and the
infinities, and float32s that are exact; strings with characters outside the
Basic Multilingual Plane and U+FEFF, but no lone surrogates, which
``set_string()`` rejects; nested tables from builders and from bytes, with and
without an alignment; and vectors of tables of any layout.

They follow the format's rules: a table nested with ``set_table()`` has no
header, as the builder's docstring asks, and is read in place when its offsets
are as wide as its parent's; versions fit their slot; and every table fits its
offset size.
"""

import struct
import sys
from functools import cache, partial
from math import inf, nan
from typing import Any, Literal

from hypothesis import reject
from hypothesis.strategies import (
    DrawFn,
    SearchStrategy,
    booleans,
    characters,
    composite,
    fixed_dictionaries,
    floats,
    integers,
    just,
    lists,
    one_of,
    sampled_from,
    text,
)

from tests.tables import (
    ELEMENT_SIZES,
    LONGEST,
    Description,
    OffsetSize,
    from_json,
    to_json_number,
)

OFFSET_SIZES: tuple[OffsetSize, ...] = (1, 2, 4)

# Levels of tables nested below the root
MAX_DEPTH = 2
MAX_FIELDS = 6
MAX_ELEMENTS = 8
MAX_ITEMS = 3

# The most padding before a nested table, which aligns it to at most 8
_MAX_TABLE_PADDING = 7

_FLOAT32_MAX: float = struct.unpack("<f", b"\xff\xff\x7f\x7f")[0]


def _integers(bits: int, *, signed: bool) -> SearchStrategy[int]:
    """Integers of ``bits``, half of them at its limits, which Hypothesis
    otherwise draws once in a hundred"""
    low, high = (
        (-(2 ** (bits - 1)), 2 ** (bits - 1) - 1) if signed else (0, 2**bits - 1)
    )
    return one_of(integers(low, high), sampled_from([low, high]))


def _floats(width: Literal[32, 64]) -> SearchStrategy[float | str]:
    """Floats exact at ``width``, as JSON holds them, half of them the edge
    cases that Hypothesis otherwise draws less often: the zeros, the
    infinities, NaN, and the largest and smallest finite magnitudes. Any NaN is
    named, and written as the quiet NaN."""
    largest = _FLOAT32_MAX if width == 32 else sys.float_info.max
    smallest = 2.0**-149 if width == 32 else 2.0**-1074
    edges = [0.0, -0.0, inf, -inf, nan, largest, -largest, smallest, -smallest]

    return one_of(floats(width=width), sampled_from(edges)).map(to_json_number)


# The values of each scalar type, as JSON holds them
SCALAR_VALUES: dict[str, SearchStrategy[Any]] = {
    "int8": _integers(8, signed=True),
    "uint8": _integers(8, signed=False),
    "int16": _integers(16, signed=True),
    "uint16": _integers(16, signed=False),
    "int32": _integers(32, signed=True),
    "uint32": _integers(32, signed=False),
    "int64": _integers(64, signed=True).map(str),
    "uint64": _integers(64, signed=False).map(str),
    "float32": _floats(32),
    "float64": _floats(64),
}


def scalar_values(kind: str) -> SearchStrategy[Any]:
    """The values of a scalar type as tafla takes them, rather than as JSON
    holds them: ints, and floats with -0.0, NaN and the infinities, such as
    the fields of generated code take"""
    return SCALAR_VALUES[kind].map(partial(from_json, kind))


# Characters drawn more often than the default alphabet would: U+FEFF, which is
# part of a string rather than a byte order mark, and ones outside the Basic
# Multilingual Plane, which take 4 bytes of UTF-8
_RARE_CHARACTERS = ["\ufeff", "\U00010000", "\U0001f9ea", "\U0010ffff"]

_CHARACTERS = one_of(characters(codec="utf-8"), sampled_from(_RARE_CHARACTERS))

# Strings, some of them after a leading U+FEFF
STRINGS: SearchStrategy[str] = one_of(
    text(_CHARACTERS, max_size=MAX_ELEMENTS),
    text(_CHARACTERS, max_size=MAX_ELEMENTS).map(lambda string: f"\ufeff{string}"),
)

SCALAR_FIELDS: SearchStrategy[Description] = one_of(
    [
        fixed_dictionaries({"type": just(kind), "value": values})
        for kind, values in SCALAR_VALUES.items()
    ]
)

ARRAY_FIELDS: SearchStrategy[Description] = one_of(
    [
        fixed_dictionaries(
            {"type": just(f"{kind}[]"), "value": lists(values, max_size=MAX_ELEMENTS)}
        )
        for kind, values in SCALAR_VALUES.items()
    ]
)

STRING_FIELDS: SearchStrategy[Description] = fixed_dictionaries(
    {"type": just("string"), "value": STRINGS}
)


LAYOUTS: SearchStrategy[Description] = fixed_dictionaries(
    {
        "sized": booleans(),
        "versioned": booleans(),
        "offsetSize": sampled_from(OFFSET_SIZES),
    }
)


@composite
def tables(
    draw: DrawFn,
    depth: int = MAX_DEPTH,
    *,
    header: bool = True,
    offset_sizes: tuple[OffsetSize, ...] = OFFSET_SIZES,
    parent_offset_size: OffsetSize | None = None,
) -> Description:
    """Descriptions of tables with up to ``depth`` levels of tables nested
    below them. ``header=False`` leaves out the size and version slots.

    The offset size is drawn first, from ``offset_sizes`` and more often the
    parent's, so that the table's own nested tables can share it and be read in
    place. It's then widened to the next of ``offset_sizes`` if the fields
    don't fit, and the example is rejected if none fits.
    """
    sized = header and draw(booleans())
    versioned = header and draw(booleans())

    if parent_offset_size is None:
        drawn = draw(sampled_from(offset_sizes))
    else:
        drawn = draw(sampled_from((parent_offset_size, *offset_sizes)))

    fields = draw(_field_lists(depth, drawn))

    widths = sorted(size for size in offset_sizes if size >= drawn)
    layout = _fitting_layout(sized, versioned, widths, fields)
    if layout is None:
        reject()

    table: Description = {"layout": layout}
    if versioned:
        table["version"] = draw(integers(0, LONGEST[layout["offsetSize"]]))

    table["fields"] = fields
    return table


def _fitting_layout(
    sized: bool,
    versioned: bool,
    offset_sizes: list[OffsetSize],
    fields: list[Description],
) -> Description | None:
    """Returns the layout of the first of ``offset_sizes`` whose longest table
    of ``fields`` fits, or ``None`` if none fits"""
    for offset_size in offset_sizes:
        layout = {"sized": sized, "versioned": versioned, "offsetSize": offset_size}
        if longest_length({"layout": layout, "fields": fields}) <= LONGEST[offset_size]:
            return layout

    return None


# The strategies of nested tables are cached, since building them on every draw
# took most of the time that generating a table took


@cache
def _field_lists(
    depth: int, offset_size: OffsetSize
) -> SearchStrategy[list[Description]]:
    kinds = [SCALAR_FIELDS, ARRAY_FIELDS, STRING_FIELDS]
    if depth > 0:
        kinds += [
            _table_fields(depth - 1, offset_size),
            _vector_fields(depth - 1, offset_size),
        ]

    # A kind of field is drawn first, since one_of() would flatten the scalars'
    # and arrays' one_of() into a branch per type, and draw a string, table or
    # vector one time in 23
    fields = sampled_from(kinds).flatmap(lambda kind: kind)

    return lists(fields, max_size=MAX_FIELDS)


def _table_fields(depth: int, offset_size: OffsetSize) -> SearchStrategy[Description]:
    """Tables nested with ``set_table()``, without a header"""
    return fixed_dictionaries(
        {
            "type": just("table"),
            "from": sampled_from(["builder", "bytes"]),
            "table": tables(depth, header=False, parent_offset_size=offset_size),
        },
        optional={"alignment": sampled_from([1, 2, 4, 8])},
    )


def _vector_fields(depth: int, offset_size: OffsetSize) -> SearchStrategy[Description]:
    """Vectors of tables of any layout"""
    item = fixed_dictionaries(
        {
            "from": sampled_from(["builder", "bytes"]),
            "table": tables(depth, parent_offset_size=offset_size),
        }
    )
    return fixed_dictionaries(
        {"type": just("vector"), "items": lists(item, max_size=MAX_ITEMS)}
    )


def longest_length(table: Description) -> int:
    """The most bytes that a table of the description can take, from the
    format's rules, with the most padding that each array and nested table can
    need. It keeps generated tables within their offset size without building
    them."""
    layout = table["layout"]
    offset_size = layout["offsetSize"]
    fields = table["fields"]

    slots = int(layout["sized"]) + int(layout["versioned"]) + len(fields)
    values = sum(_longest_field_length(field, offset_size) for field in fields)

    return slots * offset_size + values


def _longest_field_length(field: Description, offset_size: int) -> int:
    kind = field["type"]

    if kind == "table":
        return _MAX_TABLE_PADDING + longest_length(field["table"])

    if kind == "vector":
        items = field["items"]
        lengths = sum(longest_length(item["table"]) for item in items)
        return offset_size * (1 + len(items)) + lengths

    if kind == "string":
        return offset_size + len(field["value"].encode())

    if kind.endswith("[]"):
        element_size = ELEMENT_SIZES[kind.removesuffix("[]")]
        padding = element_size - 1
        return padding + offset_size + element_size * len(field["value"])

    return ELEMENT_SIZES[kind]
