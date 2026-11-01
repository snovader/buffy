//! Reads tables with `ByteBuffer` as their descriptions set them out, and
//! returns the descriptions with the values read in place of theirs, so
//! that they equal the descriptions when the bytes hold the described
//! values.
//!
//! A table nested with `set_table()` is read in place when it has no header
//! and offsets as wide as its parent's, and otherwise as a table of its own,
//! from its position to the end of its parent, since its length isn't
//! written. Vector items are read as tables of their own, from the bytes
//! that `Vector` returns; an item without a header has the same offsets
//! either way.

use tafla::{ByteBuffer, DecodeError};

use super::description::{Field, Layout, Nested, Table};

/// Why bytes didn't read as a description
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// A read returned the runtime's error
    Decode(DecodeError),
    /// A sized table's header holds `size`, rather than its byte `length`
    Size { size: usize, length: usize },
    /// A vector holds `count` items, and iterates over `iterated`, rather
    /// than the `described` number
    Items {
        count: usize,
        iterated: usize,
        described: usize,
    },
}

impl From<DecodeError> for ReadError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

/// Evaluates `$expression` with `$reader` reading `$bytes` in `$layout`,
/// whose `ByteBuffer` type depends on whether the layout is versioned
macro_rules! with_reader {
    ($bytes:expr, $layout:expr, $reader:ident => $expression:expr) => {{
        let layout = $layout.unversioned();
        if $layout.versioned {
            let $reader = Reader(ByteBuffer::new($bytes, layout.versioned()));
            $expression
        } else {
            let $reader = Reader(ByteBuffer::new($bytes, layout));
            $expression
        }
    }};
}

/// Reads a table from its own bytes, as the root or a vector item is read:
/// its header, whose size must be the table's byte length when it's sized,
/// and then its fields
pub fn read_table(bytes: &[u8], table: &Table) -> Result<Table, ReadError> {
    let layout = table.layout.unversioned();

    let (version, fields) = if table.layout.versioned {
        let reader = Reader(ByteBuffer::new(bytes, layout.versioned()));
        reader.check_size(table.layout)?;
        (
            Some(reader.0.version()?),
            reader.read_root_fields(&table.fields)?,
        )
    } else {
        let reader = Reader(ByteBuffer::new(bytes, layout));
        reader.check_size(table.layout)?;
        (None, reader.read_root_fields(&table.fields)?)
    };

    Ok(Table {
        layout: table.layout,
        version,
        fields,
    })
}

/// Reads root field `index` of a table of `layout` as `field`, without
/// reading the header
pub fn read_root_field(
    bytes: &[u8],
    layout: Layout,
    index: usize,
    field: &Field,
) -> Result<Field, ReadError> {
    with_reader!(bytes, layout, reader => reader.read_root_field(index, field))
}

/// Reads `field` at `position` of a table of `layout`
pub fn read_field(
    bytes: &[u8],
    layout: Layout,
    position: usize,
    field: &Field,
) -> Result<Field, ReadError> {
    with_reader!(bytes, layout, reader => reader.read_field(position, field))
}

struct Reader<'a, const V: bool>(ByteBuffer<'a, V>);

impl<const V: bool> Reader<'_, V> {
    fn check_size(&self, layout: Layout) -> Result<(), ReadError> {
        let length = self.0.bytes().len();
        if layout.sized {
            let size = self.0.size()?;
            if size != length {
                return Err(ReadError::Size { size, length });
            }
        }

        Ok(())
    }

    fn read_root_fields(&self, fields: &[Field]) -> Result<Vec<Field>, ReadError> {
        let fields = fields.iter().enumerate();
        fields
            .map(|(index, field)| self.read_root_field(index, field))
            .collect()
    }

    fn read_root_field(&self, index: usize, field: &Field) -> Result<Field, ReadError> {
        self.read_field(self.0.root_offset(index)?, field)
    }

    fn read_field(&self, position: usize, field: &Field) -> Result<Field, ReadError> {
        let buffer = &self.0;

        Ok(match field {
            Field::I8(_) => Field::I8(buffer.get_i8(position)?),
            Field::U8(_) => Field::U8(buffer.get_u8(position)?),
            Field::I16(_) => Field::I16(buffer.get_i16(position)?),
            Field::U16(_) => Field::U16(buffer.get_u16(position)?),
            Field::I32(_) => Field::I32(buffer.get_i32(position)?),
            Field::U32(_) => Field::U32(buffer.get_u32(position)?),
            Field::I64(_) => Field::I64(buffer.get_i64(position)?),
            Field::U64(_) => Field::U64(buffer.get_u64(position)?),
            Field::F32(_) => Field::F32(buffer.get_f32(position)?),
            Field::F64(_) => Field::F64(buffer.get_f64(position)?),
            Field::I8Array(_) => Field::I8Array(buffer.get_i8_array(position)?.to_vec()),
            Field::U8Array(_) => Field::U8Array(buffer.get_u8_array(position)?.to_vec()),
            Field::I16Array(_) => Field::I16Array(buffer.get_i16_array(position)?.to_vec()),
            Field::U16Array(_) => Field::U16Array(buffer.get_u16_array(position)?.to_vec()),
            Field::I32Array(_) => Field::I32Array(buffer.get_i32_array(position)?.to_vec()),
            Field::U32Array(_) => Field::U32Array(buffer.get_u32_array(position)?.to_vec()),
            Field::I64Array(_) => Field::I64Array(buffer.get_i64_array(position)?.to_vec()),
            Field::U64Array(_) => Field::U64Array(buffer.get_u64_array(position)?.to_vec()),
            Field::F32Array(_) => Field::F32Array(buffer.get_f32_array(position)?.to_vec()),
            Field::F64Array(_) => Field::F64Array(buffer.get_f64_array(position)?.to_vec()),
            Field::String(_) => Field::String(buffer.get_str(position)?.to_owned()),
            Field::Table { nested, alignment } => Field::Table {
                nested: Nested {
                    from: nested.from,
                    table: self.read_nested(position, &nested.table)?,
                },
                alignment: *alignment,
            },
            Field::Vector(items) => Field::Vector(self.read_vector(position, items)?),
        })
    }

    /// Reads a table nested with `set_table()`
    fn read_nested(&self, position: usize, table: &Table) -> Result<Table, ReadError> {
        let layout = &table.layout;
        let in_place = !layout.sized
            && !layout.versioned
            && layout.offset_size == self.0.layout().offset_size();

        if !in_place {
            // A position past the end leaves no bytes, which the first read
            // fails for
            let bytes = self.0.bytes();
            return read_table(&bytes[position.min(bytes.len())..], table);
        }

        let fields = table.fields.iter().enumerate();
        let fields = fields
            .map(|(index, field)| self.read_field(self.0.field_offset(position, index)?, field))
            .collect::<Result<_, _>>()?;

        Ok(Table {
            layout: table.layout,
            version: None,
            fields,
        })
    }

    fn read_vector(&self, position: usize, items: &[Nested]) -> Result<Vec<Nested>, ReadError> {
        let vector = self.0.get_vector(position)?;
        let bytes: Vec<&[u8]> = vector.iter().collect();

        if vector.len() != items.len() || bytes.len() != items.len() {
            return Err(ReadError::Items {
                count: vector.len(),
                iterated: bytes.len(),
                described: items.len(),
            });
        }

        let read_item = |(bytes, item): (&[u8], &Nested)| {
            Ok(Nested {
                from: item.from,
                table: read_table(bytes, &item.table)?,
            })
        };

        bytes.into_iter().zip(items).map(read_item).collect()
    }
}
