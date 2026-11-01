"""The code generated from ``schemas/all_types.buffy``, of every type of field,
writes and reads tables with buffy_core."""

import unittest

from hypothesis import given
from hypothesis.strategies import binary, integers

from buffy_core import DecodeError
from tests.generated import SETTINGS
from tests.generated.all_types.all_types import (
    Arrays,
    ArraysFields,
    Documented,
    Nested,
    NestedFields,
    Packed,
    Reserved,
    ReservedFields,
    Scalars,
    ScalarsFields,
)
from tests.generated.all_types_tables import (
    arrays_fields,
    describe_arrays,
    describe_nested,
    describe_packed,
    describe_reserved,
    describe_scalars,
    documented_bytes,
    documented_structs,
    nested_fields,
    packed_bytes,
    packed_structs,
    read_arrays,
    read_nested,
    read_reserved,
    read_scalars,
    reserved_fields,
    scalars_fields,
)
from tests.generated.byte_strategies import (
    Overwrite,
    Slice,
    Truncation,
    corruptions,
    slices,
)
from tests.generated.expectations import (
    assert_same_fields,
    assert_same_table,
    assert_values_or_decode_error,
)
from tests.tables import build_table


class TestRoundTrip(unittest.TestCase):
    @SETTINGS
    @given(scalars_fields, slices)
    def test_reads_back_a_scalar_of_every_type_wherever_its_bytes_lie(
        self, fields: ScalarsFields, slice: Slice
    ) -> None:
        data = slice.of(Scalars.serialize(**fields))

        assert_same_table(
            self,
            describe_scalars(read_scalars(Scalars(data))),
            describe_scalars(fields),
        )

    @SETTINGS
    @given(arrays_fields, slices)
    def test_reads_back_an_array_of_every_type_wherever_its_bytes_lie(
        self, fields: ArraysFields, slice: Slice
    ) -> None:
        data = slice.of(Arrays.serialize(**fields))

        assert_same_table(
            self, describe_arrays(read_arrays(Arrays(data))), describe_arrays(fields)
        )

    @SETTINGS
    @given(nested_fields, slices)
    def test_reads_back_tables_nested_three_deep_and_vectors_of_trees(
        self, fields: NestedFields, slice: Slice
    ) -> None:
        data = slice.of(Nested.serialize(**fields))

        assert_same_table(
            self, describe_nested(read_nested(Nested(data))), describe_nested(fields)
        )

    @SETTINGS
    @given(reserved_fields, slices)
    def test_reads_back_fields_named_after_keywords(
        self, fields: ReservedFields, slice: Slice
    ) -> None:
        data = slice.of(Reserved.serialize(**fields))

        assert_same_table(
            self,
            describe_reserved(read_reserved(Reserved(data))),
            describe_reserved(fields),
        )


class TestTablesBuiltByHand(unittest.TestCase):
    @SETTINGS
    @given(scalars_fields)
    def test_writes_the_bytes_of_scalars_that_table_builder_writes(
        self, fields: ScalarsFields
    ) -> None:
        by_hand = bytes(build_table(describe_scalars(fields)).bytes())

        self.assertEqual(bytes(Scalars.serialize(**fields)), by_hand)

    @SETTINGS
    @given(arrays_fields)
    def test_writes_the_bytes_of_aligned_arrays_that_table_builder_writes(
        self, fields: ArraysFields
    ) -> None:
        by_hand = bytes(build_table(describe_arrays(fields)).bytes())

        self.assertEqual(bytes(Arrays.serialize(**fields)), by_hand)

    @SETTINGS
    @given(nested_fields)
    def test_writes_the_bytes_of_nested_tables_and_vectors_that_table_builder_writes(
        self, fields: NestedFields
    ) -> None:
        by_hand = bytes(build_table(describe_nested(fields)).bytes())

        self.assertEqual(bytes(Nested.serialize(**fields)), by_hand)

    @SETTINGS
    @given(reserved_fields)
    def test_writes_the_bytes_of_fields_named_after_keywords_that_table_builder_writes(
        self, fields: ReservedFields
    ) -> None:
        by_hand = bytes(build_table(describe_reserved(fields)).bytes())

        self.assertEqual(bytes(Reserved.serialize(**fields)), by_hand)


class TestDecodeErrors(unittest.TestCase):
    @SETTINGS
    @given(scalars_fields, corruptions)
    def test_reads_corrupted_scalars_as_values_or_raises_decode_error(
        self, fields: ScalarsFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Scalars.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_scalars(Scalars(data)))

    @SETTINGS
    @given(arrays_fields, corruptions)
    def test_reads_corrupted_arrays_as_values_or_raises_decode_error(
        self, fields: ArraysFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Arrays.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_arrays(Arrays(data)))

    @SETTINGS
    @given(nested_fields, corruptions)
    def test_reads_corrupted_nested_tables_and_trees_as_values_or_raises_decode_error(
        self, fields: NestedFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Nested.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_nested(Nested(data)))

    @SETTINGS
    @given(reserved_fields, corruptions)
    def test_reads_corrupted_keywords_as_values_or_raises_decode_error(
        self, fields: ReservedFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Reserved.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_reserved(Reserved(data)))

    @SETTINGS
    @given(binary(max_size=64))
    def test_reads_arbitrary_bytes_as_any_message_or_raises_decode_error(
        self, data: bytes
    ) -> None:
        assert_values_or_decode_error(lambda: read_scalars(Scalars(data)))
        assert_values_or_decode_error(lambda: read_arrays(Arrays(data)))
        assert_values_or_decode_error(lambda: read_nested(Nested(data)))
        assert_values_or_decode_error(lambda: read_reserved(Reserved(data)))


class TestStructs(unittest.TestCase):
    @SETTINGS
    @given(packed_structs)
    def test_writes_a_packed_little_endian_at_fixed_offsets_in_42_bytes(
        self, packed: Packed
    ) -> None:
        self.assertEqual(Packed.SIZE, 42)
        self.assertEqual(packed.serialize(), packed_bytes(packed))

    @SETTINGS
    @given(documented_structs)
    def test_writes_a_documented_little_endian_at_fixed_offsets_in_7_bytes(
        self, documented: Documented
    ) -> None:
        self.assertEqual(Documented.SIZE, 7)
        self.assertEqual(documented.serialize(), documented_bytes(documented))

    @SETTINGS
    @given(packed_structs, binary(max_size=8), binary(max_size=8))
    def test_reads_a_packed_back_at_an_offset_in_longer_data(
        self, packed: Packed, before: bytes, after: bytes
    ) -> None:
        data = before + packed.serialize() + after

        read = Packed.read(data, len(before))
        assert_same_fields(self, describe_packed(read), describe_packed(packed))

    @SETTINGS
    @given(documented_structs, binary(max_size=8), binary(max_size=8))
    def test_reads_a_documented_back_at_an_offset_in_longer_data(
        self, documented: Documented, before: bytes, after: bytes
    ) -> None:
        data = before + documented.serialize() + after

        self.assertEqual(Documented.read(data, len(before)), documented)

    @SETTINGS
    @given(binary(max_size=64), integers(1, Packed.SIZE))
    def test_raises_decode_error_when_the_data_ends_before_a_packed(
        self, data: bytes, missing: int
    ) -> None:
        # Where the struct would end ``missing`` bytes after the data, or at
        # the start of data shorter than a struct
        offset = max(len(data) + missing - Packed.SIZE, 0)

        with self.assertRaises(DecodeError):
            Packed.read(data, offset)

    @SETTINGS
    @given(binary(max_size=16), integers(1, Documented.SIZE))
    def test_raises_decode_error_when_the_data_ends_before_a_documented(
        self, data: bytes, missing: int
    ) -> None:
        offset = max(len(data) + missing - Documented.SIZE, 0)

        with self.assertRaises(DecodeError):
            Documented.read(data, offset)


if __name__ == "__main__":
    unittest.main()
