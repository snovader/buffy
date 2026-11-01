import sys
import unittest
from array import array
from unittest.mock import patch

from hypothesis import given
from hypothesis.strategies import integers

from buffy_core import SIZEOF_8, SIZEOF_32, ByteBuffer, TableBuilder, TableLayout
from tests.descriptions import ARRAY_FIELDS
from tests.tables import DEFAULT_LAYOUT, Description, build_table


class TestByteBuffer(unittest.TestCase):
    @unittest.skipUnless(sys.byteorder == "little", "Big-endian hosts read copies")
    @given(ARRAY_FIELDS, integers(1, 7))
    def test_reads_arrays_at_any_alignment_as_views_of_the_bytes(
        self, field: Description, misalignment: int
    ) -> None:
        table = {"layout": DEFAULT_LAYOUT, "fields": [field]}
        backing = bytearray(misalignment) + build_table(table).bytes()
        buffer = ByteBuffer(memoryview(backing)[misalignment:])

        element = field["type"].removesuffix("[]")
        values = getattr(buffer, f"get_{element}_array")(buffer.root_offset(0))

        self.assertIs(values.obj, backing)

    def test_swaps_each_elements_bytes_on_big_endian_hosts(self) -> None:
        values = array("i", [1, -2])

        # Take the big-endian branches on any host. The host's own order is
        # little-endian, so each element's bytes come out reversed.
        with patch("buffy_core.elements.IS_LITTLE_ENDIAN", False):
            builder = TableBuilder(1)
            builder.set_int32_array(values)
            serialized = bytes(builder.bytes())

            read = ByteBuffer(serialized).get_int32_array(SIZEOF_32)

        self.assertEqual(values.tolist(), [1, -2])
        self.assertEqual(serialized[2 * SIZEOF_32 :], b"\0\0\0\x01\xff\xff\xff\xfe")
        self.assertEqual(read.tolist(), [1, -2])
        self.assertIsNot(read.obj, serialized)

    def test_writes_nothing_past_the_supplied_bytes(self) -> None:
        backing = bytearray(b"\xaa" * 32)
        buffer = ByteBuffer(memoryview(backing)[:12])

        writes = [
            lambda: buffer.set_float32_array(array("f", [1, 2, 3])),
            lambda: buffer.set_float32_array([1, 2, 3]),
            lambda: buffer.set_string(bytes(9)),
            lambda: buffer.set_vector([bytes(4), bytes(1)]),
            lambda: buffer.set_bytes(bytes(13)),
            lambda: buffer.set_padding(13),
        ]

        for write in writes:
            with self.subTest(), self.assertRaises(ValueError):
                write()

        buffer.set_bytes(bytes(8))
        with self.assertRaises(ValueError):
            buffer.set_float64(1.5)

        self.assertEqual(backing, bytes(8) + b"\xaa" * 24)

    def test_rejects_a_value_that_doesnt_fit_its_slot_and_keeps_its_position(
        self,
    ) -> None:
        # CPython's struct zeroes a slot before it checks the value, so only
        # the bytes after the slot are left as they were
        for write, value, size in [
            ("set_uint8", 256, 1),
            ("set_int16", -(2**15) - 1, 2),
            ("set_uint32", 2**32, 4),
            ("set_uint64", -1, 8),
            ("set_float32", 1e300, 4),
            ("set_float32", -1e300, 4),
            ("set_offset", 2**32, 4),
        ]:
            with self.subTest(write=write, value=value):
                backing = bytearray(b"\xaa" * 12)
                buffer = ByteBuffer(backing)

                with self.assertRaises(ValueError):
                    getattr(buffer, write)(value)
                buffer.set_uint8(1)

                self.assertEqual(backing[0], 1)
                self.assertEqual(backing[size:], b"\xaa" * (12 - size))

    def test_rejects_a_count_or_length_too_large_for_its_slot_and_writes_nothing(
        self,
    ) -> None:
        backing = bytearray(b"\xaa" * 600)
        buffer = ByteBuffer(backing, TableLayout(offset_size=SIZEOF_8))

        writes = [
            lambda: buffer.set_uint8_array(bytes(256)),
            lambda: buffer.set_int16_array(array("h", range(256))),
            lambda: buffer.set_string(bytes(256)),
            lambda: buffer.set_vector([bytes(1), bytes(256)]),
            lambda: buffer.set_vector([b""] * 256),
        ]

        for write in writes:
            with self.subTest():
                with self.assertRaises(ValueError):
                    write()
                self.assertEqual(backing, b"\xaa" * 600)

        buffer.set_vector([bytes(1), bytes(255)])
        self.assertEqual(backing[:4], b"\x02\x01\x00\xff")

    def test_writes_padding_as_zeros(self) -> None:
        backing = bytearray(b"\xff" * 4)
        buffer = ByteBuffer(backing)
        buffer.set_padding(3)
        buffer.set_uint8(1)

        self.assertEqual(backing, b"\0\0\0\x01")


if __name__ == "__main__":
    unittest.main()
