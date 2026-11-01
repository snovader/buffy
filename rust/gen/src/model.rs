//! A checked schema, which the generators read: every type resolved, every
//! name unique and every layout within what buffy can write.

#[derive(Debug)]
pub struct Schema {
    /// In declaration order, which the generators keep
    pub definitions: Vec<Definition>,
}

#[derive(Debug)]
pub struct Definition {
    /// `PascalCase`
    pub name: String,
    pub docs: Vec<String>,
    pub kind: Kind,
    pub fields: Vec<Field>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A table that's sent on its own: it's read from bytes, with its layout
    Message(Layout),
    /// A table nested in a message or another table
    Table,
    /// Scalars at fixed offsets, without a header or padding
    Struct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub sized: bool,
    pub version: Option<u32>,
    pub offset_size: OffsetSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetSize {
    U8,
    U16,
    U32,
}

impl OffsetSize {
    pub const fn bytes(self) -> u8 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::U32 => 4,
        }
    }

    pub const fn max_value(self) -> u64 {
        match self {
            Self::U8 => 0xff,
            Self::U16 => 0xffff,
            Self::U32 => 0xffff_ffff,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
pub struct Field {
    /// `snake_case`
    pub name: String,
    pub docs: Vec<String>,
    pub field_type: FieldType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldType {
    Scalar(Scalar),
    Array(Scalar),
    String,
    /// A nested table, by its index in [`Schema::definitions`]
    Table(usize),
    /// A vector of tables, by the tables' index in [`Schema::definitions`]
    Vector(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Int64,
    Uint64,
    Float32,
    Float64,
}

impl Scalar {
    pub const ALL: [Self; 10] = [
        Self::Int8,
        Self::Uint8,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Int64,
        Self::Uint64,
        Self::Float32,
        Self::Float64,
    ];

    /// The name in schemas, which every runtime's getters and setters use too
    pub const fn name(self) -> &'static str {
        match self {
            Self::Int8 => "int8",
            Self::Uint8 => "uint8",
            Self::Int16 => "int16",
            Self::Uint16 => "uint16",
            Self::Int32 => "int32",
            Self::Uint32 => "uint32",
            Self::Int64 => "int64",
            Self::Uint64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
        }
    }

    pub const fn size(self) -> u8 {
        match self {
            Self::Int8 | Self::Uint8 => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Int64 | Self::Uint64 | Self::Float64 => 8,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|scalar| scalar.name() == name)
    }
}

impl Schema {
    pub fn definition(&self, index: usize) -> &Definition {
        &self.definitions[index]
    }
}

impl Definition {
    pub const fn layout(&self) -> Option<Layout> {
        match self.kind {
            Kind::Message(layout) => Some(layout),
            Kind::Table | Kind::Struct => None,
        }
    }

    /// The byte length of a struct
    pub fn struct_size(&self) -> usize {
        self.fields
            .iter()
            .map(|field| match field.field_type {
                FieldType::Scalar(scalar) => usize::from(scalar.size()),
                _ => unreachable!("structs hold scalars only"),
            })
            .sum()
    }
}
