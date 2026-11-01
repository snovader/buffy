//! Readers decode untrusted bytes: any bytes, and valid tables cut short or
//! overwritten, read as values or fail with a `DecodeError`, and never
//! panic. The examples document the errors.

mod common;

use std::{error::Error, iter};

use buffy_core::{ByteBuffer, DecodeError, TableBuilder, TableLayout};
use common::{
    at_byte_offset,
    description::{Field, Table},
    read::{ReadError, read_field, read_root_field, read_table},
    strategy,
};
use proptest::{collection::vec, prelude::*, sample::Index};

proptest! {
    #![proptest_config(common::config())]

    /// A field that lies before the cut reads the value that the table was
    /// built with, and any other fails with `OutOfRange`
    #[test]
    fn test_truncated_tables_read_their_values_or_out_of_range(
        table in strategy::table(),
        cuts in vec(any::<Index>(), 1..=16),
    ) {
        let bytes = table.to_bytes().unwrap();
        prop_assume!(!bytes.is_empty());

        for cut in cuts {
            let length = cut.index(bytes.len());
            // A copy, so that no read can reach the bytes after the cut
            let truncated = bytes[..length].to_vec();

            for (index, field) in table.fields.iter().enumerate() {
                match read_root_field(&truncated, table.layout, index, field) {
                    Ok(read) => prop_assert_eq!(&read, field),
                    Err(ReadError::Decode(DecodeError::OutOfRange { .. })) => {}
                    Err(error) => {
                        prop_assert!(false, "Field {} cut at {}: {:?}", index, length, error);
                    }
                }
            }
        }
    }

    /// Overwritten bytes can hold other values, offsets and counts, which
    /// read as other values, as vectors of another number of items or as a
    /// size other than the table's length, or fail with a `DecodeError`
    #[test]
    fn test_overwritten_tables_read_values_or_decode_errors(
        table in strategy::table(),
        overwrites in vec((any::<Index>(), byte()), 1..=4),
    ) {
        let mut bytes = table.to_bytes().unwrap();
        prop_assume!(!bytes.is_empty());

        for (index, byte) in overwrites {
            *index.get_mut(&mut bytes) = byte;
        }

        let _ = read_table(&bytes, &table);
        for (index, field) in table.fields.iter().enumerate() {
            let _ = read_root_field(&bytes, table.layout, index, field);
        }
    }

    /// Every getter, at the root offsets of a table of any layout, in the
    /// tables nested in it and in its vectors, and at any position and
    /// field offset, returns a value or a `DecodeError`. Bytes read the same
    /// within larger bytes as on their own, at any address, and an
    /// `OutOfRange` error describes a read that would leave them.
    #[test]
    fn test_arbitrary_bytes_read_as_values_or_decode_errors(
        bytes in vec(byte(), 0..64),
        surrounding in (vec(byte(), 0..8), vec(byte(), 0..8)),
        byte_offset in 0..8_usize,
        table in strategy::table(),
        position in position(),
        index in index(),
    ) {
        let (before, after) = surrounding;
        let backing = [before.as_slice(), &bytes, &after].concat();
        let within = &backing[before.len()..before.len() + bytes.len()];
        let copy = at_byte_offset(&bytes, byte_offset);

        let reads = Reads::of(copy.bytes(), &table, position, index);
        prop_assert_eq!(&Reads::of(within, &table, position, index), &reads);

        for error in reads.decode_errors() {
            if let DecodeError::OutOfRange { position, byte_length, available } = *error {
                let end = position as u128 + u128::from(byte_length);
                prop_assert!(end > available as u128 && available <= bytes.len(), "{:?}", error);
            }
        }
    }
}

/// Bytes, mostly small, so that offsets, counts and lengths often lie
/// within them, and some 0xff
fn byte() -> impl Strategy<Value = u8> {
    prop_oneof![4 => Just(0), 4 => 1..16_u8, 1 => Just(0xff), 1 => any::<u8>()]
}

/// Positions within small bytes, and up to the largest
fn position() -> impl Strategy<Value = usize> {
    prop_oneof![8 => 0..80_usize, 1 => Just(usize::MAX), 1 => any::<usize>()]
}

/// Indexes of fields that a small table holds, and up to the largest
fn index() -> impl Strategy<Value = usize> {
    prop_oneof![8 => 0..12_usize, 1 => Just(usize::MAX), 1 => any::<usize>()]
}

/// Everything that a test reads from arbitrary bytes, to compare between
/// copies of them
#[derive(Debug, PartialEq)]
struct Reads {
    table: Result<Table, ReadError>,
    fields: Vec<Result<Field, ReadError>>,
    size: Result<usize, DecodeError>,
    version: Result<u32, DecodeError>,
    field_offset: Result<usize, DecodeError>,
}

impl Reads {
    /// Reads `bytes` as `table`, and reads each of its root fields at its
    /// own root offset, at root offset `index` and at `position`
    fn of(bytes: &[u8], table: &Table, position: usize, index: usize) -> Self {
        let layout = table.layout;
        let mut fields = Vec::with_capacity(3 * table.fields.len());
        for (root_index, field) in table.fields.iter().enumerate() {
            fields.push(read_root_field(bytes, layout, root_index, field));
            fields.push(read_root_field(bytes, layout, index, field));
            fields.push(read_field(bytes, layout, position, field));
        }

        let buffer = ByteBuffer::new(bytes, layout.unversioned());
        let versioned = ByteBuffer::new(bytes, layout.unversioned().versioned());

        Self {
            table: read_table(bytes, table),
            fields,
            size: buffer.size(),
            version: versioned.version(),
            field_offset: buffer.field_offset(position, index),
        }
    }

    fn decode_errors(&self) -> Vec<&DecodeError> {
        let reads = iter::once(self.table.as_ref().err())
            .chain(self.fields.iter().map(|field| field.as_ref().err()));
        let read_errors = reads.flatten().filter_map(|error| match error {
            ReadError::Decode(error) => Some(error),
            _ => None,
        });

        let errors = [
            self.size.as_ref().err(),
            self.version.as_ref().err(),
            self.field_offset.as_ref().err(),
        ];

        read_errors.chain(errors.into_iter().flatten()).collect()
    }
}

#[test]
fn test_return_error_outside_slice() {
    let backing = [0_u8; 32];
    let buffer = ByteBuffer::new(&backing[..8], TableLayout::DEFAULT);

    assert_eq!(
        buffer.get_f64(4),
        Err(DecodeError::OutOfRange {
            position: 4,
            byte_length: 8,
            available: 8
        })
    );
    assert_eq!(
        buffer.get_i64(usize::MAX).unwrap_err().to_string(),
        format!(
            "Read exceeds the supplied buffer: 8 bytes at {} of 8",
            usize::MAX
        )
    );
    assert!(buffer.get_u32(5).is_err());
    assert!(buffer.get_i8(8).is_err());
    assert_eq!(buffer.get_u32(4), Ok(0));

    assert!(buffer.root_offset(usize::MAX).is_err());
    assert!(buffer.field_offset(usize::MAX, usize::MAX).is_err());
    assert!(buffer.field_offset(4, 1).is_err());
}

/// A slice of 8 bytes holding `uint32s`, followed by bytes that a read past
/// its end would return
fn slice_of(uint32s: &[u32]) -> Vec<u8> {
    let mut backing = vec![65; 32];
    for (i, uint32) in uint32s.iter().enumerate() {
        backing[4 + i * 4..8 + i * 4].copy_from_slice(&uint32.to_le_bytes());
    }

    backing
}

#[test]
fn test_reject_array_string_and_vector_outside_slice() {
    let backing = slice_of(&[5]);
    let buffer = ByteBuffer::new(&backing[4..12], TableLayout::DEFAULT);
    let outside = DecodeError::OutOfRange {
        position: 4,
        byte_length: 5,
        available: 8,
    };

    assert_eq!(buffer.get_u8_array(0).unwrap_err(), outside);
    assert_eq!(buffer.get_str(0).unwrap_err(), outside);
    assert_eq!(
        buffer.get_f32_array(0).unwrap_err(),
        DecodeError::OutOfRange {
            position: 4,
            byte_length: 20,
            available: 8
        }
    );

    let backing = slice_of(&[1, 5]);
    let buffer = ByteBuffer::new(&backing[4..12], TableLayout::DEFAULT);
    assert!(buffer.get_vector(0).is_err());
}

#[test]
fn test_reject_array_whose_byte_length_overflows() {
    let bytes = [255, 255, 255, 255, 0, 0, 0, 0];
    let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);

    assert_eq!(
        buffer.get_f64_array(0).unwrap_err(),
        DecodeError::OutOfRange {
            position: 4,
            byte_length: 8 * 0xffff_ffff,
            available: 8
        }
    );
}

#[test]
fn test_reject_invalid_utf8() {
    let cases: [(&str, &[u8]); 4] = [
        ("a lone continuation byte", &[0x80]),
        ("a truncated sequence", &[0xe2, 0x82]),
        ("an overlong encoding", &[0xc0, 0xaf]),
        ("an encoded surrogate", &[0xed, 0xa0, 0x80]),
    ];

    for (name, utf8) in cases {
        let mut bytes = u32::try_from(utf8.len()).unwrap().to_le_bytes().to_vec();
        bytes.extend(utf8);

        let error = ByteBuffer::new(&bytes, TableLayout::DEFAULT)
            .get_str(0)
            .unwrap_err();
        assert!(
            matches!(error, DecodeError::InvalidUtf8 { position: 0, .. }),
            "{name}"
        );
        assert_eq!(error.to_string(), "The string at 0 isn't valid UTF-8");
        assert!(error.source().is_some(), "{name}");
    }
}

#[test]
fn test_describe_unsupported_version() {
    let layout = TableLayout::DEFAULT.sized().versioned();
    let mut builder = TableBuilder::versioned(1, layout, 2).unwrap();
    builder.set_u8(42).unwrap();
    let bytes = builder.to_bytes().unwrap();

    let version = ByteBuffer::new(&bytes, layout).version().unwrap();
    let error = DecodeError::UnsupportedVersion {
        version,
        supported: 1,
    };

    assert_eq!(
        error.to_string(),
        "Unsupported format version 2; the reader reads version 1"
    );
    assert!(error.source().is_none());
}
