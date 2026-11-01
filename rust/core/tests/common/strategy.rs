//! Strategies of table descriptions in the conformance suite's shape: tables
//! of every layout, with fields of every type holding values across their
//! range, and tables nested in them, from builders and from bytes, to a
//! bounded depth.
//!
//! Tables follow the rules of the format and the builder's documentation:
//! a version fits in its slot, a table nested with `set_table()` has no
//! header, and a table and the tables in it fit in their offset sizes.

use buffy_core::{Alignment, OffsetSize};
use proptest::{char, collection::vec, num, option, prelude::*, sample::select};

use super::description::{Field, Layout, Nested, Source, Table};

/// How deeply tables nest: the root's fields can hold tables whose fields
/// hold tables
const DEPTH: u32 = 2;

const OFFSET_SIZES: [OffsetSize; 3] = [OffsetSize::U8, OffsetSize::U16, OffsetSize::U32];

/// Tables of any layout
pub fn table() -> BoxedStrategy<Table> {
    tables(layout())
}

/// Tables of the layouts that `layouts` generates
pub fn tables(layouts: impl Strategy<Value = Layout> + 'static) -> BoxedStrategy<Table> {
    tables_of(layouts.boxed(), DEPTH, false, &Values::new())
}

/// Layouts of any header and offset size
pub fn layout() -> impl Strategy<Value = Layout> {
    layouts(offset_size())
}

/// Layouts of any header, of the offset sizes that `offset_sizes` generates
pub fn layouts(offset_sizes: impl Strategy<Value = OffsetSize>) -> impl Strategy<Value = Layout> {
    (any::<bool>(), any::<bool>(), offset_sizes).prop_map(|(sized, versioned, offset_size)| {
        Layout {
            sized,
            versioned,
            offset_size,
        }
    })
}

pub fn offset_size() -> impl Strategy<Value = OffsetSize> + Clone {
    select(OFFSET_SIZES.to_vec())
}

/// The most fields, array elements, characters and vector items that a
/// table holds. A table of 1-byte offsets, or nested in one, holds few
/// enough to fit in its 255 bytes more often than not, and others hold
/// enough for 2-byte lengths and offsets past 255.
#[derive(Clone, Copy)]
struct Limits {
    fields: usize,
    elements: usize,
    characters: usize,
    items: usize,
}

impl Limits {
    const fn of(small: bool) -> Self {
        if small {
            Self {
                fields: 5,
                elements: 6,
                characters: 6,
                items: 2,
            }
        } else {
            Self {
                fields: 8,
                elements: 24,
                characters: 16,
                items: 4,
            }
        }
    }
}

/// The strategies of scalars, arrays and strings, which a strategy of
/// tables creates once for all its fields: creating them for each kind of
/// field took Miri 16 seconds for every test
struct Values {
    large: BoxedStrategy<Field>,
    small: BoxedStrategy<Field>,
}

impl Values {
    fn new() -> Self {
        Self {
            large: value(Limits::of(false)),
            small: value(Limits::of(true)),
        }
    }

    fn of(&self, small: bool) -> BoxedStrategy<Field> {
        if small { &self.small } else { &self.large }.clone()
    }
}

/// Tables of `layouts` whose fields hold tables `depth` deep, and hold few
/// values when `small`
fn tables_of(
    layouts: BoxedStrategy<Layout>,
    depth: u32,
    small: bool,
    values: &Values,
) -> BoxedStrategy<Table> {
    // The fields of each offset size, created once rather than for each table
    let fields_of_size = OFFSET_SIZES.map(|offset_size| {
        let small = small || offset_size == OffsetSize::U8;
        vec(
            field(offset_size, depth, small, values),
            0..=Limits::of(small).fields,
        )
        .boxed()
    });

    let fields_of = move |layout: Layout| {
        let index = OFFSET_SIZES
            .iter()
            .position(|&size| size == layout.offset_size);
        let fields = fields_of_size[index.unwrap()].clone();

        (Just(layout), version(layout), fields)
    };

    layouts
        .prop_flat_map(fields_of)
        .prop_map(|(layout, version, fields)| {
            fitted(Table {
                layout,
                version,
                fields,
            })
        })
        .boxed()
}

/// Versions that fit in the layout's slot, from 0 to its limit, for a
/// versioned layout
fn version(layout: Layout) -> BoxedStrategy<Option<u32>> {
    if !layout.versioned {
        return Just(None).boxed();
    }

    let max = layout.offset_size.max_value();
    prop_oneof![Just(0), Just(max), 0..=max]
        .prop_map(Some)
        .boxed()
}

/// Fields of every type, and tables nested `depth` deep
fn field(
    offset_size: OffsetSize,
    depth: u32,
    small: bool,
    values: &Values,
) -> BoxedStrategy<Field> {
    if depth == 0 {
        return values.of(small);
    }

    prop_oneof![
        8 => values.of(small),
        1 => nested_table(offset_size, depth - 1, small, values),
        1 => vector(depth - 1, small, values),
    ]
    .boxed()
}

/// Scalars, arrays and strings, which hold values rather than tables
fn value(limits: Limits) -> BoxedStrategy<Field> {
    let elements = 0..=limits.elements;

    prop_oneof![
        1 => int8().prop_map(Field::I8),
        1 => uint8().prop_map(Field::U8),
        1 => int16().prop_map(Field::I16),
        1 => uint16().prop_map(Field::U16),
        1 => int32().prop_map(Field::I32),
        1 => uint32().prop_map(Field::U32),
        1 => int64().prop_map(Field::I64),
        1 => uint64().prop_map(Field::U64),
        1 => float32().prop_map(Field::F32),
        1 => float64().prop_map(Field::F64),
        1 => vec(int8(), elements.clone()).prop_map(Field::I8Array),
        1 => vec(uint8(), elements.clone()).prop_map(Field::U8Array),
        1 => vec(int16(), elements.clone()).prop_map(Field::I16Array),
        1 => vec(uint16(), elements.clone()).prop_map(Field::U16Array),
        1 => vec(int32(), elements.clone()).prop_map(Field::I32Array),
        1 => vec(uint32(), elements.clone()).prop_map(Field::U32Array),
        1 => vec(int64(), elements.clone()).prop_map(Field::I64Array),
        1 => vec(uint64(), elements.clone()).prop_map(Field::U64Array),
        1 => vec(float32(), elements.clone()).prop_map(Field::F32Array),
        1 => vec(float64(), elements).prop_map(Field::F64Array),
        // As likely as three types of number
        3 => string(limits.characters).prop_map(Field::String),
    ]
    .boxed()
}

macro_rules! integers {
    ($($name:ident: $type:ty),*) => {$(
        /// Integers across their range, and their limits and 0 more often
        /// than chance
        fn $name() -> impl Strategy<Value = $type> {
            prop_oneof![
                1 => select(vec![<$type>::MIN, 0, <$type>::MAX]),
                3 => any::<$type>(),
            ]
        }
    )*};
}

integers!(
    int8: i8, uint8: u8,
    int16: i16, uint16: u16,
    int32: i32, uint32: u32,
    int64: i64, uint64: u64
);

/// Floats of every class and sign, NaNs with any payload among them, and
/// -0, NaN, the infinities and the limits more often than chance
fn float32() -> impl Strategy<Value = f32> {
    let special = vec![
        -0.0,
        f32::NAN,
        f32::from_bits(f32::NAN.to_bits() | 1 << 31),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
    ];

    prop_oneof![
        1 => select(special),
        3 => num::f32::ANY | num::f32::SIGNALING_NAN,
    ]
}

/// Floats like `float32()`'s, as float64s
fn float64() -> impl Strategy<Value = f64> {
    let special = vec![
        -0.0,
        f64::NAN,
        f64::from_bits(f64::NAN.to_bits() | 1 << 63),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::MIN,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
    ];

    prop_oneof![
        1 => select(special),
        3 => num::f64::ANY | num::f64::SIGNALING_NAN,
    ]
}

/// Strings of ASCII, of characters of two, three and four bytes in UTF-8,
/// the last outside the Basic Multilingual Plane, and of U+FEFF, which
/// isn't a byte order mark within a table, leading or not
fn string(characters: usize) -> impl Strategy<Value = String> {
    let character = prop_oneof![
        3 => char::range('a', 'z'),
        1 => Just('\u{feff}'),
        1 => char::range('\u{80}', '\u{7ff}'),
        1 => char::range('\u{800}', '\u{ffff}'),
        1 => char::range('\u{1_0000}', '\u{10_ffff}'),
    ];

    vec(character, 0..=characters).prop_map(String::from_iter)
}

/// Tables nested with `set_table()`, which have no header, from their
/// builders or bytes, aligned by default or explicitly. They're read in
/// place when their offsets are as wide as their parent's, as they are more
/// often than not.
fn nested_table(
    parent: OffsetSize,
    depth: u32,
    small: bool,
    values: &Values,
) -> impl Strategy<Value = Field> + use<> {
    let offset_sizes = prop_oneof![2 => Just(parent), 1 => offset_size()];
    let layouts = offset_sizes.prop_map(Layout::headerless).boxed();

    (
        nested(layouts, depth, small, values),
        option::of(alignment()),
    )
        .prop_map(|(nested, alignment)| Field::Table { nested, alignment })
}

/// Vectors of tables of any layout, from their builders or bytes
fn vector(depth: u32, small: bool, values: &Values) -> impl Strategy<Value = Field> + use<> {
    let items = 0..=Limits::of(small).items;
    let item = nested(layout().boxed(), depth, small, values);

    vec(item, items).prop_map(Field::Vector)
}

fn nested(
    layouts: BoxedStrategy<Layout>,
    depth: u32,
    small: bool,
    values: &Values,
) -> impl Strategy<Value = Nested> + use<> {
    let sources = prop_oneof![Just(Source::Builder), Just(Source::Bytes)];
    let tables = tables_of(layouts, depth, small, values);

    (sources, tables).prop_map(|(from, table)| Nested { from, table })
}

fn alignment() -> impl Strategy<Value = Alignment> {
    select(vec![
        Alignment::One,
        Alignment::Two,
        Alignment::Four,
        Alignment::Eight,
    ])
}

/// Keeps the table's first fields that surely fit in its offset size, as
/// many as its header and their longest lengths leave room for. The tables
/// nested in it were fitted to theirs.
fn fitted(mut table: Table) -> Table {
    let offset_size = table.layout.offset_size;
    let width = offset_size.bytes() as u64;
    let max = u64::from(offset_size.max_value());

    let mut length = table.layout.header_size() as u64;
    let fits = |field: &&Field| {
        length += width + longest(field, width);
        length <= max
    };

    let fields = table.fields.iter().take_while(fits).count();
    table.fields.truncate(fields);

    table
}

fn longest_table(table: &Table) -> u64 {
    let width = table.layout.offset_size.bytes() as u64;
    let fields = table.fields.iter();

    table.layout.header_size() as u64
        + fields
            .map(|field| width + longest(field, width))
            .sum::<u64>()
}

/// The longest that a field can be in a table of `width`-byte offsets, with
/// the most padding that it can need
fn longest(field: &Field, width: u64) -> u64 {
    let array = |len: usize, size: u64| size - 1 + width + len as u64 * size;

    match field {
        Field::I8(_) | Field::U8(_) => 1,
        Field::I16(_) | Field::U16(_) => 2,
        Field::I32(_) | Field::U32(_) | Field::F32(_) => 4,
        Field::I64(_) | Field::U64(_) | Field::F64(_) => 8,
        Field::I8Array(values) => array(values.len(), 1),
        Field::U8Array(values) => array(values.len(), 1),
        Field::I16Array(values) => array(values.len(), 2),
        Field::U16Array(values) => array(values.len(), 2),
        Field::I32Array(values) => array(values.len(), 4),
        Field::U32Array(values) => array(values.len(), 4),
        Field::F32Array(values) => array(values.len(), 4),
        Field::I64Array(values) => array(values.len(), 8),
        Field::U64Array(values) => array(values.len(), 8),
        Field::F64Array(values) => array(values.len(), 8),
        Field::String(string) => width + string.len() as u64,
        Field::Table { nested, .. } => 7 + longest_table(&nested.table),
        Field::Vector(items) => {
            let items = items.iter();
            width
                + items
                    .map(|item| width + longest_table(&item.table))
                    .sum::<u64>()
        }
    }
}
