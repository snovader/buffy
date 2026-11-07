//! A checked schema, which the generators read: every type resolved, every
//! name unique and every layout within what tafla can write. Tables and
//! structs nest in place at most 64 deep, so the generators and the methods
//! here can follow them by recursion.

#[derive(Debug)]
pub struct Schema {
    /// In declaration order, which the generators keep
    pub definitions: Vec<Definition>,
    /// Whether a vector or an array of structs holds each definition, by its
    /// index, found once rather than by a search of every field per definition
    items: Vec<bool>,
    /// The alignment of each message and table, by its index, and 1 for each
    /// struct, whose layout has its alignment
    table_alignments: Vec<u8>,
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
    /// Scalars at fixed offsets, without a header or padding, read and
    /// written on their own, as a field of a table or in an array
    Struct(StructLayout),
}

/// The bytes that a struct takes, which the checker measures once
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructLayout {
    /// The byte length of the struct: the sizes of its scalars in all
    pub size: usize,
    /// The alignment of an array of the struct: its largest scalar's size
    pub alignment: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub sized: bool,
    pub version: Option<u32>,
    pub offset_type: OffsetType,
}

/// The offset type of a message, which its `offset_type` attribute names
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetType {
    Uint8,
    Uint16,
    Uint32,
}

impl OffsetType {
    /// The offset type of a schema's `offset_type` attribute, or `None` for
    /// any other name
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "uint8" => Some(Self::Uint8),
            "uint16" => Some(Self::Uint16),
            "uint32" => Some(Self::Uint32),
            _ => None,
        }
    }

    /// The type's name, as schemas spell it: `uint8`, `uint16` or `uint32`
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uint8 => "uint8",
            Self::Uint16 => "uint16",
            Self::Uint32 => "uint32",
        }
    }

    pub const fn max_value(self) -> u64 {
        match self {
            Self::Uint8 => 0xff,
            Self::Uint16 => 0xffff,
            Self::Uint32 => 0xffff_ffff,
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
    /// A struct, in place, by its index in [`Schema::definitions`]
    Struct(usize),
    /// An array of structs, by the structs' index in [`Schema::definitions`]
    StructArray(usize),
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
    /// The schema of checked definitions, whose fields' indices are their
    /// positions in `definitions`. It finds what the generators ask of each
    /// definition once, so that generating code takes time in proportion to
    /// the schema.
    pub fn new(definitions: Vec<Definition>) -> Self {
        let mut items = vec![false; definitions.len()];
        for field in definitions.iter().flat_map(|definition| &definition.fields) {
            if let FieldType::Vector(item) | FieldType::StructArray(item) = field.field_type {
                items[item] = true;
            }
        }

        let mut alignments = vec![None; definitions.len()];
        let table_alignments = (0..definitions.len())
            .map(|index| {
                if definitions[index].is_struct() {
                    1
                } else {
                    alignment_within(&definitions, index, &mut alignments)
                }
            })
            .collect();

        Self {
            definitions,
            items,
            table_alignments,
        }
    }

    pub fn definition(&self, index: usize) -> &Definition {
        &self.definitions[index]
    }

    /// Whether a vector or an array of structs holds the definition at
    /// `index`, so that the generators write a view of its items
    pub fn is_item(&self, index: usize) -> bool {
        self.items[index]
    }

    /// The scalars of a struct in order, those of the structs in it in their
    /// place
    pub fn struct_scalars(&self, definition: &Definition) -> Vec<Scalar> {
        let mut scalars = Vec::new();
        self.push_scalars(definition, &mut scalars);
        scalars
    }

    fn push_scalars(&self, definition: &Definition, scalars: &mut Vec<Scalar>) {
        for field in &definition.fields {
            match field.field_type {
                FieldType::Scalar(scalar) => scalars.push(scalar),
                FieldType::Struct(index) => self.push_scalars(self.definition(index), scalars),
                _ => unreachable!("structs hold scalars and structs only"),
            }
        }
    }

    /// The alignment that a parent pads the nested table at `index` to, so
    /// that its arrays are aligned from the parent's start too: the largest
    /// alignment of its arrays, of scalars and of structs, and of the tables
    /// nested in it, or 1 if it has none. A vector's items aren't padded, so
    /// they don't count.
    pub fn table_alignment(&self, index: usize) -> u8 {
        self.table_alignments[index]
    }
}

/// The alignment of the message or table at `index`, as
/// [`Schema::table_alignment()`] describes it, with those of the tables
/// nested in it by index in `alignments` once they're known, since a table
/// may nest another in many fields, and that one the next, and so on
fn alignment_within(definitions: &[Definition], index: usize, alignments: &mut [Option<u8>]) -> u8 {
    if let Some(alignment) = alignments[index] {
        return alignment;
    }

    let mut largest = 1;
    for field in &definitions[index].fields {
        let alignment = match field.field_type {
            FieldType::Array(scalar) => scalar.size(),
            FieldType::StructArray(item) => definitions[item].struct_layout().alignment,
            // Tables nest in place only acyclically and at most 64 deep,
            // which the checker ensures, so this ends soon
            FieldType::Table(nested) => alignment_within(definitions, nested, alignments),
            FieldType::Scalar(_)
            | FieldType::String
            | FieldType::Struct(_)
            | FieldType::Vector(_) => 1,
        };
        largest = largest.max(alignment);
    }

    alignments[index] = Some(largest);
    largest
}

impl Definition {
    pub const fn layout(&self) -> Option<Layout> {
        match self.kind {
            Kind::Message(layout) => Some(layout),
            Kind::Table | Kind::Struct(_) => None,
        }
    }

    pub const fn is_struct(&self) -> bool {
        matches!(self.kind, Kind::Struct(_))
    }

    /// The size and alignment of a struct
    ///
    /// # Panics
    ///
    /// If it isn't a struct.
    pub fn struct_layout(&self) -> StructLayout {
        match self.kind {
            Kind::Struct(layout) => layout,
            Kind::Message(_) | Kind::Table => unreachable!("only structs have a struct layout"),
        }
    }
}
