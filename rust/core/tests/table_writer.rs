//! A `TableSizer` and its `TableWriter` write the bytes that `TableBuilder`
//! builds from the same fields, with nested tables written in place, and
//! fail as it fails: for a table too long for its offsets, a header too long
//! and a version too large. A writer given other fields than the sizer added
//! fails rather than writing past the bytes, and no calls make it panic.

mod common;

use std::mem::discriminant;

use common::{
    description::{Field, Layout, Nested, ScalarType, Source, Table},
    strategy,
};
use proptest::{
    collection::vec,
    prelude::*,
    sample::{Index, select},
};
use tafla::{
    Alignment, BuildError, OffsetSize, TableBuilder, TableLayout, TableSizer, TableWriter,
    VectorPosition, WriterParent,
};

proptest! {
    #![proptest_config(common::config())]

    #[test]
    fn test_writes_the_bytes_that_table_builder_builds_from_the_same_fields(
        table in in_place_tables(strategy::layout()),
    ) {
        let built = table.to_bytes();
        prop_assume!(built.is_ok());

        prop_assert_eq!(table.write(), built);
    }

    /// TableBuilder reports the error of a nested table too long for its
    /// offsets, which the sizer counts as part of the root, so only the
    /// errors' variants are the same, and the field of the root that fails
    #[test]
    fn test_fails_as_table_builder_fails_for_any_table_too_long_for_its_offsets(
        table in in_place_tables(strategy::layouts(select(SHORT_OFFSET_SIZES.to_vec()))),
        length in 0..=300_usize,
    ) {
        let mut longer = table;
        longer.fields.push(Field::U8Array(vec![0; length]));

        match (longer.write(), longer.to_bytes()) {
            (Ok(written), Ok(built)) => prop_assert_eq!(written, built),
            (Err(written), Err(built)) => {
                prop_assert_eq!(discriminant(&written), discriminant(&built));
                if let BuildError::TooLong { index, .. } = written {
                    prop_assert_eq!(Some(index), failing_field(&longer));
                }
            }
            (written, built) => prop_assert!(false, "{:?} rather than {:?}", written, built),
        }
    }

    /// The fields' lengths don't depend on where they start after the
    /// bytes, so that the bytes can make the table exactly as long as its
    /// offsets address
    #[test]
    fn test_writes_a_table_as_long_as_its_offsets_address_and_fails_for_one_a_byte_longer(
        table in in_place_tables(short_layouts()),
        last in unpadded_field(),
    ) {
        let offset_size = table.layout.offset_size;
        let max = offset_size.max_value() as usize;
        let with_bytes = |length: usize| {
            let mut table = table.clone();
            table.fields.extend([Field::U8Array(vec![0; length]), last.clone()]);
            table.nested_in_place()
        };

        let shortest = with_bytes(0).to_bytes();
        prop_assume!(shortest.is_ok());
        let room = max - shortest.unwrap().len();

        let longest = with_bytes(room);
        let bytes = longest.write()?;
        prop_assert_eq!(bytes.len(), max);
        // Compared as a whole, since a failure would print 64 KiB
        prop_assert!(bytes == longest.to_bytes()?);

        let error = BuildError::TooLong {
            index: table.fields.len() + 1,
            length: max as u64 + 1,
            offset_size,
        };
        let too_long = with_bytes(room + 1);
        prop_assert_eq!(too_long.write(), Err(error));
        prop_assert_eq!(too_long.to_bytes(), Err(error));
    }

    /// A writer given a field of another length than the sizer added fails,
    /// and one given a field of the same length writes it as TableBuilder
    /// does, since the bytes come from the fields written
    #[test]
    fn test_a_writer_of_other_fields_than_those_sized_fails_or_writes_them_as_table_builder_does(
        table in in_place_tables(strategy::layout()),
        replacements in strategy::table(),
        at in any::<Index>(),
        with in any::<Index>(),
    ) {
        prop_assume!(!table.fields.is_empty() && !replacements.fields.is_empty());
        prop_assume!(table.to_bytes().is_ok());

        let mut written = table.clone();
        let replacement = with.get(&replacements.fields).clone();
        written.fields[at.index(table.fields.len())] = replacement;
        let written = written.nested_in_place();

        match table.write_as(&written) {
            Ok(bytes) => prop_assert_eq!(Ok(bytes), written.to_bytes()),
            Err(error) => prop_assert!(
                matches!(error, BuildError::NotAsSized { length, .. }
                    if Ok(length) == table.to_bytes().map(|bytes| bytes.len())),
                "{:?}",
                error
            ),
        }
    }

    /// Calls in any order, with any indices, counts and items, write a table
    /// or fail with an error, and never write past its length
    #[test]
    fn test_no_calls_make_a_writer_panic(
        table in strategy::table(),
        calls in vec(call(), 0..=24),
    ) {
        let Ok(sizer) = sized(&table) else {
            return Ok(());
        };
        let length = sizer.byte_length()?;
        let mut writer = sizer.writer()?;

        let mut parents = Vec::new();
        let mut vectors = Vec::new();
        for call in calls {
            call.apply(&mut writer, &mut parents, &mut vectors);
        }

        if let Ok(bytes) = writer.into_bytes() {
            prop_assert_eq!(bytes.len(), length);
        }
    }
}

const SHORT_OFFSET_SIZES: [OffsetSize; 2] = [OffsetSize::U8, OffsetSize::U16];

/// Tables of the layouts that `layouts` generates whose nested tables a
/// `TableWriter` writes in place
fn in_place_tables(
    layouts: impl Strategy<Value = Layout> + 'static,
) -> impl Strategy<Value = Table> {
    strategy::tables(layouts).prop_map(|table| table.nested_in_place())
}

/// Layouts of 1- and 2-byte offsets, whose limits a test can reach. Under
/// Miri, only of 1-byte offsets, since writing 64 KiB takes it minutes.
fn short_layouts() -> impl Strategy<Value = Layout> {
    let offset_sizes = if cfg!(miri) {
        vec![OffsetSize::U8]
    } else {
        SHORT_OFFSET_SIZES.to_vec()
    };

    strategy::layouts(select(offset_sizes))
}

/// Fields after which a table's length doesn't depend on where they start:
/// those that aren't padded, from tables of 1-byte offsets, which hold
/// few enough values to fit after others more often than not
fn unpadded_field() -> impl Strategy<Value = Field> {
    let unpadded = |field: &Field| {
        field.alignment() == Alignment::One && !matches!(field, Field::Table { .. })
    };

    strategy::tables(strategy::layouts(Just(OffsetSize::U8)))
        .prop_filter_map("a table of padded fields", move |table| {
            table.fields.into_iter().find(unpadded)
        })
}

/// The index of the root field that `TableBuilder` fails to set, whether
/// the root or a table nested in that field is too long
fn failing_field(table: &Table) -> Option<usize> {
    let mut builder = table.builder().ok()?;
    table
        .fields
        .iter()
        .position(|field| field.set(&mut builder).is_err())
}

/// A sizer of the table with every field added
fn sized(table: &Table) -> Result<TableSizer, BuildError> {
    let mut sizer = table.sizer()?;
    for field in &table.fields {
        field.add(&mut sizer);
    }

    Ok(sizer)
}

/// A call of a writer, with any index, count or item
#[derive(Clone, Debug)]
enum Call {
    U8 {
        index: usize,
    },
    F64Array {
        index: usize,
        count: usize,
    },
    Str {
        index: usize,
        length: usize,
    },
    StructArray {
        index: usize,
        length: usize,
        size: usize,
    },
    Table {
        index: usize,
        alignment: Alignment,
    },
    StartTable {
        fields: usize,
    },
    EndTable,
    Vector {
        index: usize,
        count: usize,
    },
    ItemEnd {
        item: usize,
    },
}

fn call() -> impl Strategy<Value = Call> {
    // Mostly small, so that they fit, and sometimes any
    let small = || prop_oneof![4 => 0..8_usize, 1 => any::<usize>()];
    let alignments = select(vec![
        Alignment::One,
        Alignment::Two,
        Alignment::Four,
        Alignment::Eight,
    ]);

    prop_oneof![
        small().prop_map(|index| Call::U8 { index }),
        (small(), 0..8_usize).prop_map(|(index, count)| Call::F64Array { index, count }),
        (small(), 0..8_usize).prop_map(|(index, length)| Call::Str { index, length }),
        (small(), 0..12_usize, 0..4_usize).prop_map(|(index, length, size)| Call::StructArray {
            index,
            length,
            size
        }),
        (small(), alignments).prop_map(|(index, alignment)| Call::Table { index, alignment }),
        small().prop_map(|fields| Call::StartTable { fields }),
        Just(Call::EndTable),
        (small(), small()).prop_map(|(index, count)| Call::Vector { index, count }),
        small().prop_map(|item| Call::ItemEnd { item }),
    ]
}

impl Call {
    fn apply(
        self,
        writer: &mut TableWriter,
        parents: &mut Vec<WriterParent>,
        vectors: &mut Vec<VectorPosition>,
    ) {
        match self {
            Self::U8 { index } => writer.set_u8(index, 1),
            Self::F64Array { index, count } => writer.set_f64_array(index, &vec![-0.0; count]),
            Self::Str { index, length } => writer.set_str(index, &"🦀".repeat(length)),
            Self::StructArray {
                index,
                length,
                size,
            } => writer.set_struct_array(index, &vec![1; length], size, Alignment::Two),
            Self::Table { index, alignment } => writer.set_table(index, alignment),
            Self::StartTable { fields } => parents.push(writer.start_table(fields)),
            Self::EndTable => {
                if let Some(parent) = parents.pop() {
                    writer.end_table(parent);
                }
            }
            Self::Vector { index, count } => vectors.push(writer.set_vector(index, count)),
            Self::ItemEnd { item } => {
                if let Some(&vector) = vectors.last() {
                    writer.set_item_end(vector, item);
                }
            }
        }
    }
}

#[test]
fn test_writes_a_struct_array_a_nested_table_and_a_vector_as_table_builder_pads_them() {
    // A uint8 leaves a uint32[] 3 bytes short of its alignment, and a uint16
    // a nested table of float64s 1 byte short of its own
    let nested = |fields| Nested {
        from: Source::Builder,
        table: Table {
            layout: Layout::headerless(OffsetSize::U16),
            version: None,
            fields,
        },
    };
    let table = Table {
        layout: Layout {
            sized: true,
            versioned: true,
            offset_size: OffsetSize::U16,
        },
        version: Some(7),
        fields: vec![
            Field::U8(1),
            Field::U32Array(vec![2, 3]),
            Field::StructArray {
                types: vec![ScalarType::U16, ScalarType::F64],
                items: vec![vec![Field::U16(4), Field::F64(5.0)]],
            },
            Field::Table {
                nested: nested(vec![Field::F64Array(vec![6.0])]),
                alignment: None,
            },
            Field::Vector(vec![nested(vec![Field::String("🦀".to_owned())])]),
        ],
    };

    let bytes = table.write().unwrap();
    assert_eq!(bytes, table.to_bytes().unwrap());
    assert_eq!(common::read::read_table(&bytes, &table), Ok(table));
}

#[test]
fn test_names_the_root_field_that_makes_the_table_too_long_with_the_length_after_it() {
    // A vector of three items of 102 bytes, which is too long for 1-byte
    // offsets after its second item, and makes the table 314 bytes
    let item = Nested {
        from: Source::Builder,
        table: Table {
            layout: Layout::headerless(OffsetSize::U8),
            version: None,
            fields: vec![Field::U8Array(vec![0; 100])],
        },
    };
    let table = Table {
        layout: Layout::headerless(OffsetSize::U8),
        version: None,
        fields: vec![Field::U8(1), Field::Vector(vec![item; 3]), Field::U8(2)],
    };

    let error = BuildError::TooLong {
        index: 1,
        length: 314,
        offset_size: OffsetSize::U8,
    };
    assert_eq!(table.write(), Err(error));
    assert_eq!(table.to_bytes(), Err(error));
}

#[test]
fn test_rejects_a_table_of_32_bit_offsets_past_4_gib_before_allocating_it() {
    // The table would hold its offset, its count and 2^30 uint32s
    let mut sizer = TableSizer::new(1, TableLayout::DEFAULT).unwrap();
    sizer.add_u32_array(1 << 30);

    let error = BuildError::TooLong {
        index: 0,
        length: 4_294_967_304,
        offset_size: OffsetSize::U32,
    };
    assert_eq!(sizer.byte_length(), Err(error));
    assert_eq!(sizer.writer().unwrap_err(), error);
    assert_eq!(
        error.to_string(),
        "Field 1 makes the table 4294967304 bytes, too long for 4-byte offsets"
    );
}

#[test]
fn test_counts_lengths_past_any_usize_without_overflowing() {
    let mut sizer = TableSizer::new(3, TableLayout::DEFAULT).unwrap();
    sizer.add_u8();
    sizer.add_f64_array(usize::MAX);
    sizer.add_vector(usize::MAX);
    for _ in 0..3 {
        let parent = sizer.start_table(usize::MAX);
        sizer.add_struct_array(usize::MAX, usize::MAX, Alignment::Eight);
        sizer.add_table(Alignment::Eight);
        sizer.end_table(parent);
    }

    assert!(matches!(
        sizer.writer(),
        Err(BuildError::TooLong {
            index: 1,
            offset_size: OffsetSize::U32,
            ..
        })
    ));
}

#[test]
fn test_rejects_a_header_too_long_for_its_offsets_as_table_builder_does() {
    for (offset_size, max) in [(OffsetSize::U8, 0xff), (OffsetSize::U16, 0xffff)] {
        let width = offset_size.bytes();

        for sized in [false, true] {
            let layout = TableLayout::DEFAULT.with_offset_size(offset_size);
            let layout = if sized { layout.sized() } else { layout };
            let fields = max / width - usize::from(sized);

            assert!(TableSizer::new(fields, layout).is_ok());
            assert_eq!(
                TableSizer::new(fields + 1, layout).unwrap_err(),
                TableBuilder::new(fields + 1, layout).unwrap_err()
            );

            let versioned = layout.versioned();
            assert!(TableSizer::versioned(fields - 1, versioned, 0).is_ok());
            assert_eq!(
                TableSizer::versioned(fields, versioned, 0).unwrap_err(),
                TableBuilder::versioned(fields, versioned, 0).unwrap_err()
            );
        }
    }

    assert_eq!(
        TableSizer::new(usize::MAX, TableLayout::DEFAULT).unwrap_err(),
        TableBuilder::new(usize::MAX, TableLayout::DEFAULT).unwrap_err()
    );
}

#[test]
fn test_rejects_a_version_too_large_for_its_slot_as_table_builder_does() {
    for (offset_size, version) in [(OffsetSize::U8, 0x100), (OffsetSize::U16, 0x1_0000)] {
        let layout = TableLayout::DEFAULT
            .with_offset_size(offset_size)
            .versioned();

        let error = BuildError::VersionTooLarge {
            version,
            offset_size,
        };
        assert_eq!(
            TableSizer::versioned(1, layout, version).unwrap_err(),
            error
        );
        assert_eq!(
            TableBuilder::versioned(1, layout, version).unwrap_err(),
            error
        );
        assert!(TableSizer::versioned(1, layout, version - 1).is_ok());
    }
}

#[test]
fn test_fails_for_a_string_longer_than_the_one_sized() {
    let mut sizer = TableSizer::new(1, TableLayout::DEFAULT).unwrap();
    sizer.add_str("a");
    let mut writer = sizer.writer().unwrap();
    writer.set_str(0, "ab");

    // The offset and length were written before the string didn't fit
    let error = BuildError::NotAsSized {
        written: 8,
        length: 9,
    };
    assert_eq!(writer.into_bytes(), Err(error));
}

#[test]
fn test_fails_for_fields_shorter_than_those_sized() {
    let mut sizer = TableSizer::new(1, TableLayout::DEFAULT).unwrap();
    sizer.add_str("ab");
    let mut writer = sizer.writer().unwrap();
    writer.set_str(0, "a");

    let error = writer.into_bytes().unwrap_err();
    assert_eq!(
        error,
        BuildError::NotAsSized {
            written: 9,
            length: 10
        }
    );
    assert_eq!(
        error.to_string(),
        "The fields wrote 9 of the table's 10 bytes"
    );
}

#[test]
fn test_fails_for_a_field_past_its_tables_fields() {
    let mut sizer = TableSizer::new(2, TableLayout::DEFAULT).unwrap();
    sizer.add_u8();
    sizer.add_table(Alignment::One);
    let parent = sizer.start_table(1);
    sizer.add_u8();
    sizer.end_table(parent);

    let mut writer = sizer.writer().unwrap();
    writer.set_u8(0, 1);
    writer.set_table(1, Alignment::One);
    let parent = writer.start_table(1);
    writer.set_u8(1, 2);
    writer.end_table(parent);

    assert_eq!(
        writer.into_bytes(),
        Err(BuildError::TooManyFields { fields: 1 })
    );
}

#[test]
fn test_fails_for_an_item_past_its_vectors_items() {
    let mut sizer = TableSizer::new(1, TableLayout::DEFAULT).unwrap();
    sizer.add_vector(1);
    let parent = sizer.start_table(0);
    sizer.end_table(parent);

    let mut writer = sizer.writer().unwrap();
    let vector = writer.set_vector(0, 1);
    let parent = writer.start_table(0);
    writer.end_table(parent);
    writer.set_item_end(vector, 1);

    assert_eq!(
        writer.into_bytes(),
        Err(BuildError::NotAsSized {
            written: 12,
            length: 12
        })
    );
}

#[test]
fn test_fails_for_bytes_that_arent_whole_structs_as_table_builder_does() {
    for (length, size) in [(5, 2), (4, 0), (0, 0)] {
        let mut sizer = TableSizer::new(1, TableLayout::DEFAULT).unwrap();
        sizer.add_struct_array(2, 2, Alignment::One);
        let mut writer = sizer.writer().unwrap();
        writer.set_struct_array(0, &vec![0; length], size, Alignment::One);

        let mut builder = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
        let error = builder
            .set_struct_array(vec![0; length], size, Alignment::One)
            .unwrap_err();
        assert_eq!(writer.into_bytes(), Err(error));
    }
}
