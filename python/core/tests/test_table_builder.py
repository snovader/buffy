import unittest
from array import array
from collections.abc import Callable
from struct import pack
from typing import Any, cast

from hypothesis import given
from hypothesis.strategies import integers

from buffy_core import (
    SIZEOF_8,
    SIZEOF_16,
    SIZEOF_32,
    ByteBuffer,
    TableBuilder,
    TableLayout,
)
from tests.descriptions import tables
from tests.tables import Description, OffsetSize, alignment_of, build_table

MAX_LENGTHS: list[tuple[OffsetSize, int]] = [
    (SIZEOF_8, 2**8 - 1),
    (SIZEOF_16, 2**16 - 1),
]


def nested_as_bytes(table: Description) -> Description:
    """Returns a table's description with every table nested in it, at any
    depth, given as its bytes, and nested with ``set_table()`` at the
    alignment that its builder had"""
    return {**table, "fields": [_field_as_bytes(field) for field in table["fields"]]}


def _field_as_bytes(field: Description) -> Description:
    kind = field["type"]

    if kind == "table":
        nested = nested_as_bytes(field["table"])
        return {
            **field,
            "from": "bytes",
            "alignment": alignment_of(field),
            "table": nested,
        }

    if kind == "vector":
        items = [
            {"from": "bytes", "table": nested_as_bytes(item["table"])}
            for item in field["items"]
        ]
        return {**field, "items": items}

    return field


def root_vector(buffer: ByteBuffer, index: int) -> list[bytes]:
    """Reads the items of the vector of a root field"""
    return [bytes(item) for item in buffer.get_vector(buffer.root_offset(index))]


class TestTableBuilder(unittest.TestCase):
    def test_finalizes_once_and_returns_the_same_buffer(self) -> None:
        builder = TableBuilder(1)
        builder.set_string("once")

        self.assertIs(builder.finalize(), builder.finalize())
        self.assertEqual(builder.bytes(), pack("<II", 4, 4) + b"once")

    def test_finalizes_an_empty_table(self) -> None:
        self.assertEqual(TableBuilder(0).bytes(), b"")

    def test_rejects_a_field_beyond_the_number_the_table_holds(self) -> None:
        builder = TableBuilder(1)
        builder.set_uint8(1)

        with self.assertRaisesRegex(ValueError, "Can't add field 2 to a table of 1"):
            builder.set_uint8(2)

    @given(integers(0, 4))
    def test_rejects_finalizing_before_every_field_is_set(self, count: int) -> None:
        builder = TableBuilder(count + 1)
        for i in range(count):
            builder.set_float64(i)

        with self.assertRaisesRegex(
            ValueError, f"Only {count} of the table's {count + 1} fields were set"
        ):
            builder.finalize()

    def test_rejects_setting_a_field_after_finalizing(self) -> None:
        builder = TableBuilder(0)
        builder.finalize()

        with self.assertRaises(TypeError):
            builder.set_int8(1)
        with self.assertRaises(TypeError):
            builder.set_float64_array(array("d", [1.5]))

    def test_writes_values_as_they_are_when_finalizing(self) -> None:
        values = array("f", [1, 2])
        builder = TableBuilder(1)
        builder.set_float32_array(values)
        values[0] = 3

        buffer = builder.finalize()

        self.assertEqual(
            buffer.get_float32_array(buffer.root_offset(0)).tolist(), [3, 2]
        )

    def test_keeps_a_converted_list_as_it_was_when_set(self) -> None:
        values = [1.0, 2.0]
        builder = TableBuilder(1)
        builder.set_float32_array(values)
        values.append(3.0)

        buffer = builder.finalize()

        self.assertEqual(
            buffer.get_float32_array(buffer.root_offset(0)).tolist(), [1, 2]
        )

    def test_keeps_a_vector_as_it_was_when_set(self) -> None:
        vector = [b"ab", b"c"]
        builder = TableBuilder(1)
        builder.set_vector(vector)
        vector.pop()

        buffer = builder.finalize()

        self.assertEqual(root_vector(buffer, 0), [b"ab", b"c"])


class TestChangedLengths(unittest.TestCase):
    def test_rejects_buffers_whose_length_changed_after_they_were_set(self) -> None:
        cases: list[tuple[str, Callable[[TableBuilder, Any], None], Any]] = [
            ("uint8 array", TableBuilder.set_uint8_array, bytearray(b"ab")),
            ("float64 array", TableBuilder.set_float64_array, array("d", [1.5, 2.5])),
            ("table", TableBuilder.set_table, bytearray(b"abcd")),
            ("vector item", lambda b, v: b.set_vector([v]), bytearray(b"ab")),
        ]

        for name, set_buffer, values in cases:
            for change in ["pop", "append"]:
                with self.subTest(field=name, change=change):
                    builder = TableBuilder(2)
                    set_buffer(builder, values)
                    # A different length would otherwise shift this field
                    builder.set_uint8(7)

                    original = values[:]
                    if change == "pop":
                        values.pop()
                    else:
                        values.append(0)

                    with self.assertRaisesRegex(ValueError, "Field length changed"):
                        builder.finalize()

                    values[:] = original

    def test_writes_every_field_after_a_failed_finalization(self) -> None:
        values = bytearray(b"ab")
        builder = TableBuilder(2)
        builder.set_uint8(7)
        builder.set_uint8_array(values)

        values.append(0)
        with self.assertRaises(ValueError):
            builder.finalize()

        values.pop()

        self.assertEqual(builder.bytes(), pack("<IIBI", 8, 9, 7, 2) + b"ab")


class TestVersions(unittest.TestCase):
    def test_rejects_a_version_that_its_slot_cannot_hold(self) -> None:
        cases: list[tuple[OffsetSize, int]] = [
            (SIZEOF_8, 2**8),
            (SIZEOF_16, 2**16),
            (SIZEOF_32, 2**32),
            (SIZEOF_32, -1),
        ]

        for size, version in cases:
            with (
                self.subTest(size=size, version=version),
                self.assertRaisesRegex(ValueError, "doesn't fit"),
            ):
                TableBuilder(1, TableLayout(versioned=True, offset_size=size), version)

    def test_rejects_a_version_missing_from_or_given_for_a_layout(self) -> None:
        with self.assertRaisesRegex(TypeError, "needs a version"):
            TableBuilder(1, cast(Any, TableLayout(versioned=True)))

        with self.assertRaisesRegex(TypeError, "has no version slot"):
            TableBuilder(1, cast(Any, TableLayout()), 1)

    def test_rejects_reading_the_version_of_an_unversioned_table(self) -> None:
        layout = TableLayout(sized=True)
        buffer = ByteBuffer(TableBuilder(0, layout).bytes(), layout)

        with self.assertRaises(TypeError):
            buffer.get_version()


class TestTableLength(unittest.TestCase):
    def test_rejects_a_header_longer_than_the_offsets_address(self) -> None:
        for size, max_length in MAX_LENGTHS:
            for sized in [False, True]:
                with self.subTest(size=size, sized=sized):
                    layout = TableLayout(sized=sized, offset_size=size)
                    count = max_length // size - (1 if sized else 0)

                    TableBuilder(count, layout)
                    with self.assertRaisesRegex(ValueError, "doesn't fit"):
                        TableBuilder(count + 1, layout)


class TestNestedBuilders(unittest.TestCase):
    @given(tables())
    def test_writes_nested_builders_as_their_bytes_at_the_builders_alignment(
        self, table: Description
    ) -> None:
        built = bytes(build_table(table).bytes())

        self.assertEqual(built, build_table(nested_as_bytes(table)).bytes())

    def test_rejects_a_builder_with_fields_left_to_set(self) -> None:
        nested = TableBuilder(2)
        nested.set_uint8(1)

        parent = TableBuilder(2)
        with self.assertRaisesRegex(ValueError, "Only 1 of the table's 2 fields"):
            parent.set_table(nested)
        with self.assertRaises(ValueError):
            parent.set_vector([bytes(1), nested])

        # The parent is as it was
        nested.set_uint8(2)
        parent.set_table(nested)
        parent.set_vector([nested])

        buffer = parent.finalize()
        table = buffer.root_offset(0)

        self.assertEqual(buffer.get_uint8(buffer.field_offset(table, 1)), 2)
        self.assertEqual(root_vector(buffer, 1), [nested.bytes()])

    def test_writes_a_builder_nested_several_times_the_same_each_time(self) -> None:
        nested = TableBuilder(2)
        nested.set_float64_array([1, 2])
        nested.set_string("nested")

        # Written straight into the parent, and then copied once finalized
        first = TableBuilder(2)
        first.set_table(nested)
        first.set_vector([nested, nested])
        written = bytes(first.bytes())

        own = bytes(nested.bytes())
        second = TableBuilder(2)
        second.set_table(nested)
        second.set_vector([nested, nested])

        self.assertEqual(second.bytes(), written)
        self.assertEqual(root_vector(second.finalize(), 1), [own, own])

    def test_writes_a_nested_builders_values_as_they_are_when_finalized(self) -> None:
        values = array("f", [1, 2])
        nested = TableBuilder(1)
        nested.set_float32_array(values)

        parent = TableBuilder(1)
        parent.set_table(nested)
        values[0] = 3

        buffer = parent.finalize()
        position = buffer.field_offset(buffer.root_offset(0), 0)

        self.assertEqual(buffer.get_float32_array(position).tolist(), [3, 2])


if __name__ == "__main__":
    unittest.main()
