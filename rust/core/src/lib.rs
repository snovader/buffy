#![doc = include_str!("../README.md")]

mod array_view;
mod byte_buffer;
mod element;
mod errors;
mod layout;
mod table_builder;
mod vector;

pub use array_view::{ArrayView, Iter};
pub use byte_buffer::ByteBuffer;
pub use element::Element;
pub use errors::{BuildError, DecodeError};
pub use layout::{Alignment, OffsetSize, TableLayout};
pub use table_builder::{NestedTable, TableBuilder};
pub use vector::{Items, Vector};
