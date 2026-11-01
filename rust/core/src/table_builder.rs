use std::borrow::Cow;

use crate::{Alignment, BuildError, Element, OffsetSize, TableLayout, element::extend_elements};

/// Builds a table, which a `ByteBuffer` reads back.
///
/// A table is a header with one offset per field, after the table's total
/// length and version when its layout has them, followed by the fields in
/// the order they're set. Arrays, strings and vectors start with their
/// count, and a vector then holds the end of each item. Arrays of scalars
/// and of structs are padded so their elements are aligned from the start of
/// the table. Fields are read back by index with `ByteBuffer::root_offset()`,
/// or `ByteBuffer::field_offset()` for a table nested in another.
///
/// Setting a field records it, borrowing or owning its value, and
/// [`TableBuilder::to_bytes()`] then allocates the table at its exact size
/// and writes every field in one pass. Tables nested from their builders are
/// written in the same pass, straight into this one.
///
/// A setter returns a [`BuildError`], and leaves the builder as it was, when
/// the table already holds all its fields or the field would make it longer
/// than the layout's offsets address: 255 bytes, 64 KiB or 4 GiB. Errors are
/// values rather than panics, since a WebAssembly build that aborts on panic
/// couldn't report them.
#[derive(Debug)]
pub struct TableBuilder<'a> {
    // Narrow fields, since the builder is inline in a field that nests it,
    // and every field is as large as the largest kind
    values: Vec<Field<'a>>,
    /// The number of fields that the table holds, which a header of 32-bit
    /// offsets limits to fewer than 2^30
    fields: u32,
    /// The table's length so far, which is where the next field goes
    length: u32,
    /// The version, when `versioned`
    version: u32,
    versioned: bool,
    sized: bool,
    offset_size: OffsetSize,
    /// The largest alignment that a field needs from the table's start,
    /// which a parent pads this table to when it nests it
    alignment: Alignment,
}

macro_rules! scalar_setters {
    ($($name:ident($type:ty)),*) => {$(
        #[doc = concat!("Sets a `", stringify!($type), "` field")]
        ///
        /// # Errors
        ///
        /// [`BuildError::TooManyFields`] if the table holds all its fields
        /// already, or [`BuildError::TooLong`] if the field would make it
        /// longer than its offsets address.
        #[inline]
        pub fn $name(&mut self, value: $type) -> Result<(), BuildError> {
            self.add_field(Field::scalar(value))
        }
    )*};
}

macro_rules! array_setters {
    ($($name:ident($type:ty) => $variant:ident),*) => {$(
        #[doc = concat!(
            "Sets an array field of `", stringify!($type), "`, from a slice that ",
            "the builder borrows or a `Vec` that it owns until the table is written"
        )]
        ///
        /// # Errors
        ///
        /// [`BuildError::TooManyFields`] if the table holds all its fields
        /// already, or [`BuildError::TooLong`] if the field would make it
        /// longer than its offsets address.
        #[inline]
        pub fn $name(&mut self, values: impl Into<Cow<'a, [$type]>>) -> Result<(), BuildError> {
            self.add_field(Field::Array(Array::$variant(values.into())))
        }
    )*};
}

impl<'a> TableBuilder<'a> {
    /// Starts a table of `fields` fields in `layout`, which its readers must
    /// use too. Nested tables use [`TableLayout::DEFAULT`].
    ///
    /// # Errors
    ///
    /// [`BuildError::HeaderTooLong`] if the header alone is longer than the
    /// layout's offsets address.
    pub fn new(fields: usize, layout: TableLayout) -> Result<Self, BuildError> {
        let header = layout.header_length(fields)?;

        Ok(Self::create(fields, header, layout, None))
    }

    /// Starts a table of `fields` fields in `layout`, with `version` in its
    /// header. Readers can check it with `ByteBuffer::version()` before
    /// reading any field.
    ///
    /// # Errors
    ///
    /// [`BuildError::VersionTooLarge`] if `version` doesn't fit in the
    /// layout's offset size, or [`BuildError::HeaderTooLong`] if the header
    /// alone is longer than the layout's offsets address.
    pub fn versioned(
        fields: usize,
        layout: TableLayout<true>,
        version: u32,
    ) -> Result<Self, BuildError> {
        layout.check_version(version)?;
        let header = layout.header_length(fields)?;

        Ok(Self::create(fields, header, layout, Some(version)))
    }

    /// A builder of a table of `fields` fields, whose header of `header`
    /// bytes its layout checked
    #[allow(clippy::cast_possible_truncation)]
    fn create<const VERSIONED: bool>(
        fields: usize,
        header: u32,
        layout: TableLayout<VERSIONED>,
        version: Option<u32>,
    ) -> Self {
        Self {
            values: Vec::with_capacity(fields),
            // The header holds an offset of at least a byte per field
            fields: fields as u32,
            length: header,
            version: version.unwrap_or(0),
            versioned: version.is_some(),
            sized: layout.is_sized(),
            offset_size: layout.offset_size(),
            alignment: Alignment::One,
        }
    }

    /// Writes the table, at its exact size.
    ///
    /// # Errors
    ///
    /// [`BuildError::Incomplete`] if fewer fields were set than the table
    /// holds.
    pub fn to_bytes(&self) -> Result<Vec<u8>, BuildError> {
        self.check_complete()?;

        let mut bytes = Vec::with_capacity(self.length as usize);
        self.write(&mut bytes);

        Ok(bytes)
    }

    scalar_setters!(
        set_i8(i8),
        set_u8(u8),
        set_i16(i16),
        set_u16(u16),
        set_i32(i32),
        set_u32(u32),
        set_i64(i64),
        set_u64(u64),
        set_f32(f32),
        set_f64(f64)
    );

    array_setters!(
        set_i8_array(i8) => I8, set_u8_array(u8) => U8,
        set_i16_array(i16) => I16, set_u16_array(u16) => U16,
        set_i32_array(i32) => I32, set_u32_array(u32) => U32,
        set_i64_array(i64) => I64, set_u64_array(u64) => U64,
        set_f32_array(f32) => F32, set_f64_array(f64) => F64
    );

    /// Sets a string field, from a `&str` that the builder borrows or a
    /// `String` that it owns until the table is written
    ///
    /// # Errors
    ///
    /// [`BuildError::TooManyFields`] if the table holds all its fields
    /// already, or [`BuildError::TooLong`] if the field would make it longer
    /// than its offsets address.
    pub fn set_str(&mut self, string: impl Into<Cow<'a, str>>) -> Result<(), BuildError> {
        self.add_field(Field::String(string.into()))
    }

    /// Sets a struct field from its bytes: its scalars, little-endian, one
    /// after another, which are written as they are, without padding. The
    /// builder borrows a slice or owns a `Vec` until the table is written.
    ///
    /// # Errors
    ///
    /// [`BuildError::TooManyFields`] if the table holds all its fields
    /// already, or [`BuildError::TooLong`] if the field would make it longer
    /// than its offsets address.
    pub fn set_struct(&mut self, bytes: impl Into<Cow<'a, [u8]>>) -> Result<(), BuildError> {
        self.add_field(Field::Struct(bytes.into()))
    }

    /// Sets an array of structs of `size` bytes each, from their bytes one
    /// after another, which it counts. It's padded so that the structs start
    /// at a multiple of `alignment` from the start of the table, which is
    /// the size of the structs' largest scalar.
    ///
    /// # Errors
    ///
    /// [`BuildError::NotWholeStructs`] if `size` is 0 or the bytes aren't a
    /// whole number of structs, or an error of any setter.
    pub fn set_struct_array(
        &mut self,
        bytes: impl Into<Cow<'a, [u8]>>,
        size: usize,
        alignment: Alignment,
    ) -> Result<(), BuildError> {
        let bytes = bytes.into();
        let length = bytes.len();
        if size == 0 || !length.is_multiple_of(size) {
            return Err(BuildError::NotWholeStructs { length, size });
        }

        self.add_field(Field::StructArray {
            bytes,
            count: length / size,
            alignment,
        })
    }

    /// Embeds a table, from its builder or its bytes, without a length prefix.
    /// Readers find its fields with `ByteBuffer::field_offset()` from the
    /// field's position, which reads offsets as wide as the parent's, so the
    /// table should have no header and the parent's offset size: that's
    /// [`TableLayout::DEFAULT`] in a parent of 32-bit offsets.
    ///
    /// A builder, borrowed or owned, is written straight into this table
    /// when it's written, without bytes of its own. It must have all its
    /// fields set.
    ///
    /// The nested table's arrays are aligned from its own start, so it's
    /// padded to start at a multiple of an alignment, which they need to be
    /// read in place. For a builder, that's the largest alignment of its
    /// arrays, of scalars and of structs, and of its nested tables, or 1 if
    /// it has neither. For bytes, it's 4, which suits tables of 8-, 16- and
    /// 32-bit elements; [`TableBuilder::set_table_aligned()`] takes another.
    ///
    /// # Errors
    ///
    /// [`BuildError::Incomplete`] if the builder has fields left to set, or
    /// an error of any setter.
    pub fn set_table(&mut self, table: impl Into<NestedTable<'a>>) -> Result<(), BuildError> {
        let table = table.into();
        let alignment = table.default_alignment();

        self.add_table(table, alignment)
    }

    /// Embeds a table like [`TableBuilder::set_table()`], padded to start at
    /// a multiple of `alignment`, such as [`Alignment::Eight`] for a table
    /// of 64-bit arrays given as bytes
    ///
    /// # Errors
    ///
    /// The errors of [`TableBuilder::set_table()`].
    pub fn set_table_aligned(
        &mut self,
        table: impl Into<NestedTable<'a>>,
        alignment: Alignment,
    ) -> Result<(), BuildError> {
        self.add_table(table.into(), alignment)
    }

    /// Writes a vector of tables, from their builders or their bytes, after
    /// the end of each, which lets readers find any item without reading the
    /// others. Builders must have all their fields set, and are written
    /// straight into this table as `set_table()` writes them, each in its own
    /// layout. Items aren't padded, so their arrays are read in place only
    /// when they happen to be aligned.
    ///
    /// # Errors
    ///
    /// [`BuildError::Incomplete`] if a builder has fields left to set, or an
    /// error of any setter.
    pub fn set_vector<I>(&mut self, items: I) -> Result<(), BuildError>
    where
        I: IntoIterator,
        I::Item: Into<NestedTable<'a>>,
    {
        let items: Vec<_> = items.into_iter().map(Into::into).collect();
        for item in &items {
            item.check_complete()?;
        }

        self.add_field(Field::Vector(items))
    }

    fn add_table(
        &mut self,
        table: NestedTable<'a>,
        alignment: Alignment,
    ) -> Result<(), BuildError> {
        table.check_complete()?;
        self.add_field(Field::Table { table, alignment })
    }

    /// Records a field after the padding that it needs. Checking the table's
    /// length here, rather than when writing it, reports a field too long for
    /// the layout from its own setter and leaves the builder as it was. Every
    /// offset, count, length and item end is at most the table's length, so
    /// that's the one value to check.
    fn add_field(&mut self, field: Field<'a>) -> Result<(), BuildError> {
        let index = self.values.len();
        if index == self.fields as usize {
            return Err(BuildError::TooManyFields {
                fields: self.fields as usize,
            });
        }

        let width = self.offset_size.bytes();
        let padding = field.padding(self.length as usize, width);
        let length = u64::from(self.length) + padding as u64 + field.byte_length(width);

        self.length = u32::try_from(length)
            .ok()
            .filter(|&length| length <= self.offset_size.max_value())
            .ok_or(BuildError::TooLong {
                index,
                length,
                offset_size: self.offset_size,
            })?;
        self.alignment = self.alignment.max(field.alignment());
        self.values.push(field);

        Ok(())
    }

    const fn check_complete(&self) -> Result<(), BuildError> {
        if self.values.len() == self.fields as usize {
            Ok(())
        } else {
            Err(BuildError::Incomplete {
                set: self.values.len(),
                fields: self.fields as usize,
            })
        }
    }

    /// Appends the table to `bytes`, with its offsets counted from its own
    /// start.
    ///
    /// Appending to reserved capacity rather than writing into a slice saves
    /// zeroing the slice first: writing 27,028 floats took 2.0 µs rather than
    /// 3.1 µs, and a table of 1.2 MB 19 µs rather than 29 µs.
    fn write(&self, bytes: &mut Vec<u8>) {
        let start = bytes.len();
        let size = self.offset_size;
        let width = size.bytes();

        if self.sized {
            push_slot(bytes, size, self.length as usize);
        }

        if self.versioned {
            push_slot(bytes, size, self.version as usize);
        }

        // Each field's offset is set once its position is known
        let mut slot = bytes.len();
        bytes.resize(slot + self.values.len() * width, 0);

        for field in &self.values {
            let padding = field.padding(bytes.len() - start, width);
            if padding > 0 {
                bytes.resize(bytes.len() + padding, 0);
            }

            let position = bytes.len() - start;
            set_slot(bytes, slot, size, position);
            slot += width;

            field.write(bytes, size);
        }

        debug_assert_eq!(bytes.len() - start, self.length as usize);
    }
}

/// A table to nest with [`TableBuilder::set_table()`] or in
/// [`TableBuilder::set_vector()`]: a builder, borrowed or owned, which is
/// written straight into its parent, or bytes, which are copied
#[derive(Debug)]
pub struct NestedTable<'a>(Nested<'a>);

#[derive(Debug)]
enum Nested<'a> {
    Builder(&'a TableBuilder<'a>),
    OwnedBuilder(TableBuilder<'a>),
    Bytes(Cow<'a, [u8]>),
}

impl<'a> NestedTable<'a> {
    const fn builder(&self) -> Option<&TableBuilder<'a>> {
        match &self.0 {
            Nested::Builder(builder) => Some(builder),
            Nested::OwnedBuilder(builder) => Some(builder),
            Nested::Bytes(_) => None,
        }
    }

    fn byte_length(&self) -> usize {
        match &self.0 {
            Nested::Builder(builder) => builder.length as usize,
            Nested::OwnedBuilder(builder) => builder.length as usize,
            Nested::Bytes(bytes) => bytes.len(),
        }
    }

    fn default_alignment(&self) -> Alignment {
        self.builder()
            .map_or(Alignment::Four, |builder| builder.alignment)
    }

    fn check_complete(&self) -> Result<(), BuildError> {
        self.builder().map_or(Ok(()), TableBuilder::check_complete)
    }

    fn write(&self, bytes: &mut Vec<u8>) {
        match &self.0 {
            Nested::Builder(builder) => builder.write(bytes),
            Nested::OwnedBuilder(builder) => builder.write(bytes),
            Nested::Bytes(table) => bytes.extend_from_slice(table),
        }
    }
}

impl<'a> From<TableBuilder<'a>> for NestedTable<'a> {
    fn from(builder: TableBuilder<'a>) -> Self {
        Self(Nested::OwnedBuilder(builder))
    }
}

impl<'b, 'a: 'b> From<&'b TableBuilder<'a>> for NestedTable<'b> {
    fn from(builder: &'b TableBuilder<'a>) -> Self {
        Self(Nested::Builder(builder))
    }
}

impl<'a> From<Cow<'a, [u8]>> for NestedTable<'a> {
    fn from(bytes: Cow<'a, [u8]>) -> Self {
        Self(Nested::Bytes(bytes))
    }
}

impl<'a> From<&'a [u8]> for NestedTable<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self::from(Cow::Borrowed(bytes))
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for NestedTable<'a> {
    fn from(bytes: &'a [u8; N]) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl<'a> From<&'a Vec<u8>> for NestedTable<'a> {
    fn from(bytes: &'a Vec<u8>) -> Self {
        Self::from(bytes.as_slice())
    }
}

impl From<Vec<u8>> for NestedTable<'_> {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from(Cow::Owned(bytes))
    }
}

#[derive(Debug)]
enum Field<'a> {
    /// The first `size` of `bytes` are the scalar's, little-endian
    Scalar {
        bytes: [u8; 8],
        size: usize,
    },
    Array(Array<'a>),
    String(Cow<'a, str>),
    /// A struct's bytes, written as they are
    Struct(Cow<'a, [u8]>),
    /// The bytes of `count` structs, which start at a multiple of
    /// `alignment` from the table's start
    StructArray {
        bytes: Cow<'a, [u8]>,
        count: usize,
        alignment: Alignment,
    },
    Table {
        table: NestedTable<'a>,
        alignment: Alignment,
    },
    Vector(Vec<NestedTable<'a>>),
}

impl Field<'_> {
    #[inline]
    fn scalar<T: Element>(value: T) -> Self {
        let size = size_of::<T>();
        let mut bytes = [0; 8];
        bytes[..size].copy_from_slice(value.to_le().as_ref());

        Self::Scalar { bytes, size }
    }

    /// The alignment that the field needs from the table's start
    fn alignment(&self) -> Alignment {
        match self {
            Self::Array(array) => array.alignment(),
            Self::StructArray { alignment, .. } | Self::Table { alignment, .. } => *alignment,
            Self::Scalar { .. } | Self::String(_) | Self::Struct(_) | Self::Vector(_) => {
                Alignment::One
            }
        }
    }

    /// The padding before the field when it would start at `offset`, which
    /// aligns an array's elements after its count and a nested table's start
    fn padding(&self, offset: usize, width: usize) -> usize {
        match self {
            // Wrapping, since only the remainder matters
            Self::Array(array) => array.alignment().padding(offset.wrapping_add(width)),
            Self::StructArray { alignment, .. } => alignment.padding(offset.wrapping_add(width)),
            Self::Table { alignment, .. } => alignment.padding(offset),
            Self::Scalar { .. } | Self::String(_) | Self::Struct(_) | Self::Vector(_) => 0,
        }
    }

    /// The field's byte length after its padding, in u64, which only a
    /// vector that nests a builder many times can overflow, saturating far
    /// past the longest table
    fn byte_length(&self, width: usize) -> u64 {
        let width = width as u64;

        match self {
            Self::Scalar { size, .. } => *size as u64,
            Self::Array(array) => width + array.byte_length() as u64,
            Self::String(string) => width + string.len() as u64,
            Self::Struct(bytes) => bytes.len() as u64,
            Self::StructArray { bytes, .. } => width + bytes.len() as u64,
            Self::Table { table, .. } => table.byte_length() as u64,
            Self::Vector(items) => items.iter().fold(width, |length, item| {
                length.saturating_add(width + item.byte_length() as u64)
            }),
        }
    }

    /// Appends the field, with lengths as wide as `offset_size`
    fn write(&self, bytes: &mut Vec<u8>, offset_size: OffsetSize) {
        match self {
            Self::Scalar {
                bytes: scalar,
                size,
            } => match size {
                1 => bytes.push(scalar[0]),
                2 => bytes.extend_from_slice(&scalar[..2]),
                4 => bytes.extend_from_slice(&scalar[..4]),
                _ => bytes.extend_from_slice(scalar),
            },
            Self::Array(array) => {
                push_slot(bytes, offset_size, array.len());
                array.write(bytes);
            }
            Self::String(string) => {
                push_slot(bytes, offset_size, string.len());
                bytes.extend_from_slice(string.as_bytes());
            }
            Self::Struct(scalars) => bytes.extend_from_slice(scalars),
            Self::StructArray {
                bytes: structs,
                count,
                ..
            } => {
                push_slot(bytes, offset_size, *count);
                bytes.extend_from_slice(structs);
            }
            Self::Table { table, .. } => table.write(bytes),
            Self::Vector(items) => {
                push_slot(bytes, offset_size, items.len());

                // Each end counts from the vector's position, which is its
                // count's
                let mut end = (items.len() + 1) * offset_size.bytes();
                for item in items {
                    end += item.byte_length();
                    push_slot(bytes, offset_size, end);
                }

                for item in items {
                    item.write(bytes);
                }
            }
        }
    }
}

#[derive(Debug)]
enum Array<'a> {
    I8(Cow<'a, [i8]>),
    U8(Cow<'a, [u8]>),
    I16(Cow<'a, [i16]>),
    U16(Cow<'a, [u16]>),
    I32(Cow<'a, [i32]>),
    U32(Cow<'a, [u32]>),
    I64(Cow<'a, [i64]>),
    U64(Cow<'a, [u64]>),
    F32(Cow<'a, [f32]>),
    F64(Cow<'a, [f64]>),
}

/// Evaluates `$expression` with `$values` bound to the elements of `$array`,
/// whatever their type
macro_rules! with_elements {
    ($array:expr, $values:ident => $expression:expr) => {
        match $array {
            Array::I8($values) => $expression,
            Array::U8($values) => $expression,
            Array::I16($values) => $expression,
            Array::U16($values) => $expression,
            Array::I32($values) => $expression,
            Array::U32($values) => $expression,
            Array::I64($values) => $expression,
            Array::U64($values) => $expression,
            Array::F32($values) => $expression,
            Array::F64($values) => $expression,
        }
    };
}

impl Array<'_> {
    fn alignment(&self) -> Alignment {
        with_elements!(self, values => alignment_of(values))
    }

    fn len(&self) -> usize {
        with_elements!(self, values => values.len())
    }

    fn byte_length(&self) -> usize {
        with_elements!(self, values => size_of_val::<[_]>(values))
    }

    fn write(&self, bytes: &mut Vec<u8>) {
        with_elements!(self, values => extend_elements(values, bytes));
    }
}

const fn alignment_of<T: Element>(_: &[T]) -> Alignment {
    T::ALIGNMENT
}

// The slots, like the scalars, are copied at constant lengths, which are
// single stores rather than calls to copy a slice. The builder checks that
// the table's length fits in its offsets, and every value is at most that
// long.

/// Appends a length, offset or header slot as wide as `offset_size`
#[allow(clippy::cast_possible_truncation)]
fn push_slot(bytes: &mut Vec<u8>, offset_size: OffsetSize, value: usize) {
    debug_assert!(value <= offset_size.max_value() as usize);

    match offset_size {
        OffsetSize::U8 => bytes.push(value as u8),
        OffsetSize::U16 => bytes.extend_from_slice(&(value as u16).to_le_bytes()),
        OffsetSize::U32 => bytes.extend_from_slice(&(value as u32).to_le_bytes()),
    }
}

/// Sets the slot at `position`, which `bytes` must already hold
#[allow(clippy::cast_possible_truncation)]
fn set_slot(bytes: &mut [u8], position: usize, offset_size: OffsetSize, value: usize) {
    debug_assert!(value <= offset_size.max_value() as usize);

    match offset_size {
        OffsetSize::U8 => bytes[position] = value as u8,
        OffsetSize::U16 => {
            bytes[position..position + 2].copy_from_slice(&(value as u16).to_le_bytes());
        }
        OffsetSize::U32 => {
            bytes[position..position + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
    }
}
