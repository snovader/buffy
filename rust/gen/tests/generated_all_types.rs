//! The code generated from `schemas/all_types.buffy`, which has a field of
//! every type, writes the tables that `TableBuilder` writes by hand, reads
//! back what it writes, and reads corrupted and arbitrary bytes as values or
//! decode errors without panicking. Its structs are written and read at
//! their fixed sizes.

#[path = "golden/all_types/rust/all_types.rs"]
mod all_types;
mod tables;

use std::ops::Range;

use all_types::{
    Arrays, ArraysFields, Documented, Inner, InnerFields, Keywords, KeywordsFields, Nested,
    NestedFields, Outer, OuterFields, Packed, Reserved, ReservedFields, Scalars, ScalarsFields,
    Tree, TreeFields,
};
use buffy_core::{DecodeError, TableLayout};
use proptest::array::uniform5;
use proptest::collection::vec;
use proptest::prelude::*;
use tables::{
    CASES, Fields, Header, MAX_LENGTH, Value, arbitrary_bytes, arrays, build, check_truncated,
    expected, float32s, float64s, integers, overwrites, placements, strings, table, to_vec,
    truncations, vector,
};

/// The header of every message, none of which has attributes
const MESSAGE: Header = Header::new(TableLayout::DEFAULT);

// Reading every field with the generated getters, including those after one
// that fails

fn read_scalars(scalars: &Scalars) -> Fields {
    vec![
        scalars.int8().map(Value::I8),
        scalars.uint8().map(Value::U8),
        scalars.int16().map(Value::I16),
        scalars.uint16().map(Value::U16),
        scalars.int32().map(Value::I32),
        scalars.uint32().map(Value::U32),
        scalars.int64().map(Value::I64),
        scalars.uint64().map(Value::U64),
        scalars.float32().map(Value::f32),
        scalars.float64().map(Value::f64),
    ]
}

fn read_arrays(arrays: &Arrays) -> Fields {
    vec![
        arrays.int8().map(to_vec).map(Value::I8Array),
        arrays.uint8().map(<[u8]>::to_vec).map(Value::U8Array),
        arrays.int16().map(to_vec).map(Value::I16Array),
        arrays.uint16().map(to_vec).map(Value::U16Array),
        arrays.int32().map(to_vec).map(Value::I32Array),
        arrays.uint32().map(to_vec).map(Value::U32Array),
        arrays.int64().map(to_vec).map(Value::I64Array),
        arrays.uint64().map(to_vec).map(Value::U64Array),
        arrays.float32().map(Value::f32_array),
        arrays.float64().map(Value::f64_array),
    ]
}

fn read_nested(nested: &Nested) -> Fields {
    vec![
        nested.label().map(Value::string),
        nested.outer().and_then(|outer| table(read_outer(&outer))),
        nested
            .trees()
            .and_then(|trees| vector(trees.iter().map(read_tree))),
    ]
}

fn read_outer(outer: &Outer) -> Fields {
    vec![
        outer.count().map(Value::U32),
        outer.inner().and_then(|inner| table(read_inner(&inner))),
    ]
}

fn read_inner(inner: &Inner) -> Fields {
    vec![
        inner.name().map(Value::string),
        inner.values().map(Value::f64_array),
    ]
}

fn read_tree(tree: &Tree) -> Fields {
    vec![
        tree.value().map(Value::I32),
        tree.children()
            .and_then(|children| vector(children.iter().map(read_tree))),
    ]
}

fn read_reserved(reserved: &Reserved) -> Fields {
    vec![
        reserved
            .keywords()
            .and_then(|keywords| table(read_keywords(&keywords))),
    ]
}

fn read_keywords(keywords: &Keywords) -> Fields {
    vec![
        keywords.class().map(Value::U8),
        keywords.default().map(Value::U8),
        keywords.r#in().map(Value::U8),
        keywords.r#match().map(Value::U8),
        keywords.self_().map(Value::U8),
        keywords.r#type().map(Value::string),
    ]
}

// The values of the fields, of their types in the schema

fn describe_scalars(fields: &ScalarsFields) -> Vec<Value> {
    vec![
        Value::I8(fields.int8),
        Value::U8(fields.uint8),
        Value::I16(fields.int16),
        Value::U16(fields.uint16),
        Value::I32(fields.int32),
        Value::U32(fields.uint32),
        Value::I64(fields.int64),
        Value::U64(fields.uint64),
        Value::f32(fields.float32),
        Value::f64(fields.float64),
    ]
}

fn describe_arrays(fields: &ArraysFields) -> Vec<Value> {
    vec![
        Value::I8Array(fields.int8.to_vec()),
        Value::U8Array(fields.uint8.to_vec()),
        Value::I16Array(fields.int16.to_vec()),
        Value::U16Array(fields.uint16.to_vec()),
        Value::I32Array(fields.int32.to_vec()),
        Value::U32Array(fields.uint32.to_vec()),
        Value::I64Array(fields.int64.to_vec()),
        Value::U64Array(fields.uint64.to_vec()),
        Value::f32_array(fields.float32.iter().copied()),
        Value::f64_array(fields.float64.iter().copied()),
    ]
}

fn describe_nested(fields: &NestedFields) -> Vec<Value> {
    vec![
        Value::string(fields.label),
        Value::Table(describe_outer(&fields.outer)),
        Value::Vector(fields.trees.iter().map(describe_tree).collect()),
    ]
}

fn describe_outer(fields: &OuterFields) -> Vec<Value> {
    vec![
        Value::U32(fields.count),
        Value::Table(describe_inner(&fields.inner)),
    ]
}

fn describe_inner(fields: &InnerFields) -> Vec<Value> {
    vec![
        Value::string(fields.name),
        Value::f64_array(fields.values.iter().copied()),
    ]
}

fn describe_tree(fields: &TreeFields) -> Vec<Value> {
    vec![
        Value::I32(fields.value),
        Value::Vector(fields.children.iter().map(describe_tree).collect()),
    ]
}

fn describe_reserved(fields: &ReservedFields) -> Vec<Value> {
    vec![Value::Table(describe_keywords(&fields.keywords))]
}

fn describe_keywords(fields: &KeywordsFields) -> Vec<Value> {
    vec![
        Value::U8(fields.class),
        Value::U8(fields.default),
        Value::U8(fields.r#in),
        Value::U8(fields.r#match),
        Value::U8(fields.self_),
        Value::string(fields.r#type),
    ]
}

/// The fields of a `Packed`, which is read whole, to compare by their bits
fn describe_packed(packed: &Packed) -> Vec<Value> {
    vec![
        Value::I8(packed.int8),
        Value::U8(packed.uint8),
        Value::I16(packed.int16),
        Value::U16(packed.uint16),
        Value::I32(packed.int32),
        Value::U32(packed.uint32),
        Value::I64(packed.int64),
        Value::U64(packed.uint64),
        Value::f32(packed.float32),
        Value::f64(packed.float64),
    ]
}

/// A `Packed`'s fields little-endian at their offsets, one after another,
/// written with the standard library rather than the generated code
fn packed_bytes(packed: &Packed) -> Vec<u8> {
    [
        &packed.int8.to_le_bytes()[..],
        &packed.uint8.to_le_bytes(),
        &packed.int16.to_le_bytes(),
        &packed.uint16.to_le_bytes(),
        &packed.int32.to_le_bytes(),
        &packed.uint32.to_le_bytes(),
        &packed.int64.to_le_bytes(),
        &packed.uint64.to_le_bytes(),
        &packed.float32.to_le_bytes(),
        &packed.float64.to_le_bytes(),
    ]
    .concat()
}

/// A `Documented`'s fields little-endian at their offsets, as
/// [`packed_bytes()`] writes a `Packed`'s
fn documented_bytes(documented: Documented) -> Vec<u8> {
    [
        &documented.one.to_le_bytes()[..],
        &documented.two.to_le_bytes(),
        &documented.undocumented.to_le_bytes(),
    ]
    .concat()
}

// Arbitrary values of the fields

fn scalars() -> impl Strategy<Value = ScalarsFields> {
    let integers = (
        integers(i8::MIN, i8::MAX),
        integers(0, u8::MAX),
        integers(i16::MIN, i16::MAX),
        integers(0, u16::MAX),
        integers(i32::MIN, i32::MAX),
        integers(0, u32::MAX),
        integers(i64::MIN, i64::MAX),
        integers(0, u64::MAX),
    );

    (integers, float32s(), float64s()).prop_map(|(integers, float32, float64)| {
        let (int8, uint8, int16, uint16, int32, uint32, int64, uint64) = integers;
        ScalarsFields {
            int8,
            uint8,
            int16,
            uint16,
            int32,
            uint32,
            int64,
            uint64,
            float32,
            float64,
        }
    })
}

/// The values of an `Arrays`' fields, which `ArraysFields` borrows
#[derive(Clone, Debug)]
struct OwnedArrays {
    int8: Vec<i8>,
    uint8: Vec<u8>,
    int16: Vec<i16>,
    uint16: Vec<u16>,
    int32: Vec<i32>,
    uint32: Vec<u32>,
    int64: Vec<i64>,
    uint64: Vec<u64>,
    float32: Vec<f32>,
    float64: Vec<f64>,
}

impl OwnedArrays {
    fn fields(&self) -> ArraysFields<'_> {
        ArraysFields {
            int8: &self.int8,
            uint8: &self.uint8,
            int16: &self.int16,
            uint16: &self.uint16,
            int32: &self.int32,
            uint32: &self.uint32,
            int64: &self.int64,
            uint64: &self.uint64,
            float32: &self.float32,
            float64: &self.float64,
        }
    }
}

fn owned_arrays() -> impl Strategy<Value = OwnedArrays> {
    let integers = (
        arrays(integers(i8::MIN, i8::MAX)),
        arrays(integers(0, u8::MAX)),
        arrays(integers(i16::MIN, i16::MAX)),
        arrays(integers(0, u16::MAX)),
        arrays(integers(i32::MIN, i32::MAX)),
        arrays(integers(0, u32::MAX)),
        arrays(integers(i64::MIN, i64::MAX)),
        arrays(integers(0, u64::MAX)),
    );

    (integers, arrays(float32s()), arrays(float64s())).prop_map(|(integers, float32, float64)| {
        let (int8, uint8, int16, uint16, int32, uint32, int64, uint64) = integers;
        OwnedArrays {
            int8,
            uint8,
            int16,
            uint16,
            int32,
            uint32,
            int64,
            uint64,
            float32,
            float64,
        }
    })
}

/// A `Tree` that owns its children, rather than borrowing them as
/// `TreeFields` does
#[derive(Clone, Debug)]
struct OwnedTree {
    value: i32,
    children: Vec<OwnedTree>,
}

/// The most trees in a vector of them, fewer than of other elements, since
/// they nest
const MAX_TREES: usize = 3;

/// Vectors of trees of up to four levels, empty too
fn forests() -> impl Strategy<Value = Vec<OwnedTree>> {
    let values = || integers(i32::MIN, i32::MAX);
    let leaves = values().prop_map(|value| OwnedTree {
        value,
        children: Vec::new(),
    });
    let trees = leaves.prop_recursive(3, 24, 3, move |trees| {
        let children = vec(trees, 0..=MAX_TREES);
        (values(), children).prop_map(|(value, children)| OwnedTree { value, children })
    });

    vec(trees, 0..=MAX_TREES)
}

/// A tree in a level of trees: its value, and the range of its children in
/// the level below
struct Node {
    value: i32,
    children: Range<usize>,
}

/// Calls `f` with the `TreeFields` of `trees`. The fields of a tree borrow
/// those of its children, so they're made a level at a time from the
/// deepest, each level in a frame of the recursion that the level above
/// borrows from.
fn with_tree_fields<R>(trees: &[OwnedTree], f: impl FnOnce(&[TreeFields<'_>]) -> R) -> R {
    with_levels(&levels(trees), &[], f)
}

/// The trees by level from the vector that holds them, with the children of
/// each level's trees in the next, in order
fn levels(trees: &[OwnedTree]) -> Vec<Vec<Node>> {
    let mut levels = Vec::new();
    let mut level: Vec<&OwnedTree> = trees.iter().collect();

    while !level.is_empty() {
        let mut below = Vec::new();
        let nodes = level
            .iter()
            .map(|&tree| {
                let start = below.len();
                below.extend(&tree.children);

                Node {
                    value: tree.value,
                    children: start..below.len(),
                }
            })
            .collect();

        levels.push(nodes);
        level = below;
    }

    levels
}

fn with_levels<R>(
    levels: &[Vec<Node>],
    below: &[TreeFields<'_>],
    f: impl FnOnce(&[TreeFields<'_>]) -> R,
) -> R {
    let Some((deepest, above)) = levels.split_last() else {
        return f(below);
    };

    let fields: Vec<TreeFields<'_>> = deepest
        .iter()
        .map(|node| TreeFields {
            value: node.value,
            children: &below[node.children.clone()],
        })
        .collect();

    with_levels(above, &fields, f)
}

/// The values of a `Nested`'s fields and of the tables it nests
#[derive(Clone, Debug)]
struct OwnedNested {
    label: String,
    count: u32,
    name: String,
    values: Vec<f64>,
    trees: Vec<OwnedTree>,
}

impl OwnedNested {
    /// Calls `f` with the fields, which borrow those of the trees
    fn with_fields<R>(&self, f: impl FnOnce(&NestedFields<'_>) -> R) -> R {
        with_tree_fields(&self.trees, |trees| {
            f(&NestedFields {
                label: &self.label,
                outer: OuterFields {
                    count: self.count,
                    inner: InnerFields {
                        name: &self.name,
                        values: &self.values,
                    },
                },
                trees,
            })
        })
    }
}

fn owned_nested() -> impl Strategy<Value = OwnedNested> {
    let outer = (integers(0, u32::MAX), strings(), arrays(float64s()));

    (strings(), outer, forests()).prop_map(|(label, (count, name, values), trees)| OwnedNested {
        label,
        count,
        name,
        values,
        trees,
    })
}

/// The values of a `Keywords`' fields, which a `Reserved` nests
#[derive(Clone, Debug)]
struct OwnedKeywords {
    /// `class`, `default`, `in`, `match` and `self`
    bytes: [u8; 5],
    r#type: String,
}

impl OwnedKeywords {
    fn fields(&self) -> ReservedFields<'_> {
        let [class, default, r#in, r#match, self_] = self.bytes;

        ReservedFields {
            keywords: KeywordsFields {
                class,
                default,
                r#in,
                r#match,
                self_,
                r#type: &self.r#type,
            },
        }
    }
}

fn owned_keywords() -> impl Strategy<Value = OwnedKeywords> {
    (uniform5(integers(0, u8::MAX)), strings())
        .prop_map(|(bytes, r#type)| OwnedKeywords { bytes, r#type })
}

fn packeds() -> impl Strategy<Value = Packed> {
    scalars().prop_map(|fields| Packed {
        int8: fields.int8,
        uint8: fields.uint8,
        int16: fields.int16,
        uint16: fields.uint16,
        int32: fields.int32,
        uint32: fields.uint32,
        int64: fields.int64,
        uint64: fields.uint64,
        float32: fields.float32,
        float64: fields.float64,
    })
}

fn documenteds() -> impl Strategy<Value = Documented> {
    let fields = (
        integers(0, u8::MAX),
        integers(0, u16::MAX),
        integers(0, u32::MAX),
    );

    fields.prop_map(|(one, two, undocumented)| Documented {
        one,
        two,
        undocumented,
    })
}

/// Bytes too short for a struct of `size` bytes
fn shorter_than(size: usize) -> impl Strategy<Value = Vec<u8>> {
    vec(any::<u8>(), 0..size)
}

/// Bytes to follow a struct's
fn after() -> impl Strategy<Value = Vec<u8>> {
    vec(any::<u8>(), 1..=MAX_LENGTH)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    // Round trips

    #[test]
    fn scalars_read_back_every_scalar_type_wherever_their_bytes_lie(
        fields in scalars(),
        placement in placements(),
    ) {
        let bytes = placement.place(&fields.serialize()?);

        let read = read_scalars(&Scalars::new(bytes.bytes()));
        prop_assert_eq!(read, expected(describe_scalars(&fields)));
    }

    #[test]
    fn arrays_read_back_an_array_of_every_scalar_type_wherever_their_bytes_lie(
        arrays in owned_arrays(),
        placement in placements(),
    ) {
        let fields = arrays.fields();
        let bytes = placement.place(&fields.serialize()?);

        let read = read_arrays(&Arrays::new(bytes.bytes()));
        prop_assert_eq!(read, expected(describe_arrays(&fields)));
    }

    #[test]
    fn nested_reads_back_its_nested_tables_and_trees_wherever_its_bytes_lie(
        nested in owned_nested(),
        placement in placements(),
    ) {
        nested.with_fields(|fields| -> Result<(), TestCaseError> {
            let bytes = placement.place(&fields.serialize()?);

            let read = read_nested(&Nested::new(bytes.bytes()));
            prop_assert_eq!(read, expected(describe_nested(fields)));
            Ok(())
        })?;
    }

    #[test]
    fn reserved_reads_back_the_fields_named_after_keywords_wherever_its_bytes_lie(
        keywords in owned_keywords(),
        placement in placements(),
    ) {
        let fields = keywords.fields();
        let bytes = placement.place(&fields.serialize()?);

        let read = read_reserved(&Reserved::new(bytes.bytes()));
        prop_assert_eq!(read, expected(describe_reserved(&fields)));
    }

    // Tables built by hand

    #[test]
    fn scalars_are_written_as_table_builder_writes_them(fields in scalars()) {
        prop_assert_eq!(fields.serialize(), build(MESSAGE, &describe_scalars(&fields)));
    }

    #[test]
    fn arrays_are_written_as_table_builder_writes_them(arrays in owned_arrays()) {
        let fields = arrays.fields();

        prop_assert_eq!(fields.serialize(), build(MESSAGE, &describe_arrays(&fields)));
    }

    #[test]
    fn nested_is_written_as_table_builder_writes_it(nested in owned_nested()) {
        nested.with_fields(|fields| -> Result<(), TestCaseError> {
            prop_assert_eq!(fields.serialize(), build(MESSAGE, &describe_nested(fields)));
            Ok(())
        })?;
    }

    #[test]
    fn reserved_is_written_as_table_builder_writes_it(keywords in owned_keywords()) {
        let fields = keywords.fields();

        prop_assert_eq!(fields.serialize(), build(MESSAGE, &describe_reserved(&fields)));
    }

    // Decode errors

    #[test]
    fn corrupted_scalars_read_as_values_or_decode_errors(
        fields in scalars(),
        overwrite in overwrites(),
    ) {
        let bytes = fields.serialize()?;
        let written = describe_scalars(&fields);

        for truncated in truncations(&bytes) {
            check_truncated(Ok(read_scalars(&Scalars::new(truncated))), &written)?;
        }

        // Any values can be read from overwritten bytes, as long as no getter
        // panics
        read_scalars(&Scalars::new(&overwrite.apply(&bytes)));
    }

    #[test]
    fn corrupted_arrays_read_as_values_or_decode_errors(
        arrays in owned_arrays(),
        overwrite in overwrites(),
    ) {
        let fields = arrays.fields();
        let bytes = fields.serialize()?;
        let written = describe_arrays(&fields);

        for truncated in truncations(&bytes) {
            check_truncated(Ok(read_arrays(&Arrays::new(truncated))), &written)?;
        }

        read_arrays(&Arrays::new(&overwrite.apply(&bytes)));
    }

    #[test]
    fn a_corrupted_nested_reads_as_values_or_decode_errors(
        nested in owned_nested(),
        overwrite in overwrites(),
    ) {
        nested.with_fields(|fields| -> Result<(), TestCaseError> {
            let bytes = fields.serialize()?;
            let written = describe_nested(fields);

            for truncated in truncations(&bytes) {
                check_truncated(Ok(read_nested(&Nested::new(truncated))), &written)?;
            }

            read_nested(&Nested::new(&overwrite.apply(&bytes)));
            Ok(())
        })?;
    }

    #[test]
    fn a_corrupted_reserved_reads_as_values_or_decode_errors(
        keywords in owned_keywords(),
        overwrite in overwrites(),
    ) {
        let fields = keywords.fields();
        let bytes = fields.serialize()?;
        let written = describe_reserved(&fields);

        for truncated in truncations(&bytes) {
            check_truncated(Ok(read_reserved(&Reserved::new(truncated))), &written)?;
        }

        read_reserved(&Reserved::new(&overwrite.apply(&bytes)));
    }

    #[test]
    fn arbitrary_bytes_read_as_any_message_or_decode_errors(bytes in arbitrary_bytes()) {
        read_scalars(&Scalars::new(&bytes));
        read_arrays(&Arrays::new(&bytes));
        read_nested(&Nested::new(&bytes));
        read_reserved(&Reserved::new(&bytes));
    }

    // The structs

    #[test]
    fn packed_is_written_in_42_bytes_at_fixed_offsets_and_read_back_wherever_they_lie(
        packed in packeds(),
        placement in placements(),
    ) {
        let bytes = packed.to_bytes();
        prop_assert_eq!(Packed::SIZE, 42);
        prop_assert_eq!(bytes.to_vec(), packed_bytes(&packed));

        let read = Packed::read(placement.place(&bytes).bytes())?;
        prop_assert_eq!(describe_packed(&read), describe_packed(&packed));
    }

    #[test]
    fn packed_is_read_from_the_start_of_longer_bytes(packed in packeds(), after in after()) {
        let bytes = [&packed.to_bytes()[..], &after].concat();

        let read = Packed::read(&bytes)?;
        prop_assert_eq!(describe_packed(&read), describe_packed(&packed));
    }

    #[test]
    fn packed_fails_to_read_from_fewer_than_42_bytes(bytes in shorter_than(Packed::SIZE)) {
        let read = Packed::read(&bytes);
        prop_assert!(matches!(read, Err(DecodeError::OutOfRange { .. })), "{:?}", read);
    }

    #[test]
    fn documented_is_written_in_7_bytes_at_fixed_offsets_and_read_back_wherever_they_lie(
        documented in documenteds(),
        placement in placements(),
    ) {
        let bytes = documented.to_bytes();
        prop_assert_eq!(Documented::SIZE, 7);
        prop_assert_eq!(bytes.to_vec(), documented_bytes(documented));

        let read = Documented::read(placement.place(&bytes).bytes())?;
        prop_assert_eq!(read, documented);
    }

    #[test]
    fn documented_is_read_from_the_start_of_longer_bytes(
        documented in documenteds(),
        after in after(),
    ) {
        let bytes = [&documented.to_bytes()[..], &after].concat();

        prop_assert_eq!(Documented::read(&bytes)?, documented);
    }

    #[test]
    fn documented_fails_to_read_from_fewer_than_7_bytes(bytes in shorter_than(Documented::SIZE)) {
        let read = Documented::read(&bytes);
        prop_assert!(matches!(read, Err(DecodeError::OutOfRange { .. })), "{:?}", read);
    }
}
