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
    OffsetType,
    build_table,
    nested_in_place,
    offset_size_of,
    read_table,
    write_table,
)

NARROW_OFFSET_TYPES: tuple[OffsetType, ...] = ("uint8", "uint16")


def outcome(write: Callable[[], memoryview[int]]) -> bytes | type[Exception]:
    """The bytes that ``write`` returns, or the class of the error it raises
    for a table too long for its offset type"""
    try:
        return bytes(write())
    except ValueError as error:
        return type(error)


def fits(table: Description) -> bool:
    """Whether a table and those nested in it fit their offset types, as their
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
        nested = {"sized": False, "versioned": False, "offsetType": "uint16"}
        table: Description = {
            "layout": {"sized": True, "versioned": True, "offsetType": "uint16"},
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
    @given(tables(offset_types=NARROW_OFFSET_TYPES))
    def test_writes_the_longest_table_that_its_offsets_address_and_rejects_a_longer_one(
        self, table: Description
    ) -> None:
        table = nested_in_place(table)
        longest_length = LONGEST[table["layout"]["offsetType"]]

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

    @given(tables(offset_types=NARROW_OFFSET_TYPES), integers(0, 300))
    def test_raises_for_any_table_that_table_builder_rejects_as_too_long(
        self, table: Description, length: int
    ) -> None:
        longer = nested_in_place(with_bytes(table, length))

        self.assertEqual(
            outcome(lambda: write_table(longer)),
            outcome(lambda: build_table(longer).bytes()),
        )

    @given(sampled_from(NARROW_OFFSET_TYPES), booleans())
    def test_rejects_a_header_longer_than_the_offsets_address_as_table_builder_does(
        self, offset_type: OffsetType, sized: bool
    ) -> None:
        layout = TableLayout(sized=sized, offset_type=offset_type)
        fields = LONGEST[offset_type] // offset_size_of(offset_type) - int(sized)

        TableWriter(fields, layout)
        with self.assertRaises(ValueError):
            TableBuilder(fields + 1, layout)
        with self.assertRaises(ValueError):
            TableWriter(fields + 1, layout)

    def test_names_the_offset_type_that_the_table_is_too_long_for(self) -> None:
        for offset_type in NARROW_OFFSET_TYPES:
            with self.subTest(offset_type=offset_type):
                layout = TableLayout(offset_type=offset_type)
                longest = LONGEST[offset_type]

                # The struct's offset fits, but the table ends past the longest
                ends_too_late = TableWriter(1, layout)
                ends_too_late.set_struct(0, bytes(longest))
                length = longest + offset_size_of(offset_type)
                with self.assertRaises(ValueError) as raised:
                    ends_too_late.bytes()
                self.assertEqual(
                    str(raised.exception),
                    f"The table's fields make it {length} bytes, "
                    f"too long for {offset_type} offsets",
                )

                # The uint8's offset is past the longest table
                starts_too_late = TableWriter(2, layout)
                starts_too_late.set_struct(0, bytes(longest))
                length = longest + 2 * offset_size_of(offset_type)
                with self.assertRaises(ValueError) as raised:
                    starts_too_late.set_uint8(1, 1)
                self.assertEqual(
                    str(raised.exception),
                    f"The table's fields make it {length} bytes, "
                    f"too long for {offset_type} offsets",
                )

                # The string's length doesn't fit, so the table would end
                # after its bytes, past the longest table
                counts_too_many = TableWriter(1, layout)
                length = 2 * offset_size_of(offset_type) + longest + 1
                with self.assertRaises(ValueError) as raised:
                    counts_too_many.set_string(0, "a" * (longest + 1))
                self.assertEqual(
                    str(raised.exception),
                    f"The table's fields make it {length} bytes, "
                    f"too long for {offset_type} offsets",
                )


class TestVersions(unittest.TestCase):
    def test_rejects_the_versions_that_table_builder_rejects(self) -> None:
        versioned = TableLayout(versioned=True, offset_type="uint8")
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

    def test_converts_values_as_the_package_docstring_states(self) -> None:
        nan = float("nan")
        raised: list[tuple[str, object, type[Exception]]] = [
            ("set_uint8", 300, ValueError),
            ("set_uint8", -1, ValueError),
            ("set_uint8_array", [300], OverflowError),
            ("set_uint8", 1.5, ValueError),
            ("set_uint8", nan, ValueError),
            ("set_uint8_array", [1.5], TypeError),
            ("set_uint8_array", [nan], TypeError),
            ("set_float32", 1e300, ValueError),
            ("set_string", "\ud800", UnicodeEncodeError),
        ]
        for setter, value, error in raised:
            with self.subTest(setter=setter, value=value):
                builder = TableBuilder(1)
                with self.assertRaises(error):
                    getattr(builder, setter)(value)
                    builder.finalize()
                with self.assertRaises(error):
                    getattr(TableWriter(1), setter)(0, value)

        # A float32 array writes infinity of the value's sign, as array("f")
        # converts it, and rounds a float within range to the nearest float32
        written: list[tuple[str, object, bytes]] = [
            ("set_float32_array", [1e300, -1e300], b"\x00\x00\x80\x7f\x00\x00\x80\xff"),
            ("set_float32", 0.1, b"\xcd\xcc\xcc\x3d"),
        ]
        for setter, value, expected in written:
            with self.subTest(setter=setter, value=value):
                builder = TableBuilder(1)
                getattr(builder, setter)(value)
                writer = TableWriter(1)
                getattr(writer, setter)(0, value)
                self.assertEqual(bytes(writer.bytes()), bytes(builder.bytes()))
                self.assertTrue(bytes(writer.bytes()).endswith(expected))

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


# A setter of each kind, which writes field ``index`` of a writer
SETTERS: list[tuple[str, Callable[[TableWriter, int], object]]] = [
    ("set_uint8", lambda writer, index: writer.set_uint8(index, 1)),
    ("set_float64", lambda writer, index: writer.set_float64(index, 0.5)),
    ("set_string", lambda writer, index: writer.set_string(index, "a")),
    ("set_struct", lambda writer, index: writer.set_struct(index, b"\x01")),
    ("set_int32_array", lambda writer, index: writer.set_int32_array(index, [1])),
    (
        "set_struct_array",
        lambda writer, index: writer.set_struct_array(index, b"\x01\x02", 2, 2),
    ),
    ("set_table", lambda writer, index: writer.set_table(index, 4)),
    ("set_vector", lambda writer, index: writer.set_vector(index, 0)),
]


def two_uint8s(writer: TableWriter) -> bytes:
    """Sets fields 0 and 1 of ``writer`` to the uint8s 1 and 2, and returns its
    bytes"""
    writer.set_uint8(0, 1)
    writer.set_uint8(1, 2)
    return bytes(writer.bytes())


class TestIndices(unittest.TestCase):
    @given(
        integers().filter(lambda index: not 0 <= index < 2),
        sampled_from(SETTERS),
    )
    def test_rejects_an_index_outside_the_tables_fields_and_writes_nothing(
        self, index: int, setter: tuple[str, Callable[[TableWriter, int], object]]
    ) -> None:
        _, write = setter
        writer = TableWriter(2)

        with self.assertRaises(ValueError) as raised:
            write(writer, index)

        self.assertIn("to a table of 2", str(raised.exception))
        self.assertEqual(two_uint8s(writer), two_uint8s(TableWriter(2)))

    def test_names_the_field_past_the_tables_fields_as_tafla_rust_does(self) -> None:
        writer = TableWriter(2)
        writer.set_uint8(0, 1)
        writer.set_uint8(1, 2)

        with self.assertRaises(ValueError) as raised:
            writer.set_uint8(2, 3)
        self.assertEqual(str(raised.exception), "Can't add field 3 to a table of 2")

    def test_rejects_an_index_of_a_nested_tables_parent_inside_it(self) -> None:
        writer = TableWriter(3)
        writer.set_table(0, 1)
        parent = writer.start_table(1)

        with self.assertRaises(ValueError) as raised:
            writer.set_uint8(2, 1)
        self.assertEqual(str(raised.exception), "Can't add field 3 to a table of 1")

        writer.set_uint8(0, 1)
        writer.end_table(parent)
        writer.set_uint8(1, 2)
        writer.set_uint8(2, 3)

    def test_rejects_a_field_set_twice_or_out_of_order_and_writes_nothing(
        self,
    ) -> None:
        for name, write in SETTERS:
            with self.subTest(setter=name):
                twice = TableWriter(2)
                twice.set_uint8(0, 1)
                with self.assertRaises(ValueError) as raised:
                    write(twice, 0)
                self.assertEqual(str(raised.exception), "Field 0 was already set")
                twice.set_uint8(1, 2)

                skipped = TableWriter(2)
                with self.assertRaises(ValueError) as raised:
                    write(skipped, 1)
                self.assertEqual(
                    str(raised.exception), "Field 0 wasn't set before field 1"
                )

                self.assertEqual(bytes(twice.bytes()), two_uint8s(skipped))

    def test_raises_value_error_for_an_index_or_item_of_any_size(self) -> None:
        """Indices past the bytes, and past the C ssize_t that struct takes
        positions as, which once raised OverflowError, and past the digits
        that Python converts to a string"""
        writes = [
            *SETTERS,
            ("set_item_end", lambda writer, item: writer.set_item_end(4, item)),
            ("set_item_end", lambda writer, vector: writer.set_item_end(vector, 0)),
        ]

        for index in [16, 2**63, 2**100, 2**15000, -(2**100)]:
            for name, write in writes:
                with self.subTest(write=name, bits=index.bit_length()):
                    writer = TableWriter(1)
                    if name == "set_item_end":
                        writer.set_vector(0, 1)

                    with self.assertRaises(ValueError):
                        write(writer, index)


class TestUnsetFields(unittest.TestCase):
    def test_bytes_raises_for_a_table_with_fields_left_to_set(self) -> None:
        writer = TableWriter(3)
        writer.set_uint8(0, 1)

        with self.assertRaises(ValueError) as raised:
            writer.bytes()
        self.assertEqual(
            str(raised.exception), "Only 1 of the table's 3 fields were set"
        )

    def test_end_table_raises_for_a_nested_table_with_fields_left_to_set(
        self,
    ) -> None:
        writer = TableWriter(1)
        writer.set_table(0, 1)
        parent = writer.start_table(2)
        writer.set_uint8(0, 1)

        with self.assertRaises(ValueError) as raised:
            writer.end_table(parent)
        self.assertEqual(
            str(raised.exception), "Only 1 of the nested table's 2 fields were set"
        )

    def test_bytes_raises_for_a_nested_table_that_didnt_end(self) -> None:
        writer = TableWriter(1)
        writer.set_table(0, 1)
        writer.start_table(0)

        with self.assertRaises(ValueError) as raised:
            writer.bytes()
        self.assertEqual(str(raised.exception), "A nested table hasn't ended")

    def test_raises_for_a_vector_item_whose_end_was_never_set(self) -> None:
        def vector_of_two_items(writer: TableWriter) -> int:
            vector = writer.set_vector(0, 2)
            writer.end_table(writer.start_table(0))
            writer.set_item_end(vector, 0)
            writer.end_table(writer.start_table(0))
            return vector

        before_bytes = TableWriter(1)
        vector_of_two_items(before_bytes)
        with self.assertRaises(ValueError) as raised:
            before_bytes.bytes()
        self.assertEqual(
            str(raised.exception), "Only 1 of the vector's 2 item ends were set"
        )

        before_a_field = TableWriter(2)
        vector_of_two_items(before_a_field)
        with self.assertRaises(ValueError) as raised:
            before_a_field.set_uint8(1, 1)
        self.assertEqual(
            str(raised.exception), "Only 1 of the vector's 2 item ends were set"
        )

    def test_rejects_an_item_end_outside_the_vector_or_out_of_order(self) -> None:
        writer = TableWriter(1)
        vector = writer.set_vector(0, 2)
        writer.end_table(writer.start_table(0))

        cases = [
            (vector, 2, "Can't end item 2 of a vector of 2"),
            (vector, -1, "Can't end item -1 of a vector of 2"),
            (vector, 1, "Item 0's end wasn't set before item 1's"),
            (vector + 1, 0, f"No vector at {vector + 1} has items left to end"),
        ]
        for position, item, message in cases:
            with self.subTest(item=item, vector=position):
                with self.assertRaises(ValueError) as raised:
                    writer.set_item_end(position, item)
                self.assertEqual(str(raised.exception), message)

        writer.set_item_end(vector, 0)
        with self.assertRaises(ValueError) as raised:
            writer.set_item_end(vector, 0)
        self.assertEqual(str(raised.exception), "Item 0's end was already set")

        writer.end_table(writer.start_table(0))
        writer.set_item_end(vector, 1)
        with self.assertRaises(ValueError) as raised:
            writer.set_item_end(vector, 2)
        self.assertEqual(
            str(raised.exception), f"No vector at {vector} has items left to end"
        )

        # The rejected ends wrote nothing
        builder = TableBuilder(1)
        builder.set_vector([TableBuilder(0), TableBuilder(0)])
        self.assertEqual(bytes(writer.bytes()), bytes(builder.bytes()))


class TestFailedSetters(unittest.TestCase):
    @given(
        sampled_from(
            [
                ("set_uint8", 256),
                ("set_int8", -129),
                ("set_uint16", -1),
                ("set_int32", 2**31),
                ("set_uint64", 2**64),
                ("set_float32", 1e39),
            ]
        ),
        sampled_from(["uint8", "uint16", "uint32"]),
    )
    def test_a_scalar_that_doesnt_fit_leaves_the_table_as_it_was(
        self, case: tuple[str, float], offset_type: OffsetType
    ) -> None:
        setter, value = case
        layout = TableLayout(offset_type=offset_type)
        writer = TableWriter(2, layout)

        with self.assertRaises(ValueError):
            getattr(writer, setter)(0, value)

        # Field 0 is still unset, rather than pointing at the next value
        with self.assertRaises(ValueError) as raised:
            writer.set_uint8(1, 2)
        self.assertEqual(str(raised.exception), "Field 0 wasn't set before field 1")
        self.assertEqual(two_uint8s(writer), two_uint8s(TableWriter(2, layout)))


class TestAlignments(unittest.TestCase):
    def test_rejects_an_alignment_other_than_1_2_4_or_8_and_writes_nothing(
        self,
    ) -> None:
        for alignment in [0, 3, -4, 16, 5]:
            message = f"An alignment of {alignment} isn't 1, 2, 4 or 8 bytes"
            with self.subTest(alignment=alignment):
                writer = TableWriter(2)
                writer.set_uint8(0, 1)
                with self.assertRaises(ValueError) as raised:
                    writer.set_table(1, cast(Any, alignment))
                self.assertEqual(str(raised.exception), message)
                with self.assertRaises(ValueError) as raised:
                    writer.set_struct_array(1, b"\x01", 1, cast(Any, alignment))
                self.assertEqual(str(raised.exception), message)
                writer.set_uint8(1, 2)
                self.assertEqual(bytes(writer.bytes()), two_uint8s(TableWriter(2)))

                builder = TableBuilder(1)
                with self.assertRaises(ValueError) as raised:
                    builder.set_table(b"", cast(Any, alignment))
                self.assertEqual(str(raised.exception), message)
                with self.assertRaises(ValueError) as raised:
                    builder.set_struct_array(b"\x01", 1, cast(Any, alignment))
                self.assertEqual(str(raised.exception), message)


if __name__ == "__main__":
    unittest.main()
