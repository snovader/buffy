use std::{iter::FusedIterator, mem};

use crate::{DecodeError, OffsetSize, byte_buffer::read_slot};

/// A vector read from a table: the bytes of its items, which are tables.
///
/// The vector holds its count, the end of each item, counted from the
/// vector's position, and then the items. `ByteBuffer::get_vector()` checks
/// that the ends lie within the bytes, and each item is checked when it's
/// read, so reading one can fail. [`Vector::get()`] finds any item from its
/// end and the one before it, without reading the others. Items are read
/// with their own `ByteBuffer`.
#[derive(Clone, Copy, Debug)]
pub struct Vector<'a> {
    /// The bytes from the vector's position, which is its count's, to the
    /// end of the buffer
    bytes: &'a [u8],
    /// The vector's position in the buffer, which errors report positions
    /// from
    position: usize,
    len: usize,
    offset_size: OffsetSize,
}

impl<'a> Vector<'a> {
    /// Views `len` items in `bytes`, from the vector's `position` to the end
    /// of the buffer, which must have been checked to hold the count and
    /// the ends
    pub(crate) const fn new(
        bytes: &'a [u8],
        position: usize,
        len: usize,
        offset_size: OffsetSize,
    ) -> Self {
        Self {
            bytes,
            position,
            len,
            offset_size,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the bytes of item `index`, or `None` past the end. Only the
    /// ends of the item and the one before it are read.
    ///
    /// # Errors
    ///
    /// `Some(Err(_))` with [`DecodeError::ItemEndsBeforeStart`] if the item
    /// ends before the item before it, or [`DecodeError::OutOfRange`] if it
    /// ends past the bytes.
    pub fn get(&self, index: usize) -> Option<Result<&'a [u8], DecodeError>> {
        if index >= self.len {
            return None;
        }

        let start = match index {
            0 => Ok(self.items_start()),
            _ => self.end(index - 1),
        };

        Some(start.and_then(|start| self.item(start, self.end(index)?)))
    }

    /// Iterates over the items' bytes, reading each item's end once
    pub const fn iter(&self) -> Items<'a> {
        Items {
            vector: *self,
            index: 0,
            start: self.items_start(),
        }
    }

    /// Where item 0 starts, from the vector's position: after the count and
    /// the ends
    const fn items_start(&self) -> usize {
        // The count and the ends lie within the bytes, so this doesn't
        // overflow
        (self.len + 1) * self.offset_size.bytes()
    }

    /// Reads the end of item `index`, from the vector's position
    fn end(&self, index: usize) -> Result<usize, DecodeError> {
        let width = self.offset_size.bytes();
        let slot = (index + 1) * width;

        self.bytes
            .get(slot..)
            .and_then(|bytes| read_slot(bytes, self.offset_size))
            .map(|end| end as usize)
            .ok_or_else(|| {
                DecodeError::out_of_range(self.position_of(slot), width as u64, self.available())
            })
    }

    /// Returns the bytes of the item from `start` to `end`, from the
    /// vector's position
    fn item(&self, start: usize, end: usize) -> Result<&'a [u8], DecodeError> {
        if start > end {
            return Err(DecodeError::ItemEndsBeforeStart {
                start: self.position_of(start),
                end: self.position_of(end),
            });
        }

        self.bytes.get(start..end).ok_or_else(|| {
            let byte_length = (end - start) as u64;
            DecodeError::out_of_range(self.position_of(start), byte_length, self.available())
        })
    }

    /// The position in the buffer of `offset` from the vector's position,
    /// which an error reports. Saturating, since only a 32-bit host can
    /// overflow, at a position past any bytes.
    const fn position_of(&self, offset: usize) -> usize {
        self.position.saturating_add(offset)
    }

    /// The length of the buffer that the vector lies in
    const fn available(&self) -> usize {
        self.position + self.bytes.len()
    }
}

impl<'a> IntoIterator for Vector<'a> {
    type Item = Result<&'a [u8], DecodeError>;
    type IntoIter = Items<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &Vector<'a> {
    type Item = Result<&'a [u8], DecodeError>;
    type IntoIter = Items<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over the bytes of a [`Vector`]'s items, which returns each
/// item as [`Vector::get()`] does: an item that fails doesn't keep the
/// others from being read
#[derive(Clone, Debug)]
pub struct Items<'a> {
    vector: Vector<'a>,
    index: usize,
    /// Where the next item starts, from the vector's position, which is
    /// where the item before it ends
    start: usize,
}

impl<'a> Iterator for Items<'a> {
    type Item = Result<&'a [u8], DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.vector.len {
            return None;
        }

        let item = self.vector.end(self.index).and_then(|end| {
            let start = mem::replace(&mut self.start, end);
            self.vector.item(start, end)
        });
        self.index += 1;

        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vector.len - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Items<'_> {}

impl FusedIterator for Items<'_> {}
