import sys
import unittest
from array import array
from collections.abc import Callable
from struct import calcsize, iter_unpack, pack, unpack
from unittest.mock import patch

from hypothesis import given
from hypothesis.strategies import DrawFn, binary, composite, integers, sampled_from

from tafla import SIZEOF_8, SIZEOF_32, ByteBuffer, TableBuilder, TableLayout
from tafla.types import IntTypeCode
from tests.descriptions import ARRAY_FIELDS
from tests.tables import DEFAULT_LAYOUT, Description, build_table


@composite
def strided_views(draw: DrawFn) -> memoryview:
    """Views with a step of bytes, or of wider elements, which are C-contiguous
    only when they hold at most one element"""
    codes: list[IntTypeCode] = ["B", "h", "i", "q"]
    type_code = draw(sampled_from(codes))
    count = draw(integers(0, 8))
    size = count * calcsize(type_code)
    data = draw(binary(min_size=size, max_size=size))
    step = draw(sampled_from([-3, -2, -1, 2, 3]))

    return memoryview(data).cast(type_code)[::step]


class TestByteBuffer(unittest.TestCase):
    @given(strided_views())
    def test_takes_bytes_only_when_theyre_c_contiguous(self, view: memoryview) -> None:
        """Reads return views of the bytes, which a copy of them wouldn't be"""
        if view.c_contiguous:
            self.assertEqual(bytes(ByteBuffer(view).bytes()), view.tobytes())
        else:
            with self.assertRaisesRegex(TypeError, "isn't C-contiguous"):
                ByteBuffer(view)

    def test_rejects_bytes_to_write_that_arent_c_contiguous_and_writes_nothing(
        self,
    ) -> None:
        backing = bytearray(b"\xaa" * 32)
        buffer = ByteBuffer(backing)
        strided = memoryview(bytes(range(8)))[::2]

        writes = [
            lambda: buffer.set_bytes(strided),
            lambda: buffer.set_string(strided),
            lambda: buffer.set_struct_array(strided, 2),
            lambda: buffer.set_vector([b"\x01", strided]),
        ]

        for write in writes:
            with (
                self.subTest(),
                self.assertRaisesRegex(TypeError, "isn't C-contiguous"),
            ):
                write()

        self.assertEqual(backing, b"\xaa" * 32)

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
        with patch("tafla.elements.IS_LITTLE_ENDIAN", False):
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
            lambda: buffer.set_struct_array(bytes(9), 3),
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

    def test_rejects_a_count_length_or_item_end_too_large_for_its_slot_and_writes_nothing(
        self,
    ) -> None:
        backing = bytearray(b"\xaa" * 600)
        buffer = ByteBuffer(backing, TableLayout(offset_size=SIZEOF_8))

        writes = [
            lambda: buffer.set_uint8_array(bytes(256)),
            lambda: buffer.set_int16_array(array("h", range(256))),
            lambda: buffer.set_string(bytes(256)),
            lambda: buffer.set_struct_array(bytes(256), 1),
            # The last item ends 256 bytes from the vector's count
            lambda: buffer.set_vector([bytes(1), bytes(252)]),
            lambda: buffer.set_vector([b""] * 256),
        ]

        for write in writes:
            with self.subTest():
                with self.assertRaises(ValueError):
                    write()
                self.assertEqual(backing, b"\xaa" * 600)

        # The count, then the ends of the items, which start at 3
        buffer.set_vector([bytes(1), bytes(251)])
        self.assertEqual(backing[:3], b"\x02\x04\xff")

    def test_rejects_struct_sizes_less_than_1_and_bytes_that_arent_whole_structs(
        self,
    ) -> None:
        backing = bytearray(b"\xaa" * 16)
        buffer = ByteBuffer(backing)

        cases: list[tuple[str, Callable[[], object]]] = [
            (
                "5 bytes aren't a whole number of 2-byte structs",
                lambda: buffer.set_struct_array(bytes(5), 2),
            ),
            (
                "must be a positive integer, not 0",
                lambda: buffer.set_struct_array(b"", 0),
            ),
            (
                "must be a positive integer, not -1",
                lambda: buffer.get_struct_array(0, -1),
            ),
            (
                "must be a positive integer, not 0",
                lambda: buffer.get_struct_array(0, 0),
            ),
        ]

        for message, call in cases:
            with self.subTest(message), self.assertRaisesRegex(ValueError, message):
                call()

        self.assertEqual(backing, b"\xaa" * 16)

    @unittest.skipUnless(sys.byteorder == "little", "Big-endian hosts read copies")
    def test_reads_structs_as_views_of_the_bytes_at_any_alignment(self) -> None:
        builder = TableBuilder(2)
        builder.set_struct(pack("<hf", -2, 1.5))
        builder.set_struct_array(pack("<hfhf", 3, 0.5, 4, -0.5), 6, 4)

        backing = bytearray(1) + builder.bytes()
        buffer = ByteBuffer(memoryview(backing)[1:])
        struct_bytes = buffer.get_struct(buffer.root_offset(0), 6)
        array_bytes = buffer.get_struct_array(buffer.root_offset(1), 6)

        self.assertEqual(unpack("<hf", struct_bytes), (-2, 1.5))
        self.assertEqual(list(iter_unpack("<hf", array_bytes)), [(3, 0.5), (4, -0.5)])
        self.assertIs(struct_bytes.obj, backing)
        self.assertIs(array_bytes.obj, backing)

    def test_finds_each_vector_item_from_its_ends_as_reading_every_item_finds_it(
        self,
    ) -> None:
        builder = TableBuilder(1)
        builder.set_vector([b"\x01", b"", b"\x02\x03"])
        buffer = builder.finalize()
        vector = buffer.root_offset(0)

        # The count and the three ends, at 4, then the items from 20
        self.assertEqual(buffer.get_vector_offsets(vector).tolist(), [20, 21, 21])
        self.assertEqual(
            [buffer.get_vector_offset(vector, index) for index in range(3)],
            [20, 21, 21],
        )
        self.assertEqual(buffer.get_vector_length(vector), 3)
        self.assertEqual(
            [bytes(item) for item in buffer.get_vector(vector)],
            [b"\x01", b"", b"\x02\x03"],
        )

    def test_rejects_an_index_outside_a_vector_with_index_error(self) -> None:
        builder = TableBuilder(1)
        builder.set_vector([b"\x01", b"\x02\x03"])
        buffer = builder.finalize()

        for index in [-1, 2]:
            with (
                self.subTest(index=index),
                self.assertRaisesRegex(
                    IndexError, f"Item {index} is outside the vector of 2 at 4"
                ),
            ):
                buffer.get_vector_offset(buffer.root_offset(0), index)

    def test_writes_padding_as_zeros(self) -> None:
        backing = bytearray(b"\xff" * 4)
        buffer = ByteBuffer(backing)
        buffer.set_padding(3)
        buffer.set_uint8(1)

        self.assertEqual(backing, b"\0\0\0\x01")


if __name__ == "__main__":
    unittest.main()
