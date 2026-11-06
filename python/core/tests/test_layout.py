import unittest
from typing import Any, cast

from tafla import OffsetSize, TableLayout


class TestTableLayout(unittest.TestCase):
    def test_defaults_to_no_header_slots_and_uint32_offsets(self) -> None:
        for layout in [TableLayout.DEFAULT, TableLayout()]:
            with self.subTest(layout=layout):
                self.assertFalse(layout.sized())
                self.assertFalse(layout.versioned())
                self.assertEqual(layout.offset_size(), 4)
                self.assertEqual(layout.header_size(), 0)

    def test_has_a_header_slot_for_the_length_and_the_version(self) -> None:
        cases: list[tuple[bool, bool, OffsetSize, int]] = [
            (False, False, 1, 0),
            (True, False, 1, 1),
            (False, True, 2, 2),
            (True, True, 2, 4),
            (True, True, 4, 8),
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


if __name__ == "__main__":
    unittest.main()
