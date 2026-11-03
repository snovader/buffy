from typing import Self


class DecodeError(ValueError):
    """Raised when bytes can't be read as the table, array, string or value
    that a read expects. Every read checks its range against the supplied bytes
    before it returns, so a truncated or corrupted buffer raises this rather
    than returning values from outside them. Strings must also be valid UTF-8.

    Checks can't tell a value that was corrupted within range from a real one,
    such as an offset that points at another field. Formats that need that
    guarantee can add a checksum.

    It's a ``ValueError``, so existing handlers of that keep catching it.
    """

    @classmethod
    def out_of_range(cls, position: int, byte_length: int, available: int) -> Self:
        """Returns the error of a read of ``byte_length`` bytes at
        ``position`` that leaves the ``available`` bytes, as every read of
        tafla and of the code that tafla-gen generates raises it. Building it
        doesn't fail for any position or length, however large.
        """
        return cls(
            "Read exceeds the supplied buffer: "
            f"{describe_int(byte_length)} bytes at {describe_int(position)} "
            f"of {available}"
        )


# Integers of up to 64 bits have at most 20 digits, which the least limit of
# sys.set_int_max_str_digits() still converts
_MAX_DECIMAL_BITS = 64


def describe_int(value: int) -> str:
    """Returns ``value`` in decimal for an error's message, or its sign and
    bit length if it's longer than 64 bits. Converting an integer of more
    digits than ``sys.get_int_max_str_digits()``, which callers can set,
    raises ``ValueError``, so an error built from a caller's position, length
    or index would fail to build."""
    bits = value.bit_length()
    if bits <= _MAX_DECIMAL_BITS:
        return str(value)

    sign = "negative " if value < 0 else ""
    return f"<{sign}{bits}-bit integer>"
