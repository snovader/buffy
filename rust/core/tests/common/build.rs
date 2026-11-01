//! Builds tables as their descriptions set them out, with `TableBuilder`

use buffy_core::{BuildError, NestedTable, TableBuilder};

use super::description::{Field, Nested, Source, Table};

impl Table {
    /// Starts a builder of the table's layout, version and number of
    /// fields, with no field set
    pub fn builder<'a>(&self) -> Result<TableBuilder<'a>, BuildError> {
        let layout = self.layout.unversioned();
        let fields = self.fields.len();

        if self.layout.versioned {
            let version = self.version.expect("A versioned table has a version");
            TableBuilder::versioned(fields, layout.versioned(), version)
        } else {
            TableBuilder::new(fields, layout)
        }
    }

    /// Builds the table, borrowing its values
    pub fn build(&self) -> Result<TableBuilder<'_>, BuildError> {
        let mut builder = self.builder()?;
        for field in &self.fields {
            field.set(&mut builder)?;
        }

        Ok(builder)
    }

    /// Writes the table
    pub fn to_bytes(&self) -> Result<Vec<u8>, BuildError> {
        self.build()?.to_bytes()
    }
}

impl Field {
    /// Sets the field on `builder`, which borrows its value
    pub fn set<'a>(&'a self, builder: &mut TableBuilder<'a>) -> Result<(), BuildError> {
        match self {
            Self::I8(value) => builder.set_i8(*value),
            Self::U8(value) => builder.set_u8(*value),
            Self::I16(value) => builder.set_i16(*value),
            Self::U16(value) => builder.set_u16(*value),
            Self::I32(value) => builder.set_i32(*value),
            Self::U32(value) => builder.set_u32(*value),
            Self::I64(value) => builder.set_i64(*value),
            Self::U64(value) => builder.set_u64(*value),
            Self::F32(value) => builder.set_f32(*value),
            Self::F64(value) => builder.set_f64(*value),
            Self::I8Array(values) => builder.set_i8_array(values),
            Self::U8Array(values) => builder.set_u8_array(values),
            Self::I16Array(values) => builder.set_i16_array(values),
            Self::U16Array(values) => builder.set_u16_array(values),
            Self::I32Array(values) => builder.set_i32_array(values),
            Self::U32Array(values) => builder.set_u32_array(values),
            Self::I64Array(values) => builder.set_i64_array(values),
            Self::U64Array(values) => builder.set_u64_array(values),
            Self::F32Array(values) => builder.set_f32_array(values),
            Self::F64Array(values) => builder.set_f64_array(values),
            Self::String(string) => builder.set_str(string),
            Self::Table { nested, alignment } => {
                let table = nested.build()?;
                match alignment {
                    Some(alignment) => builder.set_table_aligned(table, *alignment),
                    None => builder.set_table(table),
                }
            }
            Self::Vector(items) => {
                let items = items
                    .iter()
                    .map(Nested::build)
                    .collect::<Result<Vec<_>, _>>()?;
                builder.set_vector(items)
            }
        }
    }
}

impl Nested {
    /// Builds the table to nest, as its builder or its bytes
    pub fn build(&self) -> Result<NestedTable<'_>, BuildError> {
        let builder = self.table.build()?;

        Ok(match self.from {
            Source::Builder => builder.into(),
            Source::Bytes => builder.to_bytes()?.into(),
        })
    }
}
