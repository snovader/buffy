//! Tables as the conformance suite's `cases.json` describes them, which
//! `conformance.rs` parses and `strategy` generates. See
//! `conformance/README.md` at the root of the repository.

use tafla::{Alignment, OffsetSize, TableLayout};

/// A table: its layout, its version and its fields, in the order that
/// they're set
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub layout: Layout,
    /// The version in the header, present only when the layout is
    /// versioned
    pub version: Option<u32>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub sized: bool,
    pub versioned: bool,
    pub offset_size: OffsetSize,
}

/// A field of each type that a table holds, with its value. Equal fields
/// hold floats of the same bits, so that NaN equals itself and -0 doesn't
/// equal 0.
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
    /// A table nested with `set_table()`, or with `set_table_aligned()`
    /// when the description gives an alignment
    Table {
        nested: Nested,
        alignment: Option<Alignment>,
    },
    Vector(Vec<Nested>),
}

/// A table nested from its builder or from the bytes that its builder
/// writes
#[derive(Clone, Debug, PartialEq)]
pub struct Nested {
    pub from: Source,
    pub table: Table,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Builder,
    Bytes,
}

impl Layout {
    /// A layout without header slots, which tables nested with
    /// `set_table()` have
    pub const fn headerless(offset_size: OffsetSize) -> Self {
        Self {
            sized: false,
            versioned: false,
            offset_size,
        }
    }

    /// The layout without its version slot, which `versioned()` adds
    pub const fn unversioned(self) -> TableLayout {
        let layout = TableLayout::DEFAULT.with_offset_size(self.offset_size);
        if self.sized { layout.sized() } else { layout }
    }

    /// The byte length of the header slots before the field offsets
    pub const fn header_size(self) -> usize {
        (self.sized as usize + self.versioned as usize) * self.offset_size.bytes()
    }
}

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
            (
                Self::Table {
                    nested: a,
                    alignment: a_alignment,
                },
                Self::Table {
                    nested: b,
                    alignment: b_alignment,
                },
            ) => a == b && a_alignment == b_alignment,
            (Self::Vector(a), Self::Vector(b)) => a == b,
            _ => false,
        }
    }
}
