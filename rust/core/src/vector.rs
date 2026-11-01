use std::iter::FusedIterator;

use crate::{OffsetSize, byte_buffer::read_slot};

/// A vector read from a table: its items' bytes, each after its length
/// prefix. `ByteBuffer::get_vector()` checks every item before it returns
/// one, so iterating doesn't fail.
///
/// Items are tables, which are read with their own `ByteBuffer`. Finding an
/// item walks the items before it, so iterate rather than index.
#[derive(Clone, Copy, Debug)]
pub struct Vector<'a> {
    /// The items, each after its length prefix
    bytes: &'a [u8],
    len: usize,
    offset_size: OffsetSize,
}

impl<'a> Vector<'a> {
    /// Views `len` items in `bytes`, which must have been checked to hold
    /// them exactly
    pub(crate) const fn new(bytes: &'a [u8], len: usize, offset_size: OffsetSize) -> Self {
        Self {
            bytes,
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

    pub const fn iter(&self) -> Items<'a> {
        Items {
            bytes: self.bytes,
            remaining: self.len,
            offset_size: self.offset_size,
        }
    }
}

impl<'a> IntoIterator for Vector<'a> {
    type Item = &'a [u8];
    type IntoIter = Items<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &Vector<'a> {
    type Item = &'a [u8];
    type IntoIter = Items<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator over the bytes of a [`Vector`]'s items
#[derive(Clone, Debug)]
pub struct Items<'a> {
    bytes: &'a [u8],
    remaining: usize,
    offset_size: OffsetSize,
}

impl<'a> Iterator for Items<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.remaining == 0 {
            return None;
        }

        // The items were checked, so none of these returns early
        let length = read_slot(self.bytes, self.offset_size)? as usize;
        let items = self.bytes.get(self.offset_size.bytes()..)?;
        let (item, rest) = items.split_at_checked(length)?;

        self.bytes = rest;
        self.remaining -= 1;

        Some(item)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for Items<'_> {}

impl FusedIterator for Items<'_> {}
