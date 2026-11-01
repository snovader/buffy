mod common;

use std::error::Error;

use buffy_core::{ByteBuffer, DecodeError, OffsetSize, TableBuilder, TableLayout};
use common::{Field, KINDS, random_fields, runs, table_of};
use rand::{RngExt, SeedableRng, rngs::StdRng};

#[test]
fn test_read_truncated_table_or_return_error() {
    let mut rng = StdRng::seed_from_u64(1);

    for _ in 0..runs(200) {
        let fields = random_fields(&mut rng, 8);
        let layout = if rng.random() {
            TableLayout::DEFAULT.sized()
        } else {
            TableLayout::DEFAULT
        };
        let bytes = table_of(&fields, layout);

        for length in 0..bytes.len() {
            // A copy, so that no read can reach the bytes after the cut
            let truncated = bytes[..length].to_vec();
            let buffer = ByteBuffer::new(&truncated, layout);

            for (index, field) in fields.iter().enumerate() {
                match buffer
                    .root_offset(index)
                    .and_then(|position| field.read(&buffer, position))
                {
                    Ok(read) => assert_eq!(&read, field),
                    Err(DecodeError::OutOfRange { .. }) => {}
                    Err(error) => panic!("{error}"),
                }
            }
        }
    }
}

#[test]
fn test_read_any_bytes_without_panicking() {
    let mut rng = StdRng::seed_from_u64(2);
    let layouts = [
        TableLayout::DEFAULT,
        TableLayout::DEFAULT.sized(),
        TableLayout::DEFAULT.with_offset_size(OffsetSize::U8),
        TableLayout::DEFAULT
            .sized()
            .with_offset_size(OffsetSize::U16),
    ];

    for _ in 0..runs(20_000) {
        let len = rng.random_range(0..64);
        // Mostly small bytes, so that lengths and offsets are often in range
        let bytes: Vec<u8> = (0..len)
            .map(|_| {
                if rng.random_ratio(3, 4) {
                    rng.random_range(0..16)
                } else {
                    rng.random()
                }
            })
            .collect();
        let layout = layouts[rng.random_range(0..layouts.len())];
        let buffer = ByteBuffer::new(&bytes, layout);
        let kind = rng.random_range(0..KINDS);
        let field = Field::random_of_kind(&mut rng, kind);
        let index = rng.random_range(0..16);

        let _ = buffer.size();
        let _ = buffer
            .root_offset(index)
            .and_then(|position| field.read(&buffer, position));
        let _ = buffer
            .root_offset(0)
            .and_then(|position| buffer.field_offset(position, index))
            .and_then(|position| field.read(&buffer, position));
        let _ = field.read(&buffer, index);
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
fn test_reject_impossible_vector_count_before_walking_it() {
    let bytes = [255, 255, 255, 255];
    let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);

    assert_eq!(
        buffer.get_vector(0).unwrap_err(),
        DecodeError::OutOfRange {
            position: 4,
            byte_length: 4 * 0xffff_ffff,
            available: 4
        }
    );
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
fn test_decode_strings_relative_to_slice() {
    let bytes = [99, 99, 2, 0, 0, 0, 111, 107, 99];
    let buffer = ByteBuffer::new(&bytes[2..8], TableLayout::DEFAULT);

    assert_eq!(buffer.get_str(0), Ok("ok"));
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

#[test]
fn test_round_trip_strings_with_leading_feff() {
    let mut rng = StdRng::seed_from_u64(3);

    for _ in 0..runs(500) {
        let len = rng.random_range(0..20);
        let string: String = (0..len).map(|_| rng.random::<char>()).collect();

        for value in [string.clone(), format!("\u{feff}{string}")] {
            let mut builder = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
            builder.set_str(value.as_str()).unwrap();

            let bytes = builder.to_bytes().unwrap();
            let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);
            assert_eq!(
                buffer.get_str(buffer.root_offset(0).unwrap()),
                Ok(value.as_str())
            );
        }
    }
}
