import sys
import unittest
from array import array
from unittest.mock import patch

from hypothesis import given
from hypothesis.strategies import integers

from buffy_core import SIZEOF_32, ByteBuffer, TableBuilder
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
        backing = bytearray(32)
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

        self.assertEqual(backing, bytearray(32))

    def test_rejects_a_value_that_doesnt_fit_its_type_and_writes_nothing(self) -> None:
        backing = bytearray(8)
        buffer = ByteBuffer(backing)

        for write, value in [
            (buffer.set_uint8, 256),
            (buffer.set_int16, -(2**15) - 1),
            (buffer.set_uint64, -1),
        ]:
            with self.subTest(write=write.__name__, value=value):
                with self.assertRaises(ValueError):
                    write(value)
                self.assertEqual(backing, bytearray(8))

        for value in [1e300, -1e300]:
            with self.subTest(write="set_float32", value=value):
                with self.assertRaises(ValueError):
                    buffer.set_float32(value)
                self.assertEqual(backing, bytearray(8))

    def test_writes_padding_as_zeros(self) -> None:
        backing = bytearray(b"\xff" * 4)
        buffer = ByteBuffer(backing)
        buffer.set_padding(3)
        buffer.set_uint8(1)

        self.assertEqual(backing, b"\0\0\0\x01")


if __name__ == "__main__":
    unittest.main()
