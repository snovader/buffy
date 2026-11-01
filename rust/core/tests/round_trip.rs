mod common;

use std::borrow::Cow;

use buffy_core::{Alignment, ByteBuffer, OffsetSize, TableBuilder, TableLayout};
use common::{Field, assert_fields, at_byte_offset, builder_of, random_fields, runs, table_of};
use rand::{RngExt, SeedableRng, rngs::StdRng};

const RUNS: usize = runs(300);

#[test]
fn test_round_trip_in_each_layout() {
    let mut rng = StdRng::seed_from_u64(1);
    let layouts = [
        TableLayout::DEFAULT,
        TableLayout::DEFAULT.sized(),
        TableLayout::DEFAULT.with_offset_size(OffsetSize::U16),
        TableLayout::DEFAULT
            .sized()
            .with_offset_size(OffsetSize::U16),
    ];

    for _ in 0..RUNS {
        let fields = random_fields(&mut rng, 12);

        for layout in layouts {
            let bytes = table_of(&fields, layout);
            let buffer = ByteBuffer::new(&bytes, layout);

            assert_eq!(buffer.size().unwrap(), bytes.len());
            assert_fields(&buffer, &fields);
        }
    }
}

#[test]
fn test_round_trip_versioned() {
    let mut rng = StdRng::seed_from_u64(2);

    for _ in 0..RUNS {
        let fields = random_fields(&mut rng, 8);
        let version = rng.random();
        let layout = if rng.random() {
            TableLayout::DEFAULT.sized().versioned()
        } else {
            TableLayout::DEFAULT.versioned()
        };

        let mut builder = TableBuilder::versioned(fields.len(), layout, version).unwrap();
        for field in &fields {
            field.set(&mut builder).unwrap();
        }

        let bytes = builder.to_bytes().unwrap();
        let buffer = ByteBuffer::new(&bytes, layout);

        assert_eq!(buffer.version().unwrap(), version);
        assert_fields(&buffer, &fields);
    }
}

#[test]
fn test_round_trip_owned_values() {
    let mut builder = TableBuilder::new(3, TableLayout::DEFAULT).unwrap();
    builder.set_f64_array(vec![1.5, -2.5]).unwrap();
    builder.set_str(String::from("owned")).unwrap();
    builder.set_vector(vec![vec![1_u8, 2], vec![3]]).unwrap();

    let bytes = builder.to_bytes().unwrap();
    let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);

    let floats = buffer
        .get_f64_array(buffer.root_offset(0).unwrap())
        .unwrap();
    assert_eq!(floats.to_vec(), [1.5, -2.5]);
    assert_eq!(
        buffer.get_str(buffer.root_offset(1).unwrap()).unwrap(),
        "owned"
    );

    let vector = buffer.get_vector(buffer.root_offset(2).unwrap()).unwrap();
    assert_eq!(vector.iter().collect::<Vec<_>>(), [&[1, 2][..], &[3]]);
}

#[test]
fn test_read_at_any_byte_offset() {
    let mut rng = StdRng::seed_from_u64(3);

    for _ in 0..RUNS {
        let fields = random_fields(&mut rng, 8);
        let layout = TableLayout::DEFAULT.sized();
        let bytes = table_of(&fields, layout);

        for byte_offset in 0..16 {
            let copy = at_byte_offset(&bytes, byte_offset);
            assert_fields(&ByteBuffer::new(copy.bytes(), layout), &fields);
        }
    }
}

#[test]
fn test_arrays_in_place_when_aligned() {
    let values = [1.5, -0.0, f64::NAN];
    let mut builder = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    builder.set_u8_array(&[1, 2, 3]).unwrap();
    builder.set_f64_array(&values).unwrap();
    let bytes = builder.to_bytes().unwrap();

    for byte_offset in 0..8 {
        let copy = at_byte_offset(&bytes, byte_offset);
        let buffer = ByteBuffer::new(copy.bytes(), TableLayout::DEFAULT);

        let uint8s = buffer.get_u8_array(buffer.root_offset(0).unwrap()).unwrap();
        assert!(copy.bytes().as_ptr_range().contains(&uint8s.as_ptr()));

        let float64s = buffer
            .get_f64_array(buffer.root_offset(1).unwrap())
            .unwrap();
        let in_place = cfg!(target_endian = "little") && byte_offset == 0;

        assert_eq!(float64s.as_slice().is_some(), in_place);
        assert_eq!(matches!(float64s.to_cow(), Cow::Borrowed(_)), in_place);
        assert_eq!(bits_of(float64s.to_cow().iter().copied()), bits_of(values));
        assert_eq!(bits_of(float64s.iter()), bits_of(values));
        assert_eq!(
            bits_of(float64s.iter().rev()),
            bits_of(values.into_iter().rev())
        );
        assert_eq!(
            float64s.get(1).map(f64::to_bits),
            Some((-0.0_f64).to_bits())
        );
        assert_eq!(float64s.get(3), None);
    }
}

// The same values and byte offsets as "should read -0 and NaN from
// little-endian float arrays" in typescript/core/test/byteBuffer.test.ts
#[test]
fn test_read_floats_bitwise_at_byte_offset() {
    let float64s = [-0.0, f64::NAN, 0.0, f64::NEG_INFINITY, -1.5];
    // Converted once, since a conversion may give NaN either sign
    let float32s = [-0.0, f32::NAN, 0.0, f32::NEG_INFINITY, -1.5];

    for byte_offset in [0, 1, 4] {
        let mut bytes32 = 5_u32.to_le_bytes().to_vec();
        let mut bytes64 = bytes32.clone();
        bytes32.extend(float32s.iter().flat_map(|value| value.to_le_bytes()));
        bytes64.extend(float64s.iter().flat_map(|value| value.to_le_bytes()));

        let bytes32 = at_byte_offset(&bytes32, byte_offset);
        let bytes64 = at_byte_offset(&bytes64, byte_offset);
        let read32 = ByteBuffer::new(bytes32.bytes(), TableLayout::DEFAULT)
            .get_f32_array(0)
            .unwrap();
        let read64 = ByteBuffer::new(bytes64.bytes(), TableLayout::DEFAULT)
            .get_f64_array(0)
            .unwrap();

        assert_eq!(
            read32.iter().map(f32::to_bits).collect::<Vec<_>>(),
            float32s.map(f32::to_bits)
        );
        assert_eq!(bits_of(read64.iter()), bits_of(float64s));
    }
}

#[test]
fn test_16_bit_sizes_little_endian() {
    let layout = TableLayout::DEFAULT
        .sized()
        .with_offset_size(OffsetSize::U16);
    let mut builder = TableBuilder::new(1, layout).unwrap();
    builder.set_str("a".repeat(300)).unwrap();

    // Table size (306), offset of the string (4) and string length (300)
    let bytes = builder.to_bytes().unwrap();
    assert_eq!(bytes[..6], [0x32, 0x01, 0x04, 0x00, 0x2c, 0x01]);
}

#[test]
fn test_nested_builder_writes_bytes_at_its_alignment() {
    let mut rng = StdRng::seed_from_u64(4);

    for _ in 0..RUNS {
        let parent = random_fields(&mut rng, 6);
        let nested = random_fields(&mut rng, 6);
        let alignment = match nested.iter().map(Field::alignment).max() {
            Some(8) => Alignment::Eight,
            Some(4) => Alignment::Four,
            Some(2) => Alignment::Two,
            _ => Alignment::One,
        };

        let mut with_builder = builder_of(&parent, 1, TableLayout::DEFAULT);
        with_builder
            .set_table(builder_of(&nested, 0, TableLayout::DEFAULT))
            .unwrap();

        let nested_bytes = table_of(&nested, TableLayout::DEFAULT);
        let mut with_bytes = builder_of(&parent, 1, TableLayout::DEFAULT);
        with_bytes
            .set_table_aligned(&nested_bytes, alignment)
            .unwrap();

        assert_eq!(
            with_builder.to_bytes().unwrap(),
            with_bytes.to_bytes().unwrap()
        );
    }
}

#[test]
fn test_tables_nested_two_deep_in_place() {
    let mut rng = StdRng::seed_from_u64(5);

    for _ in 0..RUNS {
        let outer_fields = random_fields(&mut rng, 6);
        let middle_fields = random_fields(&mut rng, 6);
        let inner_fields = random_fields(&mut rng, 6);

        let mut middle = builder_of(&middle_fields, 1, TableLayout::DEFAULT);
        middle
            .set_table(builder_of(&inner_fields, 0, TableLayout::DEFAULT))
            .unwrap();

        let layout = TableLayout::DEFAULT.sized();
        let mut outer = builder_of(&outer_fields, 1, layout);
        outer.set_table(middle).unwrap();

        let bytes = at_byte_offset(&outer.to_bytes().unwrap(), 0);
        let buffer = ByteBuffer::new(bytes.bytes(), layout);
        let middle_position = buffer.root_offset(outer_fields.len()).unwrap();
        let inner_position = buffer
            .field_offset(middle_position, middle_fields.len())
            .unwrap();

        for (index, field) in inner_fields.iter().enumerate() {
            let position = buffer.field_offset(inner_position, index).unwrap();
            assert_eq!(&field.read(&buffer, position).unwrap(), field);

            if let Field::F64Array(_) = field {
                let read = buffer.get_f64_array(position).unwrap();
                assert_eq!(read.as_slice().is_some(), cfg!(target_endian = "little"));
            }
        }
    }
}

#[test]
fn test_vector_of_builders_writes_their_bytes() {
    let mut rng = StdRng::seed_from_u64(6);

    for _ in 0..RUNS {
        let len = rng.random_range(0..5);
        let items: Vec<_> = (0..len).map(|_| random_fields(&mut rng, 6)).collect();
        let bytes: Vec<_> = items
            .iter()
            .map(|item| table_of(item, TableLayout::DEFAULT))
            .collect();

        let mut with_builders = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
        with_builders
            .set_vector(
                items
                    .iter()
                    .map(|item| builder_of(item, 0, TableLayout::DEFAULT)),
            )
            .unwrap();

        let mut mixed = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
        mixed
            .set_vector(
                items
                    .iter()
                    .zip(&bytes)
                    .enumerate()
                    .map(|(i, (item, bytes))| {
                        if i % 2 == 0 {
                            builder_of(item, 0, TableLayout::DEFAULT).into()
                        } else {
                            buffy_core::NestedTable::from(bytes)
                        }
                    }),
            )
            .unwrap();

        let mut with_bytes = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
        with_bytes.set_vector(&bytes).unwrap();

        let expected = with_bytes.to_bytes().unwrap();
        assert_eq!(with_builders.to_bytes().unwrap(), expected);
        assert_eq!(mixed.to_bytes().unwrap(), expected);

        let buffer = ByteBuffer::new(&expected, TableLayout::DEFAULT);
        let vector = buffer.get_vector(buffer.root_offset(0).unwrap()).unwrap();
        assert_eq!(vector.len(), items.len());
        assert_eq!(vector.iter().len(), items.len());

        for (read, fields) in vector.iter().zip(&items) {
            assert_fields(&ByteBuffer::new(read, TableLayout::DEFAULT), fields);
        }
    }
}

#[test]
fn test_builder_without_arrays_padded_by_nothing() {
    let mut nested = TableBuilder::new(1, TableLayout::DEFAULT).unwrap();
    nested.set_f64(1.5).unwrap();

    let mut parent = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    parent.set_u8(1).unwrap();
    parent.set_table(nested).unwrap();

    let bytes = parent.to_bytes().unwrap();
    let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);
    assert_eq!(buffer.root_offset(1).unwrap(), 2 * 4 + 1);
}

#[test]
fn test_set_table_aligned_starts_at_multiple() {
    let mut rng = StdRng::seed_from_u64(7);
    let alignments = [
        Alignment::One,
        Alignment::Two,
        Alignment::Four,
        Alignment::Eight,
    ];

    for _ in 0..RUNS {
        let len = rng.random_range(0..10);
        let string: String = (0..len).map(|_| 'a').collect();
        let alignment = alignments[rng.random_range(0..alignments.len())];

        let mut builder = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
        builder.set_str(string.as_str()).unwrap();
        builder.set_table_aligned(&[1, 2, 3], alignment).unwrap();

        let bytes = builder.to_bytes().unwrap();
        let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);
        let position = buffer.root_offset(1).unwrap();

        assert_eq!(position % alignment.bytes(), 0);
        assert_eq!(bytes[position..], [1, 2, 3]);
    }
}

#[test]
fn test_builder_nested_several_times() {
    let mut nested = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    nested.set_f64_array(&[1.0, 2.0]).unwrap();
    nested.set_str("nested").unwrap();
    let own = nested.to_bytes().unwrap();

    let mut borrowed = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    borrowed.set_table(&nested).unwrap();
    borrowed.set_vector([&nested, &nested]).unwrap();

    let mut copied = TableBuilder::new(2, TableLayout::DEFAULT).unwrap();
    copied.set_table_aligned(&own, Alignment::Eight).unwrap();
    copied.set_vector([&own, &own]).unwrap();

    let bytes = borrowed.to_bytes().unwrap();
    assert_eq!(bytes, copied.to_bytes().unwrap());

    // The builder is unchanged by nesting
    assert_eq!(nested.to_bytes().unwrap(), own);

    let buffer = ByteBuffer::new(&bytes, TableLayout::DEFAULT);
    let vector = buffer.get_vector(buffer.root_offset(1).unwrap()).unwrap();
    assert_eq!(vector.iter().collect::<Vec<_>>(), [&own[..], &own]);
}

#[test]
fn test_vector_item_of_another_offset_size() {
    let cases = [
        (OffsetSize::U8, TableLayout::DEFAULT),
        (
            OffsetSize::U32,
            TableLayout::DEFAULT.with_offset_size(OffsetSize::U16),
        ),
    ];

    for (offset_size, parent_layout) in cases {
        let layout = TableLayout::DEFAULT.with_offset_size(offset_size);
        let mut nested = TableBuilder::new(2, layout).unwrap();
        nested.set_str("another width").unwrap();
        nested.set_u16_array(&[1, 2]).unwrap();

        let mut parent = TableBuilder::new(1, parent_layout).unwrap();
        parent.set_vector([&nested]).unwrap();

        let bytes = parent.to_bytes().unwrap();
        let buffer = ByteBuffer::new(&bytes, parent_layout);
        let vector = buffer.get_vector(buffer.root_offset(0).unwrap()).unwrap();
        let item = vector.iter().next().unwrap();
        assert_eq!(item, nested.to_bytes().unwrap());

        let read = ByteBuffer::new(item, layout);
        assert_eq!(
            read.get_str(read.root_offset(0).unwrap()).unwrap(),
            "another width"
        );
    }
}

fn bits_of(values: impl IntoIterator<Item = f64>) -> Vec<u64> {
    values.into_iter().map(f64::to_bits).collect()
}
