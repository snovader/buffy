import ctypes
import unittest
from array import array
from collections.abc import Buffer, Iterable
from typing import Any

from hypothesis import given
from hypothesis.strategies import floats, integers, lists, one_of

from buffy_core import ByteBuffer, TableBuilder
from buffy_core.elements import as_elements
from buffy_core.types import TypeCode
from tests.fields import TYPE_CODES

FLOAT_KINDS = ["float32_array", "float64_array"]
TYPE_KINDS = {type_code: kind for kind, type_code in TYPE_CODES.items()}
INT_KINDS = [kind for kind in TYPE_CODES if kind not in FLOAT_KINDS]

small_ints = lists(integers(0, 127), max_size=16)
numbers = lists(one_of(integers(-(2**31), 2**31), floats(allow_nan=False)), max_size=16)


def inputs(values: list[int]) -> list[tuple[str, Any]]:
    """The kinds of input that ``values`` can arrive as, holding the same
    integers, which fit in every element type"""
    return [
        ("list", values),
        ("tuple", tuple(values)),
        ("generator", (value for value in values)),
        ("bytes", bytes(values)),
        ("bytearray", bytearray(values)),
        ("array of int32", array("i", values)),
        ("memoryview of int16", memoryview(array("h", values))),
        ("ctypes array of int16", (ctypes.c_int16 * len(values))(*values)),
    ]


def round_trip(kind: str, values: Buffer | Iterable[Any]) -> list[Any]:
    builder = TableBuilder(1)
    getattr(builder, f"set_{kind}")(values)

    buffer = builder.finalize()
    return getattr(buffer, f"get_{kind}")(buffer.root_offset(0)).tolist()


class TestConversion(unittest.TestCase):
    # subTest() is left out of the tests with @given, which turns it off

    @given(small_ints)
    def test_converts_every_kind_of_input_like_array(self, values: list[int]) -> None:
        for kind, type_code in TYPE_CODES.items():
            expected = array(type_code, values).tolist()

            for name, input in inputs(values):
                self.assertEqual(round_trip(kind, input), expected, f"{kind}, {name}")

    @given(numbers)
    def test_converts_floats_like_array(self, values: list[float]) -> None:
        for kind in FLOAT_KINDS:
            expected = array(TYPE_CODES[kind], values).tolist()

            for input in [values, array("d", values)]:
                self.assertEqual(round_trip(kind, input), expected, kind)

    def test_rejects_floats_for_integer_elements_like_array(self) -> None:
        for kind in INT_KINDS:
            with self.subTest(kind=kind), self.assertRaises(TypeError):
                getattr(TableBuilder(1), f"set_{kind}")(array("d", [1.0]))

    def test_rejects_a_view_in_a_format_that_memoryview_cant_read(self) -> None:
        view = memoryview((ctypes.c_int16 * 2)(1, 2))

        with self.assertRaisesRegex(TypeError, "<h"):
            TableBuilder(1).set_int32_array(view)

    def test_converts_bytes_element_by_element(self) -> None:
        # array("f", b"...") would take the bytes as raw float32s instead
        self.assertEqual(round_trip("float32_array", b"\x00\x00\x80?"), [0, 0, 128, 63])

    def test_rejects_a_value_out_of_range_from_the_setter(self) -> None:
        for kind in INT_KINDS:
            with self.subTest(kind=kind), self.assertRaises(OverflowError):
                getattr(TableBuilder(1), f"set_{kind}")([2**64])

    def test_rejects_a_value_of_another_type_from_the_setter(self) -> None:
        for kind in INT_KINDS:
            with self.subTest(kind=kind), self.assertRaises(TypeError):
                getattr(TableBuilder(1), f"set_{kind}")([1.5])

        for kind in FLOAT_KINDS:
            with self.subTest(kind=kind), self.assertRaises(TypeError):
                getattr(TableBuilder(1), f"set_{kind}")(["1.5"])

    def test_copies_buffers_of_the_element_type_as_they_are(self) -> None:
        for type_code in TYPE_CODES.values():
            values = array(type_code, [1, 2])

            with self.subTest(type_code=type_code):
                self.assertIs(as_elements(type_code, values), values)

                view = memoryview(values)
                self.assertIs(as_elements(type_code, view), view)

    def test_copies_buffers_of_other_formats_of_the_same_kind_and_size(self) -> None:
        # numpy's int64 and uint64 arrays have the formats of a C long, which
        # has 8 bytes on most 64-bit platforms, and ctypes arrays have formats
        # with a byte order
        views: list[tuple[TypeCode, memoryview]] = [
            ("f", memoryview((ctypes.c_float * 2)(1, 2))),
            ("q", memoryview((ctypes.c_int64 * 2)(1, 2))),
            ("I", memoryview((ctypes.c_uint32 * 2)(1, 2))),
        ]
        if ctypes.sizeof(ctypes.c_long) == 8:
            longs = memoryview(array("q", [1, 2])).cast("B")
            views += [("q", longs.cast("l")), ("Q", longs.cast("L"))]

        for type_code, view in views:
            with self.subTest(type_code=type_code, format=view.format):
                self.assertIs(as_elements(type_code, view), view)
                self.assertEqual(round_trip(TYPE_KINDS[type_code], view), [1, 2])

    def test_converts_buffers_of_another_size(self) -> None:
        values = (ctypes.c_int16 * 2)(1, 2)
        elements = as_elements("i", values)

        self.assertIsNot(elements, values)
        self.assertEqual(memoryview(elements).tolist(), [1, 2])

    @given(lists(floats(allow_nan=False), max_size=16), integers(1, 7))
    def test_writes_into_bytes_unaligned_for_the_element_type(
        self, values: list[float], byte_offset: int
    ) -> None:
        aligned = bytearray(4 + len(values) * 8)
        ByteBuffer(aligned).set_float64_array(values)

        backing = bytearray(byte_offset + len(aligned))
        ByteBuffer(memoryview(backing)[byte_offset:]).set_float64_array(values)

        self.assertEqual(backing[byte_offset:], aligned)


if __name__ == "__main__":
    unittest.main()
