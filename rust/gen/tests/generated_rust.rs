//! Compiles the Rust generated from the example schemas, so that clippy and
//! rustfmt check it with this crate's lints and formatting.

#[allow(dead_code)]
#[path = "golden/shapes/rust/shapes.rs"]
mod shapes;

#[allow(dead_code)]
#[path = "golden/layouts/rust/layouts.rs"]
mod layouts;

#[allow(dead_code)]
#[path = "golden/all_types/rust/all_types.rs"]
mod all_types;
