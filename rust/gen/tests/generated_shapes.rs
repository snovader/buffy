//! The code generated from `schemas/shapes.buffy` writes the tables that
//! `TableBuilder` writes by hand, reads back what it writes, and reads
//! corrupted and arbitrary bytes as values or decode errors without
//! panicking. `typescript/core/test/generated/shapes.test.ts` checks the
//! generated TypeScript and JavaScript alike.

mod shapes;
mod tables;

use buffy_core::{DecodeError, TableLayout};
use proptest::collection::vec;
use proptest::prelude::*;
use shapes::generated::{
    ColumnsFields, Document, DocumentFields, ItemFields, ItemList, ItemListFields, PairFields,
    Point, PointFields, Sample,
};
use shapes::{
    describe_document, describe_item_list, describe_point, describe_sample, read_document,
    read_item_list, read_point,
};
use tables::{
    CASES, Header, arbitrary_bytes, arrays, build, check_truncated, expected, float32s, float64s,
    integers, overwrites, placements, strings, truncations,
};

// The headers of the messages, from their attributes in the schema
const POINT: Header = Header::new(TableLayout::DEFAULT);
const DOCUMENT: Header = Header::versioned(TableLayout::DEFAULT.sized(), 1);
const ITEM_LIST: Header = Header::new(TableLayout::DEFAULT);

/// The values of a `Point`'s fields, which `PointFields` borrows
#[derive(Clone, Debug)]
struct OwnedPoint {
    x: f64,
    y: f64,
    id: i64,
    name: String,
}

impl OwnedPoint {
    fn fields(&self) -> PointFields<'_> {
        PointFields {
            x: self.x,
            y: self.y,
            id: self.id,
            name: &self.name,
        }
    }
}

/// The values of a `Document`'s fields and of its `Columns`
#[derive(Clone, Debug)]
struct OwnedDocument {
    x: f64,
    y: f64,
    name: String,
    base: i64,
    keys: Vec<u32>,
    a: Vec<f32>,
    b: Vec<f32>,
}

impl OwnedDocument {
    fn fields(&self) -> DocumentFields<'_> {
        DocumentFields {
            x: self.x,
            y: self.y,
            name: &self.name,
            columns: ColumnsFields {
                base: self.base,
                keys: &self.keys,
                a: &self.a,
                b: &self.b,
            },
        }
    }
}

fn points() -> impl Strategy<Value = OwnedPoint> {
    let ids = integers(i64::MIN, i64::MAX);

    (float64s(), float64s(), ids, strings()).prop_map(|(x, y, id, name)| OwnedPoint {
        x,
        y,
        id,
        name,
    })
}

fn documents() -> impl Strategy<Value = OwnedDocument> {
    let columns = (
        integers(i64::MIN, i64::MAX),
        arrays(integers(0, u32::MAX)),
        arrays(float32s()),
        arrays(float32s()),
    );

    (float64s(), float64s(), strings(), columns).prop_map(|(x, y, name, (base, keys, a, b))| {
        OwnedDocument {
            x,
            y,
            name,
            base,
            keys,
            a,
            b,
        }
    })
}

/// The items of an `ItemList`, empty too
fn items() -> impl Strategy<Value = Vec<ItemFields>> {
    let pairs = (float32s(), float32s()).prop_map(|(first, second)| PairFields { first, second });
    let items = (float32s(), pairs).prop_map(|(value, pair)| ItemFields { value, pair });

    arrays(items)
}

fn samples() -> impl Strategy<Value = Sample> {
    (integers(0, u32::MAX), float32s(), float32s()).prop_map(|(key, a, b)| Sample { key, a, b })
}

/// Versions other than the `Document`'s, its limits among them
fn other_versions() -> impl Strategy<Value = u32> {
    integers(0, u32::MAX).prop_filter("the Document's version", |&version| {
        Some(version) != DOCUMENT.version
    })
}

/// A `Sample`'s fields little-endian at their offsets, one after another,
/// written with the standard library rather than the generated code
fn sample_bytes(sample: &Sample) -> Vec<u8> {
    [
        &sample.key.to_le_bytes()[..],
        &sample.a.to_le_bytes(),
        &sample.b.to_le_bytes(),
    ]
    .concat()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    // Round trips

    #[test]
    fn a_point_reads_back_its_fields_wherever_its_bytes_lie(
        point in points(),
        placement in placements(),
    ) {
        let fields = point.fields();
        let bytes = placement.place(&fields.serialize()?);

        let read = read_point(&Point::new(bytes.bytes()));
        prop_assert_eq!(read, expected(describe_point(&fields)));
    }

    #[test]
    fn a_document_reads_back_its_fields_and_its_columns_wherever_its_bytes_lie(
        document in documents(),
        placement in placements(),
    ) {
        let fields = document.fields();
        let bytes = placement.place(&fields.serialize()?);

        let read = read_document(&Document::new(bytes.bytes())?);
        prop_assert_eq!(read, expected(describe_document(&fields)));
    }

    #[test]
    fn an_item_list_reads_back_its_items_and_their_pairs_wherever_its_bytes_lie(
        items in items(),
        placement in placements(),
    ) {
        let fields = ItemListFields { items: &items };
        let bytes = placement.place(&fields.serialize()?);

        let read = read_item_list(&ItemList::new(bytes.bytes()));
        prop_assert_eq!(read, expected(describe_item_list(&fields)));
    }

    // Tables built by hand

    #[test]
    fn a_point_is_written_as_table_builder_writes_it(point in points()) {
        let fields = point.fields();

        prop_assert_eq!(fields.serialize(), build(POINT, &describe_point(&fields)));
    }

    #[test]
    fn a_document_is_written_sized_and_versioned_as_table_builder_writes_it(
        document in documents(),
    ) {
        let fields = document.fields();

        prop_assert_eq!(fields.serialize(), build(DOCUMENT, &describe_document(&fields)));
    }

    #[test]
    fn an_item_list_is_written_as_table_builder_writes_it(items in items()) {
        let fields = ItemListFields { items: &items };

        prop_assert_eq!(fields.serialize(), build(ITEM_LIST, &describe_item_list(&fields)));
    }

    // Decode errors

    #[test]
    fn a_document_of_another_version_is_rejected_with_unsupported_version(
        document in documents(),
        version in other_versions(),
    ) {
        let values = describe_document(&document.fields());
        let bytes = build(DOCUMENT.with_version(version), &values)?;

        let error = Document::new(&bytes).err();
        prop_assert_eq!(error, Some(DecodeError::UnsupportedVersion { version, supported: 1 }));
    }

    #[test]
    fn a_corrupted_point_reads_as_values_or_decode_errors(
        point in points(),
        overwrite in overwrites(),
    ) {
        let fields = point.fields();
        let bytes = fields.serialize()?;
        let written = describe_point(&fields);

        for truncated in truncations(&bytes) {
            check_truncated(Ok(read_point(&Point::new(truncated))), &written)?;
        }

        // Any values can be read from overwritten bytes, as long as no getter
        // panics
        read_point(&Point::new(&overwrite.apply(&bytes)));
    }

    #[test]
    fn a_corrupted_document_reads_as_values_or_decode_errors(
        document in documents(),
        overwrite in overwrites(),
    ) {
        let fields = document.fields();
        let bytes = fields.serialize()?;
        let written = describe_document(&fields);

        for truncated in truncations(&bytes) {
            let read = Document::new(truncated).map(|document| read_document(&document));
            check_truncated(read, &written)?;
        }

        // Any values can be read from overwritten bytes, as long as no getter
        // panics: as they are, and with their version restored, so that the
        // getters read them
        let overwritten = overwrite.apply(&bytes);
        for bytes in [overwritten.clone(), DOCUMENT.write_version(&overwritten)] {
            let _ = Document::new(&bytes).map(|document| read_document(&document));
        }
    }

    #[test]
    fn a_corrupted_item_list_reads_as_values_or_decode_errors(
        items in items(),
        overwrite in overwrites(),
    ) {
        let fields = ItemListFields { items: &items };
        let bytes = fields.serialize()?;
        let written = describe_item_list(&fields);

        for truncated in truncations(&bytes) {
            check_truncated(Ok(read_item_list(&ItemList::new(truncated))), &written)?;
        }

        read_item_list(&ItemList::new(&overwrite.apply(&bytes)));
    }

    #[test]
    fn arbitrary_bytes_read_as_any_message_or_decode_errors(bytes in arbitrary_bytes()) {
        read_point(&Point::new(&bytes));
        read_item_list(&ItemList::new(&bytes));

        // As they are, and with the Document's version, so that its getters
        // read them
        for bytes in [bytes.clone(), DOCUMENT.write_version(&bytes)] {
            let _ = Document::new(&bytes).map(|document| read_document(&document));
        }
    }

    // The struct

    #[test]
    fn a_sample_is_written_in_12_bytes_at_fixed_offsets_and_read_back_wherever_they_lie(
        sample in samples(),
        placement in placements(),
    ) {
        let bytes = sample.to_bytes();
        prop_assert_eq!(Sample::SIZE, 12);
        prop_assert_eq!(bytes.to_vec(), sample_bytes(&sample));

        let read = Sample::read(placement.place(&bytes).bytes())?;
        prop_assert_eq!(describe_sample(&read), describe_sample(&sample));
    }

    #[test]
    fn a_sample_is_read_from_the_start_of_longer_bytes(
        sample in samples(),
        after in vec(any::<u8>(), 1..=8),
    ) {
        let bytes = [&sample.to_bytes()[..], &after].concat();

        let read = Sample::read(&bytes)?;
        prop_assert_eq!(describe_sample(&read), describe_sample(&sample));
    }

    #[test]
    fn a_sample_fails_to_read_from_fewer_than_12_bytes(
        bytes in vec(any::<u8>(), 0..Sample::SIZE),
    ) {
        let read = Sample::read(&bytes);
        prop_assert!(matches!(read, Err(DecodeError::OutOfRange { .. })), "{:?}", read);
    }
}
