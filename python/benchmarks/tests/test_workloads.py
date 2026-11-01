"""Checks that the generator makes the TypeScript comparison's values, which
these were printed from, so that the languages' benchmarks time the same work"""

import struct
import unittest

from tafla_benchmarks.workloads import (
    LIST_ITEMS,
    DocumentInput,
    ItemInput,
    document_input,
    list_input,
)

_FLOAT32 = struct.Struct("<f")
_UINT32 = struct.Struct("<I")


def float32_bits(value: float) -> int:
    return _UINT32.unpack(_FLOAT32.pack(value))[0]


def hash_columns(document: DocumentInput) -> int:
    """Hashes the float columns' bits, row by row, as the TypeScript script
    that printed these did with 32-bit integers"""
    digest = 0
    for a, b in zip(document.a, document.b, strict=True):
        digest = (digest * 31 + float32_bits(a)) & 0xFFFF_FFFF
        digest = (digest * 31 + float32_bits(b)) & 0xFFFF_FFFF

    return digest


def item_bits(item: ItemInput) -> tuple[int, int, int]:
    return (
        float32_bits(item.value),
        float32_bits(item.first),
        float32_bits(item.second),
    )


class TestWorkloads(unittest.TestCase):
    def test_generates_the_typescript_document_of_100_rows(self) -> None:
        document = document_input(100)

        self.assertEqual(document.keys[0], 14_727_226)
        self.assertEqual(document.keys[99], 10_335_322)
        self.assertEqual(sum(document.keys), 825_833_448)
        self.assertEqual(float32_bits(document.a[0]), 0xC2219B29)
        self.assertEqual(float32_bits(document.b[99]), 0xC163647E)
        self.assertEqual(hash_columns(document), 1_493_961_445)

    def test_generates_the_typescript_document_of_100000_rows(self) -> None:
        document = document_input(100_000)

        self.assertEqual(document.keys[0], 13_320_945)
        self.assertEqual(document.keys[99_999], 11_621_954)
        self.assertEqual(sum(document.keys), 839_050_642_924)
        self.assertEqual(float32_bits(document.a[0]), 0xC1D4D7F6)
        self.assertEqual(float32_bits(document.b[99_999]), 0x3E8C91F5)
        self.assertEqual(hash_columns(document), 1_138_446_770)

    def test_generates_the_typescript_vector_of_nested_tables(self) -> None:
        items = list_input(LIST_ITEMS).items

        self.assertEqual(len(items), 100)
        self.assertEqual(item_bits(items[0]), (0x41D2ACB6, 0xBFCEDB15, 0x41E32E23))
        self.assertEqual(item_bits(items[99]), (0x4193D915, 0x3F979E05, 0x418F26E1))


if __name__ == "__main__":
    unittest.main()
