//! Fields of every kind with random values, like the `serializable` arbitrary
//! of typescript/core/test/serializable.ts, for tests that build tables and
//! read them back.

#![allow(dead_code)]

use buffy_core::{BuildError, ByteBuffer, DecodeError, TableBuilder, TableLayout};
use rand::{RngExt, rngs::StdRng};

#[derive(Clone, Debug)]
pub enum Field {
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    I8Array(Vec<i8>),
    U8Array(Vec<u8>),
    I16Array(Vec<i16>),
    U16Array(Vec<u16>),
    I32Array(Vec<i32>),
    U32Array(Vec<u32>),
    I64Array(Vec<i64>),
    U64Array(Vec<u64>),
    F32Array(Vec<f32>),
    F64Array(Vec<f64>),
    String(String),
    Vector(Vec<Vec<u8>>),
}

/// The number of kinds of field, for `Field::random_of_kind()`
pub const KINDS: usize = 22;

impl Field {
    pub fn random(rng: &mut StdRng) -> Self {
        let kind = rng.random_range(0..KINDS);
        Self::random_of_kind(rng, kind)
    }

    /// A field of each kind in turn for `kind` from 0 to `KINDS`. Floats come
    /// from random bits, which include NaN with any payload, infinities and
    /// -0.
    pub fn random_of_kind(rng: &mut StdRng, kind: usize) -> Self {
        let len = rng.random_range(0..20);

        match kind {
            0 => Self::I8(rng.random()),
            1 => Self::U8(rng.random()),
            2 => Self::I16(rng.random()),
            3 => Self::U16(rng.random()),
            4 => Self::I32(rng.random()),
            5 => Self::U32(rng.random()),
            6 => Self::I64(rng.random()),
            7 => Self::U64(rng.random()),
            8 => Self::F32(f32::from_bits(rng.random())),
            9 => Self::F64(f64::from_bits(rng.random())),
            10 => Self::I8Array(random_vec(rng, len)),
            11 => Self::U8Array(random_vec(rng, len)),
            12 => Self::I16Array(random_vec(rng, len)),
            13 => Self::U16Array(random_vec(rng, len)),
            14 => Self::I32Array(random_vec(rng, len)),
            15 => Self::U32Array(random_vec(rng, len)),
            16 => Self::I64Array(random_vec(rng, len)),
            17 => Self::U64Array(random_vec(rng, len)),
            18 => Self::F32Array((0..len).map(|_| f32::from_bits(rng.random())).collect()),
            19 => Self::F64Array((0..len).map(|_| f64::from_bits(rng.random())).collect()),
            20 => Self::String(random_string(rng, len)),
            _ => {
                let items = rng.random_range(0..4);
                Self::Vector((0..items).map(|_| random_vec(rng, len)).collect())
            }
        }
    }

    /// Sets the field on `builder`, which borrows its value
    pub fn set<'a>(&'a self, builder: &mut TableBuilder<'a>) -> Result<(), BuildError> {
        match self {
            Self::I8(value) => builder.set_i8(*value),
            Self::U8(value) => builder.set_u8(*value),
            Self::I16(value) => builder.set_i16(*value),
            Self::U16(value) => builder.set_u16(*value),
            Self::I32(value) => builder.set_i32(*value),
            Self::U32(value) => builder.set_u32(*value),
            Self::I64(value) => builder.set_i64(*value),
            Self::U64(value) => builder.set_u64(*value),
            Self::F32(value) => builder.set_f32(*value),
            Self::F64(value) => builder.set_f64(*value),
            Self::I8Array(values) => builder.set_i8_array(values),
            Self::U8Array(values) => builder.set_u8_array(values),
            Self::I16Array(values) => builder.set_i16_array(values),
            Self::U16Array(values) => builder.set_u16_array(values),
            Self::I32Array(values) => builder.set_i32_array(values),
            Self::U32Array(values) => builder.set_u32_array(values),
            Self::I64Array(values) => builder.set_i64_array(values),
            Self::U64Array(values) => builder.set_u64_array(values),
            Self::F32Array(values) => builder.set_f32_array(values),
            Self::F64Array(values) => builder.set_f64_array(values),
            Self::String(string) => builder.set_str(string),
            Self::Vector(items) => builder.set_vector(items),
        }
    }

    /// Reads a field of this one's kind at `position`
    pub fn read<const V: bool>(
        &self,
        buffer: &ByteBuffer<'_, V>,
        position: usize,
    ) -> Result<Self, DecodeError> {
        Ok(match self {
            Self::I8(_) => Self::I8(buffer.get_i8(position)?),
            Self::U8(_) => Self::U8(buffer.get_u8(position)?),
            Self::I16(_) => Self::I16(buffer.get_i16(position)?),
            Self::U16(_) => Self::U16(buffer.get_u16(position)?),
            Self::I32(_) => Self::I32(buffer.get_i32(position)?),
            Self::U32(_) => Self::U32(buffer.get_u32(position)?),
            Self::I64(_) => Self::I64(buffer.get_i64(position)?),
            Self::U64(_) => Self::U64(buffer.get_u64(position)?),
            Self::F32(_) => Self::F32(buffer.get_f32(position)?),
            Self::F64(_) => Self::F64(buffer.get_f64(position)?),
            Self::I8Array(_) => Self::I8Array(buffer.get_i8_array(position)?.to_vec()),
            Self::U8Array(_) => Self::U8Array(buffer.get_u8_array(position)?.to_vec()),
            Self::I16Array(_) => Self::I16Array(buffer.get_i16_array(position)?.to_vec()),
            Self::U16Array(_) => Self::U16Array(buffer.get_u16_array(position)?.to_vec()),
            Self::I32Array(_) => Self::I32Array(buffer.get_i32_array(position)?.to_vec()),
            Self::U32Array(_) => Self::U32Array(buffer.get_u32_array(position)?.to_vec()),
            Self::I64Array(_) => Self::I64Array(buffer.get_i64_array(position)?.to_vec()),
            Self::U64Array(_) => Self::U64Array(buffer.get_u64_array(position)?.to_vec()),
            Self::F32Array(_) => Self::F32Array(buffer.get_f32_array(position)?.to_vec()),
            Self::F64Array(_) => Self::F64Array(buffer.get_f64_array(position)?.to_vec()),
            Self::String(_) => Self::String(buffer.get_str(position)?.to_owned()),
            Self::Vector(_) => Self::Vector(
                buffer
                    .get_vector(position)?
                    .iter()
                    .map(<[u8]>::to_vec)
                    .collect(),
            ),
        })
    }

    /// The largest element size of the field's array, or 1 for any other
    /// field
    pub const fn alignment(&self) -> usize {
        match self {
            Self::I16Array(_) | Self::U16Array(_) => 2,
            Self::I32Array(_) | Self::U32Array(_) | Self::F32Array(_) => 4,
            Self::I64Array(_) | Self::U64Array(_) | Self::F64Array(_) => 8,
            _ => 1,
        }
    }
}

/// Compares floats by their bits, so that NaN equals itself and -0 doesn't
/// equal 0
impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::F32(a), Self::F32(b)) => a.to_bits() == b.to_bits(),
            (Self::F64(a), Self::F64(b)) => a.to_bits() == b.to_bits(),
            (Self::F32Array(a), Self::F32Array(b)) => a
                .iter()
                .map(|value| value.to_bits())
                .eq(b.iter().map(|value| value.to_bits())),
            (Self::F64Array(a), Self::F64Array(b)) => a
                .iter()
                .map(|value| value.to_bits())
                .eq(b.iter().map(|value| value.to_bits())),
            (Self::I8(a), Self::I8(b)) => a == b,
            (Self::U8(a), Self::U8(b)) => a == b,
            (Self::I16(a), Self::I16(b)) => a == b,
            (Self::U16(a), Self::U16(b)) => a == b,
            (Self::I32(a), Self::I32(b)) => a == b,
            (Self::U32(a), Self::U32(b)) => a == b,
            (Self::I64(a), Self::I64(b)) => a == b,
            (Self::U64(a), Self::U64(b)) => a == b,
            (Self::I8Array(a), Self::I8Array(b)) => a == b,
            (Self::U8Array(a), Self::U8Array(b)) => a == b,
            (Self::I16Array(a), Self::I16Array(b)) => a == b,
            (Self::U16Array(a), Self::U16Array(b)) => a == b,
            (Self::I32Array(a), Self::I32Array(b)) => a == b,
            (Self::U32Array(a), Self::U32Array(b)) => a == b,
            (Self::I64Array(a), Self::I64Array(b)) => a == b,
            (Self::U64Array(a), Self::U64Array(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Vector(a), Self::Vector(b)) => a == b,
            _ => false,
        }
    }
}

/// The number of random cases to run, which is fewer under Miri, which
/// interprets the tests to check for undefined behavior
pub const fn runs(cases: usize) -> usize {
    if cfg!(miri) {
        cases.div_ceil(50)
    } else {
        cases
    }
}

pub fn random_fields(rng: &mut StdRng, max: usize) -> Vec<Field> {
    let len = rng.random_range(0..=max);
    (0..len).map(|_| Field::random(rng)).collect()
}

/// A builder with `fields` set and `more` fields left to set
pub fn builder_of(fields: &[Field], more: usize, layout: TableLayout) -> TableBuilder<'_> {
    let mut builder = TableBuilder::new(fields.len() + more, layout).unwrap();
    for field in fields {
        field.set(&mut builder).unwrap();
    }

    builder
}

/// The bytes of a table of `fields`
pub fn table_of(fields: &[Field], layout: TableLayout) -> Vec<u8> {
    builder_of(fields, 0, layout).to_bytes().unwrap()
}

/// Asserts that each of `fields` reads back from the root table
pub fn assert_fields<const V: bool>(buffer: &ByteBuffer<'_, V>, fields: &[Field]) {
    for (index, field) in fields.iter().enumerate() {
        let position = buffer.root_offset(index).unwrap();
        assert_eq!(
            &field.read(buffer, position).unwrap(),
            field,
            "field {index}"
        );
    }
}

/// Returns a copy of `bytes` that starts `byte_offset` bytes past a multiple
/// of 8 in a larger buffer of 0xff. Its arrays are aligned in memory when
/// `byte_offset` is a multiple of 8.
pub fn at_byte_offset(bytes: &[u8], byte_offset: usize) -> AtByteOffset {
    let mut backing = vec![0xff; 8 + byte_offset + bytes.len() + 8];
    let start = backing.as_ptr().align_offset(8) + byte_offset;
    backing[start..start + bytes.len()].copy_from_slice(bytes);

    AtByteOffset {
        backing,
        start,
        len: bytes.len(),
    }
}

pub struct AtByteOffset {
    backing: Vec<u8>,
    start: usize,
    len: usize,
}

impl AtByteOffset {
    pub fn bytes(&self) -> &[u8] {
        &self.backing[self.start..self.start + self.len]
    }
}

fn random_vec<T>(rng: &mut StdRng, len: usize) -> Vec<T>
where
    rand::distr::StandardUniform: rand::distr::Distribution<T>,
{
    (0..len).map(|_| rng.random()).collect()
}

/// A string of ASCII, multi-byte characters and U+FEFF, which isn't a byte
/// order mark within a table
fn random_string(rng: &mut StdRng, len: usize) -> String {
    (0..len)
        .map(|_| match rng.random_range(0..4) {
            0 => '\u{feff}',
            1 => rng.random(),
            _ => rng.random_range('a'..='z'),
        })
        .collect()
}
