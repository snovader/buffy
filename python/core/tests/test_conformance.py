"""Reads and rebuilds the tables of buffy's conformance suite, which the
TypeScript runtime writes. See ``conformance/README.md`` at the root of the
repository for the format of their descriptions in ``cases.json``."""

import json
import unittest
from collections.abc import Buffer
from math import copysign, isinf, isnan
from pathlib import Path
from typing import Any

from buffy_core import ByteBuffer, DecodeError, TableBuilder, TableLayout
from tests.fields import read_field, set_field

CONFORMANCE = Path(__file__).parents[3] / "conformance"

# A table, field or layout as cases.json describes it
type Description = dict[str, Any]

SUITE: Description = json.loads(
    (CONFORMANCE / "cases.json").read_text(encoding="utf-8")
)
VALID_CASES: dict[str, Description] = {case["name"]: case for case in SUITE["valid"]}


def read_bytes(file: str) -> bytes:
    return (CONFORMANCE / file).read_bytes()


def build_table(table: Description) -> TableBuilder:
    """Builds a table as its description sets it out"""
    layout = table["layout"]
    sized, offset_size = layout["sized"], layout["offsetSize"]
    fields = table["fields"]

    if layout["versioned"]:
        builder = TableBuilder(
            len(fields),
            TableLayout(sized=sized, versioned=True, offset_size=offset_size),
            table["version"],
        )
    else:
        builder = TableBuilder(
            len(fields), TableLayout(sized=sized, offset_size=offset_size)
        )

    for field in fields:
        _set_field(builder, field)

    return builder


def _set_field(builder: TableBuilder, field: Description) -> None:
    kind = field["type"]

    if kind == "table":
        builder.set_table(_build_nested(field), field.get("alignment"))
    elif kind == "vector":
        builder.set_vector([_build_nested(item) for item in field["items"]])
    elif kind.endswith("[]"):
        element = kind.removesuffix("[]")
        values = [_from_json(element, value) for value in field["value"]]
        set_field(builder, (f"{element}_array", values))
    else:
        set_field(builder, (kind, _from_json(kind, field["value"])))


def _build_nested(nested: Description) -> TableBuilder | memoryview[int]:
    builder = build_table(nested["table"])
    return builder if nested["from"] == "builder" else builder.bytes()


def _from_json(kind: str, value: Any) -> Any:
    """Returns a value as buffy takes it from the JSON that holds 64-bit
    integers as strings and floats that JSON has no number for as names, which
    ``float()`` parses"""
    if kind in ("int64", "uint64"):
        return int(value)

    if kind in ("float32", "float64"):
        return float(value)

    return value


def _to_json(kind: str, value: Any) -> Any:
    if kind in ("int64", "uint64"):
        return str(value)

    if kind in ("float32", "float64"):
        return _json_float(value)

    return value


def _json_float(value: float) -> float | str:
    if isnan(value):
        return "NaN"

    if isinf(value):
        return "Infinity" if value > 0 else "-Infinity"

    return "-0" if value == 0 and copysign(1, value) < 0 else value


class TableReader:
    """Reads the bytes of a table as descriptions of its fields set them out,
    and returns those descriptions with the values that were read in place of
    theirs, so that they equal the descriptions when the bytes hold the values.

    A nested table, from ``set_table()`` or in a vector, is read in place when
    it has no header and its offsets are as wide as its parent's. Any other is
    read as a table of its own: a vector item from its bytes, and a field from
    its position to the end of its parent.
    """

    __slots__ = ["__buffer", "__offset_size"]

    def __init__(self, data: Buffer, layout: Description) -> None:
        self.__buffer = ByteBuffer(
            data,
            TableLayout(
                sized=layout["sized"],
                versioned=layout["versioned"],
                offset_size=layout["offsetSize"],
            ),
        )
        self.__offset_size = layout["offsetSize"]

    def read_table(self, table: Description) -> Description:
        """Reads the header and every field of the table, which must have the
        reader's layout. Raises ``AssertionError`` if the size slot doesn't
        hold the table's length."""
        buffer = self.__buffer
        layout = table["layout"]

        if layout["sized"] and buffer.get_size() != len(buffer.bytes()):
            raise AssertionError(
                f"The size slot holds {buffer.get_size()}, "
                f"but the table has {len(buffer.bytes())} bytes"
            )

        read: Description = {"layout": layout}
        if layout["versioned"]:
            read["version"] = buffer.get_version()

        read["fields"] = [
            self.read_root_field(index, field)
            for index, field in enumerate(table["fields"])
        ]

        return read

    def read_root_field(self, index: int, field: Description) -> Description:
        return self.__read_field(self.__buffer.root_offset(index), field)

    def __read_field(self, position: int, field: Description) -> Description:
        kind = field["type"]

        if kind == "table":
            return {**field, "table": self.__read_nested(position, field["table"])}

        if kind == "vector":
            return {**field, "items": self.__read_vector(position, field["items"])}

        if kind.endswith("[]"):
            element = kind.removesuffix("[]")
            values = read_field(self.__buffer, f"{element}_array", position)
            return {**field, "value": [_to_json(element, value) for value in values]}

        value = read_field(self.__buffer, kind, position)
        return {**field, "value": _to_json(kind, value)}

    def __read_vector(
        self, position: int, items: list[Description]
    ) -> list[Description]:
        """Raises ``AssertionError`` if the vector holds another number of
        items."""
        offsets = self.__buffer.get_vector_offsets(position)
        if len(offsets) != len(items):
            raise AssertionError(
                f"The vector holds {len(offsets)} items rather than {len(items)}"
            )

        vector = self.__buffer.get_vector(position)

        return [
            {**item, "table": self.__read_item(offset, item_bytes, item["table"])}
            for item, offset, item_bytes in zip(items, offsets, vector, strict=True)
        ]

    def __read_item(
        self, position: int, item_bytes: memoryview[int], table: Description
    ) -> Description:
        if self.__reads_in_place(table):
            return self.__read_in_place(position, table)

        return read_table(item_bytes, table)

    def __read_nested(self, position: int, table: Description) -> Description:
        """Reads a table nested with ``set_table()``"""
        if self.__reads_in_place(table):
            return self.__read_in_place(position, table)

        # Its length isn't written, so it's read up to the end of its parent
        return read_table(self.__buffer.bytes()[position:], table)

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
                self.__read_field(buffer.field_offset(position, index), field)
                for index, field in enumerate(table["fields"])
            ],
        }


def read_table(data: Buffer, table: Description) -> Description:
    """Reads a table from its own bytes, as ``TableReader.read_table()`` does"""
    return TableReader(data, table["layout"]).read_table(table)


class TestConformance(unittest.TestCase):
    def test_reads_every_valid_table(self) -> None:
        for case in SUITE["valid"]:
            with self.subTest(case["name"]):
                table = case["table"]
                self.assertEqual(read_table(read_bytes(case["file"]), table), table)

    def test_rebuilds_every_valid_table_byte_for_byte(self) -> None:
        for case in SUITE["valid"]:
            with self.subTest(case["name"]):
                built = bytes(build_table(case["table"]).bytes())
                self.assertEqual(built, read_bytes(case["file"]))

    def test_reads_invalid_tables_up_to_the_field_that_raises(self) -> None:
        for case in SUITE["invalid"]:
            with self.subTest(case["name"]):
                fields = VALID_CASES[case["case"]]["table"]["fields"]
                layout = VALID_CASES[case["case"]]["table"]["layout"]
                reader = TableReader(read_bytes(case["file"]), layout)

                for index in range(case["field"]):
                    read = reader.read_root_field(index, fields[index])
                    self.assertEqual(read, fields[index])

                with self.assertRaises(DecodeError):
                    reader.read_root_field(case["field"], fields[case["field"]])

    def test_finds_valid_and_invalid_cases(self) -> None:
        self.assertGreater(len(SUITE["valid"]), 0)
        self.assertGreater(len(SUITE["invalid"]), 0)
