"""The fields of generated messages, tables and structs in the conformance
suite's description model of ``tests/tables.py``, so that ``build_table()``
writes the same table by hand with ``TableBuilder``, as the suite builds its
tables, and ``comparable_table()`` compares what the generated code reads with
what it wrote."""

import struct
from collections.abc import Buffer, Iterable

from tests.tables import DEFAULT_LAYOUT, FORMATS, Description, from_json, to_json


def layout(
    *, sized: bool = False, versioned: bool = False, offset_size: int = 4
) -> Description:
    """A layout, as a message's attributes set it"""
    return {"sized": sized, "versioned": versioned, "offsetSize": offset_size}


def scalar(kind: str, value: float) -> Description:
    return {"type": kind, "value": to_json(kind, value)}


def typed_array(kind: str, values: Buffer | Iterable[float]) -> Description:
    """An array of ``kind``'s elements, from any values that the generated code
    takes for it"""
    elements = memoryview(values).tolist() if isinstance(values, Buffer) else values
    return {"type": f"{kind}[]", "value": [to_json(kind, value) for value in elements]}


def string(value: str) -> Description:
    return {"type": "string", "value": value}


def table(fields: list[Description]) -> Description:
    """A table nested from its builder at its default alignment, as the
    generated code nests it"""
    return {
        "type": "table",
        "from": "builder",
        "table": {"layout": DEFAULT_LAYOUT, "fields": fields},
    }


def vector(items: Iterable[list[Description]]) -> Description:
    """A vector of tables from their builders, as the generated code writes it"""
    return {
        "type": "vector",
        "items": [
            {"from": "builder", "table": {"layout": DEFAULT_LAYOUT, "fields": fields}}
            for fields in items
        ],
    }


def message(
    layout: Description, fields: list[Description], version: int | None = None
) -> Description:
    """A message's table, with its version when its layout is versioned"""
    if version is None:
        return {"layout": layout, "fields": fields}

    return {"layout": layout, "version": version, "fields": fields}


def struct_bytes(fields: list[Description]) -> bytes:
    """The bytes of a struct of scalar fields, each little-endian right after
    the one before, packed one by one rather than with the generated code's
    format"""
    packed = bytearray()
    for field in fields:
        kind = field["type"]
        packed += struct.pack(f"<{FORMATS[kind]}", from_json(kind, field["value"]))

    return bytes(packed)
