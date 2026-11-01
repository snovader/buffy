import unittest
from array import array
from collections.abc import Callable
from struct import pack
from typing import Any, cast

from hypothesis import given
from hypothesis.strategies import integers, lists

from buffy_core import (
    SIZEOF_8,
    SIZEOF_16,
    SIZEOF_32,
    ByteBuffer,
    TableBuilder,
    TableLayout,
)
from buffy_core.types import AddressableSpace
from tests.fields import (
    Field,
    alignment_of,
    builder_of,
    expected,
    fields,
    read_field,
    read_root_field,
    set_field,
)

MAX_LENGTHS: list[tuple[AddressableSpace, int]] = [
    (SIZEOF_8, 2**8 - 1),
    (SIZEOF_16, 2**16 - 1),
]


class TestTableBuilder(unittest.TestCase):
    def test_writes_the_header_offsets_alignment_and_vector(self) -> None:
        builder = TableBuilder(3, TableLayout(sized=True))
        builder.set_uint8(7)
        builder.set_float64_array(array("d", [1.5, -2.25]))
        builder.set_vector([b"ab", b"c"])

        table = (
            pack("<IIIIB3xI2dII", 55, 16, 20, 40, 7, 2, 1.5, -2.25, 2, 2)
            + b"ab"
            + pack("<I", 1)
            + b"c"
        )

        self.assertEqual(builder.bytes(), table)

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

        self.assertEqual(read_root_field(buffer, "float32_array", 0), [3, 2])

    def test_keeps_a_converted_list_as_it_was_when_set(self) -> None:
        values = [1.0, 2.0]
        builder = TableBuilder(1)
        builder.set_float32_array(values)
        values.append(3.0)

        buffer = builder.finalize()

        self.assertEqual(read_root_field(buffer, "float32_array", 0), [1, 2])

    def test_keeps_a_vector_as_it_was_when_set(self) -> None:
        vector = [b"ab", b"c"]
        builder = TableBuilder(1)
        builder.set_vector(vector)
        vector.pop()

        buffer = builder.finalize()

        self.assertEqual(read_root_field(buffer, "vector", 0), [b"ab", b"c"])


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
    def test_writes_the_version_after_the_length(self) -> None:
        # Length, version (7), offset of the field and the field (42)
        cases: list[tuple[AddressableSpace, bool, list[int]]] = [
            (SIZEOF_8, True, [4, 7, 3, 42]),
            (SIZEOF_16, True, [7, 0, 7, 0, 6, 0, 42]),
            (SIZEOF_32, True, [13, 0, 0, 0, 7, 0, 0, 0, 12, 0, 0, 0, 42]),
            (SIZEOF_32, False, [7, 0, 0, 0, 8, 0, 0, 0, 42]),
        ]

        for size, sized, table in cases:
            with self.subTest(size=size, sized=sized):
                layout = TableLayout(sized=sized, versioned=True, offset_size=size)
                builder = TableBuilder(1, layout, 7)
                builder.set_uint8(42)

                buffer = ByteBuffer(bytes(builder.bytes()), layout)

                self.assertEqual(list(buffer.bytes()), table)
                self.assertEqual(buffer.get_version(), 7)
                self.assertEqual(buffer.get_uint8(buffer.root_offset(0)), 42)

    def test_rejects_a_version_that_its_slot_cannot_hold(self) -> None:
        cases: list[tuple[AddressableSpace, int]] = [
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


# A field of `length` elements, and the bytes that each element adds for an
# offset size
LENGTH_FIELDS: list[tuple[str, Callable[[int], int], Callable[[int], Field]]] = [
    ("a byte array", lambda _: 1, lambda n: ("uint8_array", bytes(n))),
    (
        "a typed array",
        lambda _: 8,
        lambda n: ("float64_array", array("d", [0] * n)),
    ),
    ("a string", lambda _: 1, lambda n: ("string", "a" * n)),
    ("a vector's count", lambda size: size, lambda n: ("vector", [b""] * n)),
    ("a vector item", lambda _: 1, lambda n: ("vector", [bytes(n)])),
]


class TestTableLength(unittest.TestCase):
    def test_fits_the_longest_field_that_the_offsets_address(self) -> None:
        for size, max_length in MAX_LENGTHS:
            for sized in [False, True]:
                for name, unit, field in LENGTH_FIELDS:
                    with self.subTest(field=name, size=size, sized=sized):
                        layout = TableLayout(sized=sized, offset_size=size)

                        empty = TableBuilder(1, layout)
                        set_field(empty, field(0))
                        room = max_length - len(empty.bytes())
                        length = room // unit(size)

                        longest = TableBuilder(1, layout)
                        set_field(longest, field(length))
                        buffer = longest.finalize()

                        self.assertLessEqual(len(buffer.bytes()), max_length)
                        self.assertGreater(len(buffer.bytes()), max_length - unit(size))
                        self.assertEqual(
                            read_root_field(buffer, field(0)[0], 0),
                            expected(field(length)),
                        )

                        # The setter raises and leaves the builder as it was
                        too_long = TableBuilder(1, layout)
                        with self.assertRaisesRegex(ValueError, "too long"):
                            set_field(too_long, field(length + 1))

                        set_field(too_long, field(0))
                        self.assertEqual(too_long.bytes(), empty.bytes())

    def test_rejects_the_field_that_makes_the_table_too_long(self) -> None:
        for size, max_length in MAX_LENGTHS:
            with self.subTest(size=size):
                layout = TableLayout(offset_size=size)

                # Two offsets, the array's length prefix and bytes, and one byte
                fits = TableBuilder(2, layout)
                fits.set_uint8_array(bytes(max_length - 3 * size - 1))
                fits.set_uint8(7)

                buffer = fits.finalize()
                self.assertEqual(len(buffer.bytes()), max_length)
                self.assertEqual(buffer.get_uint8(buffer.root_offset(1)), 7)

                too_long = TableBuilder(2, layout)
                too_long.set_uint8_array(bytes(max_length - 3 * size))
                with self.assertRaisesRegex(ValueError, "too long"):
                    too_long.set_uint8(7)

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
    @given(lists(fields, max_size=6), lists(fields, max_size=6))
    def test_writes_the_same_bytes_as_nesting_its_bytes_at_its_alignment(
        self, parent_entries: list[Field], nested_entries: list[Field]
    ) -> None:
        with_builder = builder_of(parent_entries, 1)
        with_builder.set_table(builder_of(nested_entries))

        with_bytes = builder_of(parent_entries, 1)
        with_bytes.set_table(
            builder_of(nested_entries).bytes(),
            cast(Any, alignment_of(nested_entries)),
        )

        self.assertEqual(with_builder.bytes(), with_bytes.bytes())

    @given(
        lists(fields, max_size=6), lists(fields, max_size=6), lists(fields, max_size=6)
    )
    def test_reads_back_tables_nested_two_deep(
        self,
        outer_entries: list[Field],
        middle_entries: list[Field],
        inner_entries: list[Field],
    ) -> None:
        middle = builder_of(middle_entries, 1)
        middle.set_table(builder_of(inner_entries))

        layout = TableLayout(sized=True)
        outer = builder_of(outer_entries, 1, layout=layout)
        outer.set_table(middle)

        buffer = ByteBuffer(outer.bytes(), layout)
        middle_position = buffer.root_offset(len(outer_entries))
        inner_position = buffer.field_offset(middle_position, len(middle_entries))

        for index, entry in enumerate(inner_entries):
            position = buffer.field_offset(inner_position, index)
            self.assertEqual(read_field(buffer, entry[0], position), expected(entry))

    @given(lists(lists(fields, max_size=6), max_size=5))
    def test_writes_a_vector_of_builders_as_a_vector_of_their_bytes(
        self, items: list[list[Field]]
    ) -> None:
        with_builders = TableBuilder(1)
        with_builders.set_vector([builder_of(item) for item in items])

        mixed = TableBuilder(1)
        mixed.set_vector(
            [
                builder_of(item) if i % 2 == 0 else builder_of(item).bytes()
                for i, item in enumerate(items)
            ]
        )

        with_bytes = TableBuilder(1)
        with_bytes.set_vector([builder_of(item).bytes() for item in items])

        self.assertEqual(with_builders.bytes(), with_bytes.bytes())
        self.assertEqual(mixed.bytes(), with_bytes.bytes())

    def test_pads_a_builder_without_arrays_by_nothing(self) -> None:
        nested = TableBuilder(1)
        nested.set_float64(1.5)

        parent = TableBuilder(2)
        parent.set_uint8(1)
        parent.set_table(nested)

        self.assertEqual(parent.finalize().root_offset(1), 2 * SIZEOF_32 + 1)

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
        self.assertEqual(read_root_field(buffer, "vector", 1), [nested.bytes()])

    def test_rejects_a_builder_that_makes_its_parent_too_long(self) -> None:
        nested = TableBuilder(1)
        nested.set_uint8_array(bytes(2**8 - 1 - 8))

        parent = TableBuilder(1, TableLayout(offset_size=SIZEOF_8))
        with self.assertRaisesRegex(ValueError, "too long"):
            parent.set_table(nested)
        with self.assertRaisesRegex(ValueError, "too long"):
            parent.set_vector([nested])

        parent.set_vector([])
        self.assertEqual(list(parent.bytes()), [1, 0])

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
        self.assertEqual(read_root_field(second.finalize(), "vector", 1), [own, own])

    def test_copies_a_builder_of_another_offset_size_in_a_vector(self) -> None:
        cases: list[tuple[AddressableSpace, TableLayout[Any]]] = [
            (SIZEOF_8, TableLayout.DEFAULT),
            (SIZEOF_32, TableLayout(offset_size=SIZEOF_16)),
        ]

        for offset_size, parent_layout in cases:
            with self.subTest(offset_size=offset_size):
                layout = TableLayout(offset_size=offset_size)
                nested = TableBuilder(2, layout)
                nested.set_string("another width")
                nested.set_uint16_array([1, 2])

                parent = TableBuilder(1, cast(Any, parent_layout))
                parent.set_vector([nested])

                buffer = ByteBuffer(parent.bytes(), parent_layout)
                [item] = buffer.get_vector(buffer.root_offset(0))
                self.assertEqual(item, nested.bytes())

                read = ByteBuffer(item, layout)
                self.assertEqual(read.get_string(read.root_offset(0)), "another width")

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
