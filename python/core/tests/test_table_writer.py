import unittest
from collections.abc import Callable
from typing import Any, cast
from unittest.mock import patch

from hypothesis import assume, given
from hypothesis.strategies import booleans, integers, sampled_from

from tafla import TableBuilder, TableLayout, TableWriter
from tests.descriptions import tables
from tests.tables import (
    LONGEST,
    Description,
    OffsetSize,
    build_table,
    nested_in_place,
    read_table,
    write_table,
)

NARROW_OFFSET_SIZES: tuple[OffsetSize, ...] = (1, 2)


def outcome(write: Callable[[], memoryview[int]]) -> bytes | type[Exception]:
    """The bytes that ``write`` returns, or the class of the error it raises
    for a table too long for its offsets"""
    try:
        return bytes(write())
    except ValueError as error:
        return type(error)


def fits(table: Description) -> bool:
    """Whether a table and those nested in it fit their offset sizes, as their
    builders judge"""
    return not isinstance(outcome(lambda: build_table(table).bytes()), type)


def with_bytes(table: Description, length: int) -> Description:
    """The table with a uint8[] of ``length`` zeros after its fields, which
    isn't padded"""
    field = {"type": "uint8[]", "value": [0] * length}
    return {**table, "fields": [*table["fields"], field]}


class TestTableWriter(unittest.TestCase):
    @given(tables())
    def test_writes_the_bytes_that_table_builder_builds_from_the_same_fields(
        self, table: Description
    ) -> None:
        table = nested_in_place(table)
        assume(fits(table))

        self.assertEqual(bytes(write_table(table)), bytes(build_table(table).bytes()))

    @given(tables())
    def test_writes_the_same_bytes_on_a_big_endian_host(
        self, table: Description
    ) -> None:
        table = nested_in_place(table)
        assume(fits(table))

        # Take the big-endian branches on any host. The host's own order is
        # little-endian, so the elements come out swapped from both writers.
        with patch("tafla.elements.IS_LITTLE_ENDIAN", False):
            self.assertEqual(
                bytes(write_table(table)), bytes(build_table(table).bytes())
            )

    def test_writes_a_struct_array_a_nested_table_and_a_vector_as_table_builder_pads_them(
        self,
    ) -> None:
        # A uint8 leaves a uint32[] 3 bytes short of its alignment, and a
        # uint16 a nested table of float64s 1 byte short of its own
        nested = {"sized": False, "versioned": False, "offsetSize": 2}
        table: Description = {
            "layout": {"sized": True, "versioned": True, "offsetSize": 2},
            "version": 7,
            "fields": [
                {"type": "uint8", "value": 1},
                {"type": "uint32[]", "value": [2, 3]},
                {
                    "type": "struct[]",
                    "fields": ["uint16", "float64"],
                    "items": [[4, 5]],
                },
                {
                    "type": "table",
                    "from": "builder",
                    "table": {
                        "layout": nested,
                        "fields": [{"type": "float64[]", "value": [6]}],
                    },
                },
                {
                    "type": "vector",
                    "items": [
                        {
                            "from": "builder",
                            "table": {
                                "layout": nested,
                                "fields": [{"type": "string", "value": "🦀"}],
                            },
                        }
                    ],
                },
            ],
        }

        data = write_table(table)
        self.assertEqual(bytes(data), bytes(build_table(table).bytes()))
        self.assertEqual(read_table(data, table), table)


class TestTableLength(unittest.TestCase):
    @given(tables(offset_sizes=NARROW_OFFSET_SIZES))
    def test_writes_the_longest_table_that_its_offsets_address_and_rejects_a_longer_one(
        self, table: Description
    ) -> None:
        table = nested_in_place(table)
        longest_length = LONGEST[table["layout"]["offsetSize"]]

        shortest = with_bytes(table, 0)
        assume(fits(shortest))
        room = longest_length - len(build_table(shortest).bytes())

        longest = write_table(with_bytes(table, room))
        self.assertEqual(len(longest), longest_length)
        self.assertEqual(
            bytes(longest), bytes(build_table(with_bytes(table, room)).bytes())
        )

        with self.assertRaises(ValueError):
            write_table(with_bytes(table, room + 1))

    @given(tables(offset_sizes=NARROW_OFFSET_SIZES), integers(0, 300))
    def test_raises_for_any_table_that_table_builder_rejects_as_too_long(
        self, table: Description, length: int
    ) -> None:
        longer = nested_in_place(with_bytes(table, length))

        self.assertEqual(
            outcome(lambda: write_table(longer)),
            outcome(lambda: build_table(longer).bytes()),
        )

    @given(sampled_from(NARROW_OFFSET_SIZES), booleans())
    def test_rejects_a_header_longer_than_the_offsets_address_as_table_builder_does(
        self, offset_size: OffsetSize, sized: bool
    ) -> None:
        layout = TableLayout(sized=sized, offset_size=offset_size)
        fields = LONGEST[offset_size] // offset_size - int(sized)

        TableWriter(fields, layout)
        with self.assertRaises(ValueError):
            TableBuilder(fields + 1, layout)
        with self.assertRaises(ValueError):
            TableWriter(fields + 1, layout)


class TestVersions(unittest.TestCase):
    def test_rejects_the_versions_that_table_builder_rejects(self) -> None:
        versioned = TableLayout(versioned=True, offset_size=1)
        cases: list[tuple[TableLayout, int | None, type[Exception]]] = [
            (TableLayout.DEFAULT, 1, TypeError),
            (versioned, None, TypeError),
            (versioned, 256, ValueError),
            (versioned, -1, ValueError),
        ]

        for layout, version, error in cases:
            with self.subTest(layout=layout, version=version):
                with self.assertRaises(error):
                    TableBuilder(0, cast(Any, layout), cast(Any, version))
                with self.assertRaises(error):
                    TableWriter(0, cast(Any, layout), cast(Any, version))


class TestValues(unittest.TestCase):
    def test_raises_the_errors_that_table_builder_raises_for_values_it_cant_write(
        self,
    ) -> None:
        cases: list[tuple[str, object, type[Exception]]] = [
            ("set_uint8", 256, ValueError),
            ("set_int64", 2**63, ValueError),
            ("set_float32", 1e39, ValueError),
            ("set_uint16_array", [70000], OverflowError),
            ("set_float64_array", ["x"], TypeError),
            ("set_string", "\ud800", UnicodeEncodeError),
        ]

        for setter, value, error in cases:
            with self.subTest(setter=setter, value=value):
                builder = TableBuilder(1)
                writer = TableWriter(1)
                with self.assertRaises(error):
                    getattr(builder, setter)(value)
                    builder.finalize()
                with self.assertRaises(error):
                    getattr(writer, setter)(0, value)

    def test_raises_type_error_for_a_struct_that_isnt_c_contiguous_as_table_builder_does(
        self,
    ) -> None:
        strided = memoryview(bytes(range(8)))[::2]
        cases: list[tuple[str, tuple[object, ...]]] = [
            ("set_struct", (strided,)),
            ("set_struct_array", (strided, 2, 2)),
        ]

        for setter, arguments in cases:
            with self.subTest(setter=setter):
                with self.assertRaises(TypeError):
                    getattr(TableBuilder(1), setter)(*arguments)
                with self.assertRaises(TypeError):
                    getattr(TableWriter(1), setter)(0, *arguments)


class TestIndices(unittest.TestCase):
    def test_raises_value_error_for_an_index_or_item_that_has_no_slot_in_the_bytes(
        self,
    ) -> None:
        """Indices past the bytes, and past the C ssize_t that struct takes
        positions as, which once raised OverflowError"""
        writes: list[tuple[str, Callable[[TableWriter, int], object]]] = [
            ("set_uint8", lambda writer, index: writer.set_uint8(index, 1)),
            ("set_float64", lambda writer, index: writer.set_float64(index, 0.5)),
            ("set_string", lambda writer, index: writer.set_string(index, "a")),
            ("set_struct", lambda writer, index: writer.set_struct(index, b"\x01")),
            (
                "set_int32_array",
                lambda writer, index: writer.set_int32_array(index, [1]),
            ),
            ("set_table", lambda writer, index: writer.set_table(index, 4)),
            ("set_vector", lambda writer, index: writer.set_vector(index, 0)),
            ("set_item_end", lambda writer, item: writer.set_item_end(4, item)),
            ("set_item_end", lambda writer, vector: writer.set_item_end(vector, 0)),
        ]

        for index in [16, 2**63, 2**100, 2**15000]:
            for name, write in writes:
                with self.subTest(write=name, bits=index.bit_length()):
                    writer = TableWriter(1)
                    if name == "set_item_end":
                        writer.set_vector(0, 1)

                    with self.assertRaises(ValueError):
                        write(writer, index)


if __name__ == "__main__":
    unittest.main()
