"""Strategies for table fields of every type, and helpers that set and read them
by type, such as ``set_field(builder, ("float32_array", values))``."""

from array import array
from collections.abc import Sequence
from typing import Any

from hypothesis.strategies import (
    SearchStrategy,
    binary,
    floats,
    integers,
    just,
    lists,
    one_of,
    text,
    tuples,
)

from buffy_core import ByteBuffer, TableBuilder
from buffy_core.types import TypeCode

type Field = tuple[str, Any]


def _integers(bits: int, *, signed: bool) -> SearchStrategy[int]:
    if signed:
        return integers(-(2 ** (bits - 1)), 2 ** (bits - 1) - 1)

    return integers(0, 2**bits - 1)


# NaN is left out, since it doesn't compare equal to itself
_SCALARS: dict[str, SearchStrategy[Any]] = {
    "int8": _integers(8, signed=True),
    "uint8": _integers(8, signed=False),
    "int16": _integers(16, signed=True),
    "uint16": _integers(16, signed=False),
    "int32": _integers(32, signed=True),
    "uint32": _integers(32, signed=False),
    "int64": _integers(64, signed=True),
    "uint64": _integers(64, signed=False),
    "float32": floats(width=32, allow_nan=False),
    "float64": floats(allow_nan=False),
}

# The array module's type code of each array field, and its element size
TYPE_CODES: dict[str, TypeCode] = {
    "int8_array": "b",
    "uint8_array": "B",
    "int16_array": "h",
    "uint16_array": "H",
    "int32_array": "i",
    "uint32_array": "I",
    "int64_array": "q",
    "uint64_array": "Q",
    "float32_array": "f",
    "float64_array": "d",
}

ELEMENT_SIZES: dict[str, int] = {
    kind: array(type_code).itemsize for kind, type_code in TYPE_CODES.items()
}


def _array_of(kind: str) -> SearchStrategy[array[Any]]:
    type_code = TYPE_CODES[kind]
    elements = _SCALARS[kind.removesuffix("_array")]

    return lists(elements, max_size=16).map(lambda values: array(type_code, values))


typed_arrays: SearchStrategy[Field] = one_of(
    [tuples(just(kind), _array_of(kind)) for kind in TYPE_CODES]
)

fields: SearchStrategy[Field] = one_of(
    *(tuples(just(kind), values) for kind, values in _SCALARS.items()),
    typed_arrays,
    tuples(just("string"), text(max_size=16)),
    tuples(just("vector"), lists(binary(max_size=8), max_size=4)),
)

KINDS = [*_SCALARS, *TYPE_CODES, "string", "vector"]


def set_field(builder: TableBuilder, field: Field) -> None:
    kind, value = field
    getattr(builder, f"set_{kind}")(value)


def builder_of(entries: Sequence[Field], more: int = 0, **kwargs: Any) -> TableBuilder:
    """Returns a builder with ``entries`` set and ``more`` fields left to set"""
    builder = TableBuilder(len(entries) + more, **kwargs)
    for entry in entries:
        set_field(builder, entry)

    return builder


def read_field(buffer: ByteBuffer, kind: str, position: int) -> Any:
    """Reads a field of ``kind`` at ``position``, with arrays as lists and
    vector items as bytes, so that they compare with ``expected()``"""
    value = getattr(buffer, f"get_{kind}")(position)

    if kind == "vector":
        return [bytes(item) for item in value]

    return value.tolist() if isinstance(value, memoryview) else value


def read_root_field(buffer: ByteBuffer, kind: str, index: int) -> Any:
    return read_field(buffer, kind, buffer.root_offset(index))


def expected(field: Field) -> Any:
    """Returns the value of a field as ``read_field()`` reads it back"""
    kind, value = field

    if kind == "vector":
        return [bytes(item) for item in value]

    return list(value) if kind in TYPE_CODES else value


def alignment_of(entries: Sequence[Field]) -> int:
    """The largest element size of the entries' arrays, or 1 without any"""
    return max((ELEMENT_SIZES.get(kind, 1) for kind, _ in entries), default=1)
