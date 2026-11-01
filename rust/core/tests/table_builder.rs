mod common;

use buffy_core::{BuildError, ByteBuffer, OffsetSize, TableBuilder, TableLayout};
use common::Field;

#[test]
fn test_reject_field_beyond_count() {
    let mut builder = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    builder.set_u8(1).unwrap();
    let bytes = builder.to_bytes().unwrap();

    let error = builder.set_u8(2).unwrap_err();
    assert_eq!(error, BuildError::TooManyFields { fields: 1 });
    assert_eq!(error.to_string(), "Can't add field 2 to a table of 1");
    assert_eq!(builder.to_bytes().unwrap(), bytes);
}

#[test]
fn test_reject_writing_before_every_field_is_set() {
    for set in 0..5 {
        let mut builder = TableBuilder::new(set + 1, TableLayout::DEFAULT).unwrap();
        for _ in 0..set {
            builder.set_u8(1).unwrap();
        }

        let error = builder.to_bytes().unwrap_err();
        assert_eq!(
            error,
            BuildError::Incomplete {
                set,
                fields: set + 1
            }
        );
        assert_eq!(
            error.to_string(),
            format!("Only {set} of the table's {} fields were set", set + 1)
        );
    }
}

#[test]
fn test_write_the_same_bytes_each_time() {
    let mut builder = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    builder.set_str("once").unwrap();

    assert_eq!(builder.to_bytes().unwrap(), builder.to_bytes().unwrap());
}

/// A field of `length` units, and the bytes that each unit adds for an
/// offset size
struct LongField {
    name: &'static str,
    unit: fn(usize) -> usize,
    field: fn(usize) -> Field,
}

const LONG_FIELDS: [LongField; 5] = [
    LongField {
        name: "a byte array",
        unit: |_| 1,
        field: |length| Field::U8Array(vec![0; length]),
    },
    LongField {
        name: "a float array",
        unit: |_| 8,
        field: |length| Field::F64Array(vec![0.0; length]),
    },
    LongField {
        name: "a string",
        unit: |_| 1,
        field: |length| Field::String("a".repeat(length)),
    },
    LongField {
        name: "a vector's count",
        unit: |width| width,
        field: |length| Field::Vector(vec![Vec::new(); length]),
    },
    LongField {
        name: "a vector item",
        unit: |_| 1,
        field: |length| Field::Vector(vec![vec![0; length]]),
    },
];

#[test]
fn test_fit_field_up_to_offset_limit() {
    for (offset_size, max) in [(OffsetSize::U8, 0xff), (OffsetSize::U16, 0xffff)] {
        for layout in [
            TableLayout::DEFAULT.with_offset_size(offset_size),
            TableLayout::DEFAULT.sized().with_offset_size(offset_size),
        ] {
            for LongField { name, unit, field } in &LONG_FIELDS {
                let unit = unit(offset_size.bytes());

                let empty_field = field(0);
                let empty = common::table_of(std::slice::from_ref(&empty_field), layout);
                let length = (max - empty.len()) / unit;

                let longest_field = field(length);
                let longest = common::table_of(std::slice::from_ref(&longest_field), layout);
                assert!(longest.len() <= max, "{name}");
                assert!(longest.len() > max - unit, "{name}");

                let buffer = ByteBuffer::new(&longest, layout);
                common::assert_fields(&buffer, &[longest_field]);

                // The setter fails and leaves the builder as it was
                let too_long_field = field(length + 1);
                let mut too_long = TableBuilder::new(1, layout).unwrap();
                let error = too_long_field.set(&mut too_long).unwrap_err();
                assert!(
                    matches!(error, BuildError::TooLong { index: 0, .. }),
                    "{name}"
                );

                empty_field.set(&mut too_long).unwrap();
                assert_eq!(too_long.to_bytes().unwrap(), empty, "{name}");
            }
        }
    }
}

#[test]
fn test_reject_field_that_makes_table_too_long() {
    for (offset_size, max) in [(OffsetSize::U8, 0xff), (OffsetSize::U16, 0xffff)] {
        let layout = TableLayout::DEFAULT.with_offset_size(offset_size);
        let width = offset_size.bytes();

        // Two offsets, the array's length prefix and bytes, and then one byte
        let bytes = vec![0; max - 3 * width - 1];
        let mut fits = TableBuilder::new(2, layout).unwrap();
        fits.set_u8_array(&bytes).unwrap();
        fits.set_u8(7).unwrap();

        let table = fits.to_bytes().unwrap();
        let buffer = ByteBuffer::new(&table, layout);
        assert_eq!(table.len(), max);
        assert_eq!(buffer.get_u8(buffer.root_offset(1).unwrap()).unwrap(), 7);

        let bytes = vec![0; max - 3 * width];
        let mut too_long = TableBuilder::new(2, layout).unwrap();
        too_long.set_u8_array(&bytes).unwrap();

        let error = too_long.set_u8(7).unwrap_err();
        assert_eq!(
            error,
            BuildError::TooLong {
                index: 1,
                length: max as u64 + 1,
                offset_size,
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "Field 2 makes the table {} bytes, too long for {width}-byte offsets",
                max + 1
            )
        );
    }
}

#[test]
fn test_reject_header_too_long() {
    for (offset_size, max) in [(OffsetSize::U8, 0xff), (OffsetSize::U16, 0xffff)] {
        let width = offset_size.bytes();

        for sized in [false, true] {
            let layout = TableLayout::DEFAULT.with_offset_size(offset_size);
            let layout = if sized { layout.sized() } else { layout };
            let fields = max / width - usize::from(sized);

            assert!(TableBuilder::new(fields, layout).is_ok());
            assert_eq!(
                TableBuilder::new(fields + 1, layout).unwrap_err(),
                BuildError::HeaderTooLong {
                    fields: fields + 1,
                    offset_size
                }
            );
        }
    }

    assert!(matches!(
        TableBuilder::new(usize::MAX, TableLayout::DEFAULT),
        Err(BuildError::HeaderTooLong { .. })
    ));
}

// A table of 4 GiB can't be allocated in a test, but a vector that nests a
// builder of 64 MiB 64 times is that long without allocating more
#[test]
#[cfg_attr(miri, ignore = "allocates 64 MiB")]
fn test_reject_table_of_32_bit_offsets_past_4_gib_before_writing() {
    let bytes = vec![0; 1 << 26];
    let mut nested = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    nested.set_u8_array(&bytes).unwrap();

    let mut builder = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    let error = builder.set_vector(vec![&nested; 64]).unwrap_err();
    assert!(matches!(error, BuildError::TooLong { index: 0, length, .. } if length > 1 << 32));

    builder.set_vector(vec![&nested; 63]).unwrap();
}

#[test]
fn test_versioned_header_bytes() {
    // Length, version 7, the field's offset and the field (42), as
    // typescript/core/test/byteBuffer.test.ts writes them
    let cases: [(OffsetSize, bool, &[u8]); 4] = [
        (OffsetSize::U8, true, &[4, 7, 3, 42]),
        (OffsetSize::U16, true, &[7, 0, 7, 0, 6, 0, 42]),
        (
            OffsetSize::U32,
            true,
            &[13, 0, 0, 0, 7, 0, 0, 0, 12, 0, 0, 0, 42],
        ),
        (OffsetSize::U32, false, &[7, 0, 0, 0, 8, 0, 0, 0, 42]),
    ];

    for (offset_size, sized, expected) in cases {
        let layout = TableLayout::DEFAULT.with_offset_size(offset_size);
        let layout = if sized { layout.sized() } else { layout }.versioned();

        let mut builder = TableBuilder::versioned(1, layout, 7).unwrap();
        builder.set_u8(42).unwrap();

        assert_eq!(builder.to_bytes().unwrap(), expected);
    }
}

#[test]
fn test_reject_version_too_large_for_slot() {
    for (offset_size, version) in [(OffsetSize::U8, 0x100), (OffsetSize::U16, 0x1_0000)] {
        let layout = TableLayout::DEFAULT
            .with_offset_size(offset_size)
            .versioned();

        let error = TableBuilder::versioned(1, layout, version).unwrap_err();
        assert_eq!(
            error,
            BuildError::VersionTooLarge {
                version,
                offset_size
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "Version {version} doesn't fit in a {}-byte slot",
                offset_size.bytes()
            )
        );
        assert!(TableBuilder::versioned(1, layout, version - 1).is_ok());
    }
}

#[test]
fn test_reject_incomplete_nested_builder() {
    let mut incomplete = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    incomplete.set_u8(1).unwrap();

    let mut parent = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    let error = BuildError::Incomplete { set: 1, fields: 2 };
    assert_eq!(parent.set_table(&incomplete).unwrap_err(), error);

    let bytes = [0];
    let items = [buffy_core::NestedTable::from(&bytes), (&incomplete).into()];
    assert_eq!(parent.set_vector(items).unwrap_err(), error);

    // The parent was left as it was
    let mut nested = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    nested.set_u8(1).unwrap();
    nested.set_u8(2).unwrap();
    parent.set_table(&nested).unwrap();
    parent.set_vector([&nested]).unwrap();

    let table = parent.to_bytes().unwrap();
    let buffer = ByteBuffer::new(&table, TableLayout::DEFAULT);
    let position = buffer.root_offset(0).unwrap();
    assert_eq!(position, 8);
    assert_eq!(
        buffer
            .get_u8(buffer.field_offset(position, 1).unwrap())
            .unwrap(),
        2
    );

    let vector = buffer.get_vector(buffer.root_offset(1).unwrap()).unwrap();
    assert_eq!(
        vector.iter().collect::<Vec<_>>(),
        [nested.to_bytes().unwrap()]
    );
}

#[test]
fn test_reject_nested_builder_that_makes_parent_too_long() {
    let bytes = vec![0; 0xff - 8];
    let mut nested = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    nested.set_u8_array(&bytes).unwrap();

    let layout = TableLayout::DEFAULT.with_offset_size(OffsetSize::U8);
    let mut parent = TableBuilder::new(1, layout).unwrap();
    assert!(matches!(
        parent.set_table(&nested),
        Err(BuildError::TooLong { .. })
    ));
    assert!(matches!(
        parent.set_vector([&nested]),
        Err(BuildError::TooLong { .. })
    ));

    parent.set_vector(Vec::<Vec<u8>>::new()).unwrap();
    assert_eq!(parent.to_bytes().unwrap(), [1, 0]);
}
