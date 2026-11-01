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


def out_of_range(position: int, byte_length: int, available: int) -> DecodeError:
    """Builds the error for a read outside the supplied bytes"""
    return DecodeError(
        "Read exceeds the supplied buffer: "
        f"{byte_length} bytes at {position} of {available}"
    )
