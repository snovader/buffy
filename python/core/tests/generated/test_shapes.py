"""The code generated from ``schemas/shapes.buffy`` writes and reads tables
with buffy_core."""

import unittest

from hypothesis import given
from hypothesis.strategies import binary, integers

from buffy_core import DecodeError
from tests.generated import SETTINGS
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
from tests.generated.shapes.shapes import (
    Document,
    DocumentFields,
    ItemList,
    ItemListFields,
    Point,
    PointFields,
    Sample,
)
from tests.generated.shapes_tables import (
    DOCUMENT_VERSION,
    describe_document,
    describe_item_list,
    describe_point,
    describe_sample,
    document_fields,
    item_list_fields,
    point_fields,
    read_document,
    read_item_list,
    read_point,
    sample_bytes,
    samples,
)
from tests.tables import build_table

_OTHER_VERSIONS = integers(0, 0xFFFF_FFFF).filter(
    lambda version: version != DOCUMENT_VERSION
)


class TestRoundTrip(unittest.TestCase):
    @SETTINGS
    @given(point_fields, slices)
    def test_reads_back_the_fields_of_a_point_wherever_its_bytes_lie(
        self, fields: PointFields, slice: Slice
    ) -> None:
        data = slice.of(Point.serialize(**fields))

        assert_same_table(
            self, describe_point(read_point(Point(data))), describe_point(fields)
        )

    @SETTINGS
    @given(document_fields, slices)
    def test_reads_back_a_document_and_its_nested_columns_wherever_its_bytes_lie(
        self, fields: DocumentFields, slice: Slice
    ) -> None:
        data = slice.of(Document.serialize(**fields))

        assert_same_table(
            self,
            describe_document(read_document(Document(data))),
            describe_document(fields),
        )

    @SETTINGS
    @given(item_list_fields, slices)
    def test_reads_back_the_items_of_an_item_list_and_their_pairs_wherever_its_bytes_lie(
        self, fields: ItemListFields, slice: Slice
    ) -> None:
        data = slice.of(ItemList.serialize(**fields))

        assert_same_table(
            self,
            describe_item_list(read_item_list(ItemList(data))),
            describe_item_list(fields),
        )


class TestTablesBuiltByHand(unittest.TestCase):
    @SETTINGS
    @given(point_fields)
    def test_writes_the_bytes_of_a_point_that_table_builder_writes(
        self, fields: PointFields
    ) -> None:
        by_hand = bytes(build_table(describe_point(fields)).bytes())

        self.assertEqual(bytes(Point.serialize(**fields)), by_hand)

    @SETTINGS
    @given(document_fields)
    def test_writes_the_bytes_of_a_sized_versioned_document_that_table_builder_writes(
        self, fields: DocumentFields
    ) -> None:
        by_hand = bytes(build_table(describe_document(fields)).bytes())

        self.assertEqual(bytes(Document.serialize(**fields)), by_hand)

    @SETTINGS
    @given(item_list_fields)
    def test_writes_the_bytes_of_an_item_list_that_table_builder_writes(
        self, fields: ItemListFields
    ) -> None:
        by_hand = bytes(build_table(describe_item_list(fields)).bytes())

        self.assertEqual(bytes(ItemList.serialize(**fields)), by_hand)


class TestDecodeErrors(unittest.TestCase):
    @SETTINGS
    @given(document_fields, _OTHER_VERSIONS)
    def test_rejects_a_document_of_another_version(
        self, fields: DocumentFields, version: int
    ) -> None:
        data = build_table(describe_document(fields, version)).bytes()

        with self.assertRaises(DecodeError):
            Document(data)

    @SETTINGS
    @given(point_fields, corruptions)
    def test_reads_a_corrupted_point_as_values_or_raises_decode_error(
        self, fields: PointFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Point.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_point(Point(data)))

    @SETTINGS
    @given(document_fields, corruptions)
    def test_reads_a_corrupted_document_as_values_or_raises_decode_error(
        self, fields: DocumentFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(Document.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_document(Document(data)))

    @SETTINGS
    @given(item_list_fields, corruptions)
    def test_reads_a_corrupted_item_list_as_values_or_raises_decode_error(
        self, fields: ItemListFields, corruption: Truncation | Overwrite
    ) -> None:
        data = corruption.apply(bytes(ItemList.serialize(**fields)))

        assert_values_or_decode_error(lambda: read_item_list(ItemList(data)))

    @SETTINGS
    @given(binary(max_size=64))
    def test_reads_arbitrary_bytes_as_any_message_or_raises_decode_error(
        self, data: bytes
    ) -> None:
        assert_values_or_decode_error(lambda: read_point(Point(data)))
        assert_values_or_decode_error(lambda: read_document(Document(data)))
        assert_values_or_decode_error(lambda: read_item_list(ItemList(data)))


class TestSample(unittest.TestCase):
    @SETTINGS
    @given(samples)
    def test_writes_its_fields_little_endian_at_fixed_offsets_in_12_bytes(
        self, sample: Sample
    ) -> None:
        self.assertEqual(Sample.SIZE, 12)
        self.assertEqual(sample.serialize(), sample_bytes(sample))

    @SETTINGS
    @given(samples, binary(max_size=8), binary(max_size=8))
    def test_reads_a_sample_back_at_an_offset_in_longer_data(
        self, sample: Sample, before: bytes, after: bytes
    ) -> None:
        data = before + sample.serialize() + after

        read = Sample.read(data, len(before))
        assert_same_fields(self, describe_sample(read), describe_sample(sample))

    @SETTINGS
    @given(binary(max_size=24), integers(1, Sample.SIZE))
    def test_raises_decode_error_when_the_data_ends_before_the_sample(
        self, data: bytes, missing: int
    ) -> None:
        # Where the sample would end ``missing`` bytes after the data, or at
        # the start of data shorter than a sample
        offset = max(len(data) + missing - Sample.SIZE, 0)

        with self.assertRaises(DecodeError):
            Sample.read(data, offset)

    # Struct.unpack_from() would read a negative offset from the end of the
    # data, so Sample.read(data, -12) once read the last sample
    @SETTINGS
    @given(samples, integers(1, 64))
    def test_raises_decode_error_at_a_negative_offset(
        self, sample: Sample, distance: int
    ) -> None:
        data = sample.serialize() * 4

        with self.assertRaises(DecodeError):
            Sample.read(data, -distance)

    def test_raises_value_error_for_a_value_that_doesnt_fit_its_field(self) -> None:
        for sample in [
            Sample(2**32, 0.0, 0.0),
            Sample(-1, 0.0, 0.0),
            Sample(0, 1e300, 0.0),
        ]:
            with self.subTest(sample=sample), self.assertRaises(ValueError):
                sample.serialize()


if __name__ == "__main__":
    unittest.main()
