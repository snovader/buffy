//! Reads and rebuilds the tables of buffy's conformance suite, which
//! `@buffy/core` writes. See `conformance/README.md` at the root of the
//! repository for the format of their descriptions in `cases.json`.

mod common;

use std::{fmt::Debug, fs, path::Path, str::FromStr};

use buffy_core::{
    Alignment, BuildError, ByteBuffer, DecodeError, NestedTable, OffsetSize, TableBuilder,
    TableLayout,
};
use common::Field;
use serde_json::Value;

const CONFORMANCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance");

/// A table as `cases.json` describes it, with its scalars, arrays and strings
/// as the `Field`s of the other tests
struct Table {
    layout: Layout,
    version: Option<u32>,
    fields: Vec<Described>,
}

struct Layout {
    sized: bool,
    versioned: bool,
    offset_size: OffsetSize,
}

enum Described {
    Value(Field),
    Table {
        nested: Nested,
        alignment: Option<Alignment>,
    },
    Vector(Vec<Nested>),
}

/// A table nested from its builder or from the bytes that its builder writes
struct Nested {
    from_builder: bool,
    table: Table,
}

struct ValidCase {
    name: String,
    bytes: Vec<u8>,
    table: Table,
}

fn suite() -> Value {
    let json = fs::read_to_string(Path::new(CONFORMANCE).join("cases.json")).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn read_file(case: &Value) -> Vec<u8> {
    fs::read(Path::new(CONFORMANCE).join(case["file"].as_str().unwrap())).unwrap()
}

fn valid_cases(suite: &Value) -> Vec<ValidCase> {
    let cases = suite["valid"].as_array().unwrap();

    cases
        .iter()
        .map(|case| ValidCase {
            name: case["name"].as_str().unwrap().to_owned(),
            bytes: read_file(case),
            table: table_of(&case["table"]),
        })
        .collect()
}

#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_reads_every_valid_table() {
    for case in valid_cases(&suite()) {
        read_table(&case.bytes, &case.table, &case.name)
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
    }
}

#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_rebuilds_every_valid_table_byte_for_byte() {
    for case in valid_cases(&suite()) {
        let built = build(&case.table).and_then(|builder| builder.to_bytes());
        assert_eq!(built, Ok(case.bytes), "{}", case.name);
    }
}

#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_reads_invalid_tables_up_to_the_field_that_fails() {
    let suite = suite();
    let valid = valid_cases(&suite);

    for case in suite["invalid"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let base = case["case"].as_str().unwrap();
        let field = usize::try_from(case["field"].as_u64().unwrap()).unwrap();
        let table = &valid.iter().find(|valid| valid.name == base).unwrap().table;
        let bytes = read_file(case);

        let layout = table.layout.unversioned();
        let result = if table.layout.versioned {
            read_up_to(
                &ByteBuffer::new(&bytes, layout.versioned()),
                table,
                field,
                name,
            )
        } else {
            read_up_to(&ByteBuffer::new(&bytes, layout), table, field, name)
        };

        assert!(
            result.is_err(),
            "{name}: read field {field} without an error"
        );
    }
}

/// Reads the root fields before `field`, which must hold their described
/// values, and returns the result of reading `field`
fn read_up_to<const V: bool>(
    buffer: &ByteBuffer<'_, V>,
    table: &Table,
    field: usize,
    name: &str,
) -> Result<(), DecodeError> {
    for (index, described) in table.fields[..field].iter().enumerate() {
        let path = format!("{name}/{index}");
        read_root_field(buffer, index, described, &path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }

    read_root_field(
        buffer,
        field,
        &table.fields[field],
        &format!("{name}/{field}"),
    )
}

fn table_of(value: &Value) -> Table {
    let layout = &value["layout"];
    let offset_size = match layout["offsetSize"].as_u64() {
        Some(1) => OffsetSize::U8,
        Some(2) => OffsetSize::U16,
        Some(4) => OffsetSize::U32,
        other => panic!("Offsets can't be {other:?} bytes wide"),
    };

    Table {
        layout: Layout {
            sized: layout["sized"].as_bool().unwrap(),
            versioned: layout["versioned"].as_bool().unwrap(),
            offset_size,
        },
        version: value["version"]
            .as_u64()
            .map(|version| u32::try_from(version).unwrap()),
        fields: value["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(described_of)
            .collect(),
    }
}

fn described_of(field: &Value) -> Described {
    let value = &field["value"];

    match field["type"].as_str().unwrap() {
        "table" => Described::Table {
            nested: nested_of(field),
            alignment: field["alignment"].as_u64().map(alignment_of),
        },
        "vector" => Described::Vector(
            field["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(nested_of)
                .collect(),
        ),
        "string" => Described::Value(Field::String(value.as_str().unwrap().to_owned())),
        kind => Described::Value(field_of(kind, value)),
    }
}

/// Returns a scalar or array field from the JSON that holds 64-bit integers
/// as strings and the floats that JSON has no number for by name
fn field_of(kind: &str, value: &Value) -> Field {
    match kind {
        "int8" => Field::I8(integer(value)),
        "uint8" => Field::U8(integer(value)),
        "int16" => Field::I16(integer(value)),
        "uint16" => Field::U16(integer(value)),
        "int32" => Field::I32(integer(value)),
        "uint32" => Field::U32(integer(value)),
        "int64" => Field::I64(parse(value)),
        "uint64" => Field::U64(parse(value)),
        "float32" => Field::F32(float32(value)),
        "float64" => Field::F64(float64(value)),
        "int8[]" => Field::I8Array(elements(value, integer)),
        "uint8[]" => Field::U8Array(elements(value, integer)),
        "int16[]" => Field::I16Array(elements(value, integer)),
        "uint16[]" => Field::U16Array(elements(value, integer)),
        "int32[]" => Field::I32Array(elements(value, integer)),
        "uint32[]" => Field::U32Array(elements(value, integer)),
        "int64[]" => Field::I64Array(elements(value, parse)),
        "uint64[]" => Field::U64Array(elements(value, parse)),
        "float32[]" => Field::F32Array(elements(value, float32)),
        "float64[]" => Field::F64Array(elements(value, float64)),
        other => panic!("Unknown type {other}"),
    }
}

fn nested_of(value: &Value) -> Nested {
    Nested {
        from_builder: value["from"] == "builder",
        table: table_of(&value["table"]),
    }
}

fn alignment_of(alignment: u64) -> Alignment {
    match alignment {
        1 => Alignment::One,
        2 => Alignment::Two,
        4 => Alignment::Four,
        8 => Alignment::Eight,
        other => panic!("Tables can't be aligned to {other}"),
    }
}

fn elements<T>(value: &Value, element: fn(&Value) -> T) -> Vec<T> {
    value.as_array().unwrap().iter().map(element).collect()
}

fn integer<T: TryFrom<i64, Error: Debug>>(value: &Value) -> T {
    T::try_from(value.as_i64().unwrap()).unwrap()
}

fn parse<T: FromStr<Err: Debug>>(value: &Value) -> T {
    value.as_str().unwrap().parse().unwrap()
}

fn float64(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap(),
        Value::String(name) => match name.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            "-0" => -0.0,
            other => panic!("Not a number: {other}"),
        },
        other => panic!("Not a number: {other}"),
    }
}

/// Returns a float32 that JSON holds as the f64 that equals it, or by name
fn float32(value: &Value) -> f32 {
    let float = float64(value);

    // NaN converts to an unspecified NaN, rather than the quiet one that the
    // tables hold
    if float.is_nan() {
        return f32::NAN;
    }

    #[allow(clippy::cast_possible_truncation)]
    let narrowed = float as f32;
    assert_eq!(
        f64::from(narrowed).to_bits(),
        float.to_bits(),
        "{float} isn't a float32"
    );

    narrowed
}

impl Layout {
    /// The layout without its version slot, which `versioned()` adds
    const fn unversioned(&self) -> TableLayout {
        let layout = TableLayout::DEFAULT.with_offset_size(self.offset_size);
        if self.sized { layout.sized() } else { layout }
    }
}

/// Builds a table as its description sets it out, borrowing its values
fn build(table: &Table) -> Result<TableBuilder<'_>, BuildError> {
    let layout = table.layout.unversioned();
    let mut builder = if table.layout.versioned {
        TableBuilder::versioned(
            table.fields.len(),
            layout.versioned(),
            table.version.unwrap(),
        )?
    } else {
        TableBuilder::new(table.fields.len(), layout)?
    };

    for field in &table.fields {
        set(&mut builder, field)?;
    }

    Ok(builder)
}

fn set<'a>(builder: &mut TableBuilder<'a>, field: &'a Described) -> Result<(), BuildError> {
    match field {
        Described::Value(value) => value.set(builder),
        Described::Table { nested, alignment } => {
            let table = nested.build()?;
            match alignment {
                Some(alignment) => builder.set_table_aligned(table, *alignment),
                None => builder.set_table(table),
            }
        }
        Described::Vector(items) => {
            let items = items
                .iter()
                .map(Nested::build)
                .collect::<Result<Vec<_>, _>>()?;
            builder.set_vector(items)
        }
    }
}

impl Nested {
    fn build(&self) -> Result<NestedTable<'_>, BuildError> {
        let builder = build(&self.table)?;

        Ok(if self.from_builder {
            builder.into()
        } else {
            builder.to_bytes()?.into()
        })
    }
}

/// Reads a table from its own bytes, and asserts that they hold its described
/// header and values. A nested table with no header and its parent's offset
/// size is read in place; any other is read as a table of its own. Vector
/// items are always read from their bytes, which a `Vector` returns, and
/// which have the same offsets.
fn read_table(bytes: &[u8], table: &Table, path: &str) -> Result<(), DecodeError> {
    let layout = table.layout.unversioned();

    if table.layout.versioned {
        let buffer = ByteBuffer::new(bytes, layout.versioned());
        assert_eq!(buffer.version()?, table.version.unwrap(), "{path}: version");

        read_fields(&buffer, table, path)
    } else {
        read_fields(&ByteBuffer::new(bytes, layout), table, path)
    }
}

fn read_fields<const V: bool>(
    buffer: &ByteBuffer<'_, V>,
    table: &Table,
    path: &str,
) -> Result<(), DecodeError> {
    if table.layout.sized {
        assert_eq!(buffer.size()?, buffer.bytes().len(), "{path}: size");
    }

    for (index, field) in table.fields.iter().enumerate() {
        read_root_field(buffer, index, field, &format!("{path}/{index}"))?;
    }

    Ok(())
}

fn read_root_field<const V: bool>(
    buffer: &ByteBuffer<'_, V>,
    index: usize,
    field: &Described,
    path: &str,
) -> Result<(), DecodeError> {
    read_field(buffer, buffer.root_offset(index)?, field, path)
}

fn read_field<const V: bool>(
    buffer: &ByteBuffer<'_, V>,
    position: usize,
    field: &Described,
    path: &str,
) -> Result<(), DecodeError> {
    match field {
        Described::Value(expected) => {
            assert_eq!(&expected.read(buffer, position)?, expected, "{path}");
        }
        Described::Table { nested, .. } => read_nested(buffer, position, &nested.table, path)?,
        Described::Vector(items) => {
            let vector = buffer.get_vector(position)?;
            assert_eq!(vector.len(), items.len(), "{path}: items");

            for (index, (bytes, item)) in vector.iter().zip(items).enumerate() {
                read_table(bytes, &item.table, &format!("{path}/{index}"))?;
            }
        }
    }

    Ok(())
}

/// Reads a table nested with `set_table()`
fn read_nested<const V: bool>(
    buffer: &ByteBuffer<'_, V>,
    position: usize,
    table: &Table,
    path: &str,
) -> Result<(), DecodeError> {
    let layout = &table.layout;
    let in_place =
        !layout.sized && !layout.versioned && layout.offset_size == buffer.layout().offset_size();

    if !in_place {
        // Its length isn't written, so it's read up to the end of its parent.
        // A position past the end leaves no bytes, which the first read
        // fails for.
        let bytes = buffer.bytes();
        return read_table(&bytes[position.min(bytes.len())..], table, path);
    }

    for (index, field) in table.fields.iter().enumerate() {
        let field_position = buffer.field_offset(position, index)?;
        read_field(buffer, field_position, field, &format!("{path}/{index}"))?;
    }

    Ok(())
}
