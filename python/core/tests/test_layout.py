import unittest
from array import array
from typing import Any, cast

from hypothesis import given
from hypothesis.strategies import floats, lists, sampled_from, text

from buffy_core import (
    SIZEOF_8,
    SIZEOF_16,
    SIZEOF_32,
    SIZEOF_64,
    ByteBuffer,
    TableBuilder,
    TableLayout,
)
from buffy_core.types import AddressableSpace
from tests.fields import Field, builder_of, expected, fields, read_field


class TestTableLayout(unittest.TestCase):
    def test_defaults_to_no_header_slots_and_32_bit_offsets(self) -> None:
        for layout in [TableLayout.DEFAULT, TableLayout()]:
            with self.subTest(layout=layout):
                self.assertFalse(layout.sized())
                self.assertFalse(layout.versioned())
                self.assertEqual(layout.offset_size(), SIZEOF_32)
                self.assertEqual(layout.header_size(), 0)

    def test_has_a_header_slot_for_the_length_and_the_version(self) -> None:
        cases: list[tuple[bool, bool, AddressableSpace, int]] = [
            (False, False, SIZEOF_8, 0),
            (True, False, SIZEOF_8, 1),
            (False, True, SIZEOF_16, 2),
            (True, True, SIZEOF_16, 4),
            (True, True, SIZEOF_32, 8),
        ]

        for sized, versioned, offset_size, header_size in cases:
            with self.subTest(sized=sized, versioned=versioned, size=offset_size):
                layout = TableLayout(
                    sized=sized, versioned=versioned, offset_size=offset_size
                )

                self.assertEqual(layout.header_size(), header_size)

    def test_rejects_an_offset_size_other_than_1_2_or_4(self) -> None:
        for offset_size in [0, 3, 8]:
            with self.subTest(offset_size=offset_size), self.assertRaises(ValueError):
                TableLayout(offset_size=cast(Any, offset_size))


class TestFieldOffset(unittest.TestCase):
    @given(lists(fields, max_size=6), lists(fields, max_size=6))
    def test_reads_the_fields_of_a_nested_table(
        self, parent_entries: list[Field], nested_entries: list[Field]
    ) -> None:
        layout = TableLayout(sized=True)
        parent = builder_of(parent_entries, 1, layout=layout)
        parent.set_table(builder_of(nested_entries).bytes())

        buffer = ByteBuffer(parent.bytes(), layout)
        table_position = buffer.root_offset(len(parent_entries))

        for index, entry in enumerate(nested_entries):
            position = buffer.field_offset(table_position, index)
            self.assertEqual(read_field(buffer, entry[0], position), expected(entry))


class TestSetTable(unittest.TestCase):
    @given(text(), sampled_from([SIZEOF_8, SIZEOF_16, SIZEOF_32, SIZEOF_64]))
    def test_starts_the_nested_table_at_a_multiple_of_the_alignment(
        self, string: str, alignment: Any
    ) -> None:
        builder = TableBuilder(2)
        builder.set_string(string)
        builder.set_table(b"\x01\x02\x03", alignment)

        buffer = builder.finalize()
        position = buffer.root_offset(1)

        self.assertEqual(position % alignment, 0)
        self.assertEqual(bytes(buffer.bytes()[position:]), b"\x01\x02\x03")

    @given(text(), lists(floats(allow_nan=False), min_size=1))
    def test_keeps_64_bit_arrays_of_a_table_aligned_to_8(
        self, string: str, values: list[float]
    ) -> None:
        nested = TableBuilder(1)
        nested.set_float64_array(array("d", values))

        builder = TableBuilder(2)
        builder.set_string(string)
        builder.set_table(nested.bytes(), SIZEOF_64)

        buffer = builder.finalize()
        position = buffer.field_offset(buffer.root_offset(1), 0)

        self.assertEqual((position + SIZEOF_32) % SIZEOF_64, 0)
        self.assertEqual(buffer.get_float64_array(position).tolist(), values)


if __name__ == "__main__":
    unittest.main()
