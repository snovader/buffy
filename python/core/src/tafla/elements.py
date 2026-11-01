"""Conversions between the array-likes that callers pass and the little-endian
elements that tables store."""

from array import array
from collections.abc import Buffer, Iterable
from struct import calcsize, error as StructError
from typing import Any, cast

from .constants import IS_LITTLE_ENDIAN
from .types import TypeCode


def _host_formats(type_code: TypeCode) -> frozenset[str]:
    """Returns the buffer formats of elements of the same kind and size as
    ``type_code``'s, in the host's byte order. They include other letters of
    that size, such as ``l`` for ``q`` where a C long has 8 bytes, which is the
    format of numpy's int64 arrays."""
    if type_code in "fd":
        letters = type_code
    elif type_code.islower():
        letters = "bhilqn"
    else:
        letters = "BHILQN"

    byte_order = "<" if IS_LITTLE_ENDIAN else ">"
    formats: set[str] = set()

    for letter in letters:
        for prefix in ("", "@", "=", byte_order):
            try:
                if calcsize(prefix + letter) == calcsize(type_code):
                    formats.add(prefix + letter)
            except StructError:
                pass  # n and N have native sizes only

    return frozenset(formats)


_HOST_FORMATS: dict[str, frozenset[str]] = {
    type_code: _host_formats(type_code) for type_code in "bBhHiIqQfd"
}
_ITEM_SIZES: dict[str, int] = {
    type_code: calcsize(type_code) for type_code in "bBhHiIqQfd"
}


def as_elements(type_code: TypeCode, values: Buffer | Iterable[float]) -> Buffer:
    """Returns ``values`` if they're a one-dimensional, contiguous buffer of
    ``type_code``'s elements in the host's byte order, such as an ``array`` of
    that type code or a numpy array of that type. Anything else is converted as
    ``array(type_code, values)`` converts an iterable, which raises
    ``OverflowError`` for a value out of range and ``TypeError`` for one of
    another type. A ``memoryview`` in a format it can't iterate, such as one
    with a byte order, raises ``TypeError`` too.
    """
    if type(values) is array and values.typecode == type_code:
        return values

    if isinstance(values, Buffer):
        view = memoryview(values)
        if (
            view.ndim == 1
            and view.c_contiguous
            and view.format in _HOST_FORMATS[type_code]
            and view.itemsize == _ITEM_SIZES[type_code]
        ):
            return values

        # An iterator, since array() takes a bytes or bytearray initializer as
        # raw bytes rather than converting its elements. The values rather
        # than the view, since a view iterates native formats only.
        try:
            return array(type_code, iter(cast(Iterable[float], values)))
        except NotImplementedError as error:
            raise TypeError(
                f"Can't convert elements of format {view.format!r} to {type_code!r}"
            ) from error

    return array(type_code, values)


def little_endian_bytes(type_code: TypeCode, elements: Buffer) -> memoryview:
    """Returns the bytes of elements that ``as_elements()`` returned, in
    little-endian order: a view of them on a little-endian host, and a copy
    with each element's bytes swapped on a big-endian host."""
    view = memoryview(elements).cast("B")
    if IS_LITTLE_ENDIAN:
        return view

    swapped = array(type_code)
    swapped.frombytes(view)
    swapped.byteswap()

    return memoryview(swapped).cast("B")


def from_little_endian(type_code: TypeCode, view: memoryview) -> memoryview[Any]:
    """Returns little-endian bytes as elements of ``type_code``: a view of them
    on a little-endian host, at any alignment, and a copy in the host's byte
    order on a big-endian host."""
    if IS_LITTLE_ENDIAN:
        return view.cast(type_code)

    swapped = array(type_code)
    swapped.frombytes(view)
    swapped.byteswap()

    return memoryview(swapped)
