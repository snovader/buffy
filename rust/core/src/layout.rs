use crate::BuildError;

/// A table's offset size: the width in bytes of its offset type, the unsigned
/// integer type of its header slots, field offsets, counts, lengths and item
/// ends, which limits how long the table can be.
///
/// Each variant is named after its offset type, as `SPEC.md` and schemas
/// name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OffsetSize {
    /// One byte, of `uint8` offsets, for tables of up to 255 bytes
    Uint8,
    /// Two bytes, of `uint16` offsets, for tables of up to 65,535 bytes
    Uint16,
    /// Four bytes, of `uint32` offsets, for tables of up to 4 GiB
    Uint32,
}

impl OffsetSize {
    /// The offset size in bytes: 1, 2 or 4
    pub const fn bytes(self) -> usize {
        match self {
            Self::Uint8 => 1,
            Self::Uint16 => 2,
            Self::Uint32 => 4,
        }
    }

    /// The largest value of the offset type, which is also the longest table
    /// that its offsets address
    pub const fn max_value(self) -> u32 {
        match self {
            Self::Uint8 => 0xff,
            Self::Uint16 => 0xffff,
            Self::Uint32 => 0xffff_ffff,
        }
    }
}

/// A multiple of bytes from a table's start that a field starts at, so that
/// its elements can be read in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Alignment {
    One = 1,
    Two = 2,
    Four = 4,
    Eight = 8,
}

impl Alignment {
    /// The alignment in bytes: 1, 2, 4 or 8
    pub const fn bytes(self) -> usize {
        self as usize
    }

    /// The bytes of padding that bring `offset` to a multiple of the
    /// alignment
    pub(crate) const fn padding(self, offset: usize) -> usize {
        offset.wrapping_neg() % self.bytes()
    }
}

/// The header and offset size of a table.
///
/// A format creates its layout once and passes it to both the `TableBuilder`
/// that writes the table and the `ByteBuffer` that reads it, so that the two
/// can't disagree. Nested tables use [`TableLayout::DEFAULT`].
///
/// `VERSIONED` says whether the header has a slot for a version, so that the
/// compiler requires a version exactly where a versioned table is built, and
/// offers `ByteBuffer::version()` only where there is one.
///
/// ```
/// use tafla::{OffsetSize, TableLayout};
///
/// const LAYOUT: TableLayout<true> = TableLayout::DEFAULT.sized().versioned();
/// const FRAME: TableLayout = TableLayout::DEFAULT.with_offset_size(OffsetSize::Uint8);
///
/// assert_eq!(LAYOUT.header_size(), 8);
/// assert_eq!(FRAME.header_size(), 0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableLayout<const VERSIONED: bool = false> {
    sized: bool,
    offset_size: OffsetSize,
}

impl TableLayout {
    /// No header slots, and offsets and lengths of the offset type `uint32`
    pub const DEFAULT: Self = Self {
        sized: false,
        offset_size: OffsetSize::Uint32,
    };

    /// Adds a slot for a version to the header, after the length when
    /// sized. Readers can check it before reading any field.
    pub const fn versioned(self) -> TableLayout<true> {
        TableLayout {
            sized: self.sized,
            offset_size: self.offset_size,
        }
    }
}

impl<const VERSIONED: bool> TableLayout<VERSIONED> {
    /// Starts the header with the table's total byte length
    #[must_use]
    pub const fn sized(self) -> Self {
        Self {
            sized: true,
            ..self
        }
    }

    /// Sets the offset size, the width of the header slots, field offsets,
    /// counts, lengths and item ends, which limits how long the table can be.
    /// The default is [`OffsetSize::Uint32`].
    #[must_use]
    pub const fn with_offset_size(self, offset_size: OffsetSize) -> Self {
        Self {
            offset_size,
            ..self
        }
    }

    pub const fn is_sized(self) -> bool {
        self.sized
    }

    pub const fn is_versioned(self) -> bool {
        VERSIONED
    }

    pub const fn offset_size(self) -> OffsetSize {
        self.offset_size
    }

    /// The byte length of the header slots before the field offsets
    pub const fn header_size(self) -> usize {
        let slots = if self.sized { 1 } else { 0 } + if VERSIONED { 1 } else { 0 };
        slots * self.offset_size.bytes()
    }

    /// The byte length of the header of a table of `fields` fields: its
    /// slots and an offset per field, which `TableBuilder` and `TableSizer`
    /// start a table with
    ///
    /// # Errors
    ///
    /// [`BuildError::HeaderTooLong`] if the header is longer than the
    /// layout's offsets address.
    pub(crate) fn header_length(self, fields: usize) -> Result<u32, BuildError> {
        // In u64, since a header longer than 4 GiB would saturate to the
        // limit itself in a 32-bit usize
        let slots = (fields as u64)
            .saturating_add(u64::from(self.sized))
            .saturating_add(u64::from(VERSIONED));
        let header = slots.saturating_mul(self.offset_size.bytes() as u64);

        u32::try_from(header)
            .ok()
            .filter(|&length| length <= self.offset_size.max_value())
            .ok_or(BuildError::HeaderTooLong {
                fields,
                offset_size: self.offset_size,
            })
    }
}

impl TableLayout<true> {
    /// Checks that `version` fits in the header's slot
    ///
    /// # Errors
    ///
    /// [`BuildError::VersionTooLarge`] if it doesn't fit in the layout's
    /// offset size.
    pub(crate) const fn check_version(self, version: u32) -> Result<(), BuildError> {
        if version > self.offset_size.max_value() {
            return Err(BuildError::VersionTooLarge {
                version,
                offset_size: self.offset_size,
            });
        }

        Ok(())
    }
}

impl Default for TableLayout {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout() {
        for layout in [TableLayout::DEFAULT, TableLayout::default()] {
            assert!(!layout.is_sized());
            assert!(!layout.is_versioned());
            assert_eq!(layout.offset_size(), OffsetSize::Uint32);
            assert_eq!(layout.header_size(), 0);
        }
    }

    // The same cases as typescript/core/test/layout.test.ts
    #[test]
    fn test_header_size() {
        let layout = TableLayout::DEFAULT;

        assert_eq!(layout.with_offset_size(OffsetSize::Uint8).header_size(), 0);
        assert_eq!(
            layout
                .sized()
                .with_offset_size(OffsetSize::Uint8)
                .header_size(),
            1
        );
        assert_eq!(
            layout
                .versioned()
                .with_offset_size(OffsetSize::Uint16)
                .header_size(),
            2
        );
        assert_eq!(
            layout
                .sized()
                .versioned()
                .with_offset_size(OffsetSize::Uint16)
                .header_size(),
            4
        );
        assert_eq!(layout.sized().versioned().header_size(), 8);
    }
}
