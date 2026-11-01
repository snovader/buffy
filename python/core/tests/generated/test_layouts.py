"""The code generated from ``schemas/layouts.buffy`` writes and reads tables of
8-, 16- and 32-bit offsets, each with and without the table's length and a
version in its header. Each property holds for every message."""

import unittest
from functools import partial

from hypothesis import given, note
from hypothesis.strategies import DataObject, binary, data as data_objects

from buffy_core import DecodeError
from tests.generated import SETTINGS
from tests.generated.byte_strategies import (
    Overwrite,
    Slice,
    Truncation,
    corruptions,
    slices,
)
from tests.generated.expectations import (
    assert_same_table,
    assert_values_or_decode_error,
)
from tests.generated.layouts_tables import MESSAGES, Fields, fields_of_every_message
from tests.tables import build_table

_VERSIONED = [layout for layout in MESSAGES if layout.version is not None]


class TestRoundTrip(unittest.TestCase):
    @SETTINGS
    @given(fields_of_every_message, slices)
    def test_reads_back_the_fields_of_every_layout_wherever_its_bytes_lie(
        self, fields: Fields, slice: Slice
    ) -> None:
        for layout in MESSAGES:
            data = slice.of(layout.message.serialize(**fields))

            read = layout.read(data)
            assert_same_table(
                self,
                layout.describe(read),
                layout.describe(fields),
                layout.message.__name__,
            )


class TestTablesBuiltByHand(unittest.TestCase):
    @SETTINGS
    @given(fields_of_every_message)
    def test_writes_the_bytes_that_table_builder_writes_in_every_layout(
        self, fields: Fields
    ) -> None:
        for layout in MESSAGES:
            by_hand = bytes(build_table(layout.describe(fields)).bytes())

            serialized = bytes(layout.message.serialize(**fields))
            self.assertEqual(serialized, by_hand, layout.message.__name__)


class TestLimits(unittest.TestCase):
    def test_writes_the_longest_table_that_8_and_16_bit_offsets_address_and_rejects_longer(
        self,
    ) -> None:
        values = [1, 0xFFFF]

        for layout in MESSAGES:
            if layout.offset_size() == 4:
                continue

            with self.subTest(layout.message.__name__):
                # A table of this message ends with its values, an array of
                # uint16 aligned to 2 from the table's start, so its length is
                # even: the longest is one short of the largest length, which
                # is odd, and two more bytes of name make it one too long
                longest = layout.max_value() - 1
                shortest = len(layout.message.serialize(id=1, name="", values=values))
                name = "a" * (longest - shortest)

                data = layout.message.serialize(id=1, name=name, values=values)
                self.assertEqual(len(data), longest)
                self.assertEqual(
                    layout.read(data), {"id": 1, "name": name, "values": values}
                )

                with self.assertRaises(ValueError):
                    layout.message.serialize(id=1, name=f"{name}aa", values=values)


class TestDecodeErrors(unittest.TestCase):
    @SETTINGS
    @given(fields_of_every_message, data_objects())
    def test_rejects_a_versioned_message_of_another_version(
        self, fields: Fields, draw: DataObject
    ) -> None:
        for layout in _VERSIONED:
            name = layout.message.__name__
            version = draw.draw(layout.other_versions(), label=f"{name} version")
            table = build_table(layout.describe(fields, version)).bytes()

            with self.assertRaises(DecodeError, msg=name):
                layout.message(table)

    @SETTINGS
    @given(fields_of_every_message, corruptions)
    def test_reads_a_corrupted_message_of_every_layout_as_values_or_raises_decode_error(
        self, fields: Fields, corruption: Truncation | Overwrite
    ) -> None:
        for layout in MESSAGES:
            note(f"Reading {layout.message.__name__}")
            data = corruption.apply(bytes(layout.message.serialize(**fields)))

            assert_values_or_decode_error(partial(layout.read, data))

    @SETTINGS
    @given(binary(max_size=64))
    def test_reads_arbitrary_bytes_as_a_message_of_every_layout_or_raises_decode_error(
        self, data: bytes
    ) -> None:
        for layout in MESSAGES:
            note(f"Reading {layout.message.__name__}")

            assert_values_or_decode_error(partial(layout.read, data))


if __name__ == "__main__":
    unittest.main()
