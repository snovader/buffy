import unittest
from collections.abc import Callable
from typing import Any

from hypothesis import given
from hypothesis.strategies import binary, booleans, integers, lists, sampled_from, text

from buffy_core import ByteBuffer, DecodeError, TableBuilder, TableLayout
from tests.fields import (
    KINDS,
    Field,
    builder_of,
    expected,
    fields,
    read_root_field,
)


def read(buffer: ByteBuffer, kind: str, index: int) -> Any:
    """Reads a field, returning the exception it raises instead, which the
    tests check is a DecodeError"""
    try:
        return read_root_field(buffer, kind, index)
    except Exception as error:  # noqa: BLE001
        return error


class TestDecodeError(unittest.TestCase):
    def test_is_a_value_error(self) -> None:
        self.assertIsInstance(DecodeError("message"), ValueError)

    @given(lists(fields, min_size=1, max_size=8), booleans())
    def test_reads_each_field_of_a_truncated_table_or_raises(
        self, entries: list[Field], sized: bool
    ) -> None:
        layout = TableLayout(sized=sized)
        serialized = bytes(builder_of(entries, layout=layout).bytes())

        for length in range(len(serialized)):
            # A copy, so that no read can reach the bytes after the cut
            buffer = ByteBuffer(serialized[:length], layout)

            for index, entry in enumerate(entries):
                result = read(buffer, entry[0], index)

                if isinstance(result, Exception):
                    self.assertIsInstance(result, DecodeError)
                else:
                    self.assertEqual(result, expected(entry))

    @given(binary(max_size=64), sampled_from(KINDS), integers(0, 16), booleans())
    def test_raises_only_decode_error_for_any_bytes(
        self, data: bytes, kind: str, index: int, sized: bool
    ) -> None:
        result = read(ByteBuffer(data, TableLayout(sized=sized)), kind, index)

        if isinstance(result, Exception):
            self.assertIsInstance(result, DecodeError)

    def test_raises_from_scalar_getters_outside_the_bytes(self) -> None:
        backing = bytearray(range(32))
        buffer = ByteBuffer(memoryview(backing)[:8])

        reads: list[tuple[str, Callable[[], object]]] = [
            ("get_float64", lambda: buffer.get_float64(4)),
            ("get_int64", lambda: buffer.get_int64(-1)),
            ("get_uint32", lambda: buffer.get_uint32(5)),
            # A negative position would otherwise count from the end
            ("get_uint32", lambda: buffer.get_uint32(-4)),
            ("get_int8", lambda: buffer.get_int8(8)),
            ("root_offset", lambda: buffer.root_offset(2)),
            ("field_offset", lambda: buffer.field_offset(-8, 0)),
        ]

        for name, get in reads:
            with (
                self.subTest(read=name),
                self.assertRaisesRegex(DecodeError, "Read exceeds the supplied buffer"),
            ):
                get()

    def test_raises_for_a_string_that_isnt_utf_8(self) -> None:
        cases = [
            ("a lone continuation byte", b"\x80"),
            ("a truncated sequence", b"\xe2\x82"),
            ("an overlong encoding", b"\xc0\xaf"),
            ("an encoded surrogate", b"\xed\xa0\x80"),
        ]

        for name, utf8 in cases:
            with self.subTest(case=name):
                buffer = ByteBuffer(len(utf8).to_bytes(4, "little") + utf8)

                with self.assertRaises(DecodeError) as context:
                    buffer.get_string(0)

                self.assertIsInstance(context.exception.__cause__, UnicodeDecodeError)

    @given(text())
    def test_reads_back_any_string_with_a_leading_byte_order_mark(
        self, string: str
    ) -> None:
        for value in [string, f"﻿{string}"]:
            builder = TableBuilder(1)
            builder.set_string(value)

            buffer = builder.finalize()
            self.assertEqual(buffer.get_string(buffer.root_offset(0)), value)

    def test_rejects_a_vector_count_before_allocating_for_it(self) -> None:
        buffer = ByteBuffer((2**32 - 1).to_bytes(4, "little") + bytes(4))

        with self.assertRaises(DecodeError):
            buffer.get_vector_offsets(0)


if __name__ == "__main__":
    unittest.main()
