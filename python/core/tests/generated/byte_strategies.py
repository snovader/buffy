"""Strategies of where a reader finds a table's bytes in memory, and of how
the bytes are corrupted."""

from collections.abc import Buffer
from dataclasses import dataclass

from hypothesis.strategies import SearchStrategy, binary, builds, integers, one_of


@dataclass(frozen=True)
class Slice:
    """Where a reader finds a table's bytes: at ``offset`` in a larger buffer,
    which may leave them unaligned, with ``fill`` in the bytes around them"""

    offset: int
    fill: int

    def of(self, data: Buffer) -> memoryview:
        """Copies ``data`` into a slice of a larger buffer"""
        source = memoryview(data)
        end = self.offset + source.nbytes

        buffer = bytearray([self.fill]) * (end + 16)
        buffer[self.offset : end] = source

        return memoryview(buffer)[self.offset : end]


# Offsets that align a slice to each element size or none
slices: SearchStrategy[Slice] = builds(Slice, integers(0, 15), integers(0, 0xFF))


def _wrap(value: int, length: int) -> int:
    """``value`` within [0, length), or 0 if ``length`` is 0"""
    return 0 if length == 0 else value % length


@dataclass(frozen=True)
class Truncation:
    """Cuts 1 + ``cut`` bytes from the end, up to all of them"""

    cut: int

    def apply(self, data: bytes) -> bytes:
        cut = 1 + _wrap(self.cut, len(data))
        return data[: max(len(data) - cut, 0)]


@dataclass(frozen=True)
class Overwrite:
    """Writes ``replacement`` at ``at``, as far as the bytes go"""

    at: int
    replacement: bytes

    def apply(self, data: bytes) -> bytes:
        at = _wrap(self.at, len(data))
        replacement = self.replacement[: len(data) - at]

        corrupted = bytearray(data)
        corrupted[at : at + len(replacement)] = replacement

        return bytes(corrupted)


# Corruptions of bytes of any length, which they fit to it. Hypothesis draws
# small numbers more often, so most truncations cut a few bytes, which leaves
# a field that ends the table short.
corruptions: SearchStrategy[Truncation | Overwrite] = one_of(
    builds(Truncation, integers(min_value=0)),
    builds(Overwrite, integers(min_value=0), binary(min_size=1, max_size=4)),
)
