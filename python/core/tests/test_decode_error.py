import unittest
from collections.abc import Callable

from tafla import ByteBuffer, DecodeError


class TestDecodeError(unittest.TestCase):
    def test_is_a_value_error(self) -> None:
        self.assertIsInstance(DecodeError("message"), ValueError)

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

    def test_rejects_a_vector_count_before_allocating_for_it(self) -> None:
        buffer = ByteBuffer((2**32 - 1).to_bytes(4, "little") + bytes(4))

        with self.assertRaises(DecodeError):
            buffer.get_vector_offsets(0)


if __name__ == "__main__":
    unittest.main()
