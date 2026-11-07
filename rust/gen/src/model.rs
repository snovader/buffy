//! A checked schema, which the generators read: every type resolved, every
//! name unique and every layout within what tafla can write. Tables and
//! structs nest in place at most 64 deep, so the generators and the methods
//! here can follow them by recursion.

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
    pub fn definition(&self, index: usize) -> &Definition {
        &self.definitions[index]
    }

    /// Whether a vector or an array of structs holds the definition at
    /// `index`, so that the generators write a view of its items
    pub fn is_item(&self, index: usize) -> bool {
        self.definitions
            .iter()
            .flat_map(|definition| &definition.fields)
            .any(|field| {
                matches!(
                    field.field_type,
                    FieldType::Vector(item) | FieldType::StructArray(item) if item == index
                )
            })
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

    /// The alignment that a parent pads a nested table to, so that its
    /// arrays are aligned from the parent's start too: the largest alignment
    /// of its arrays, of scalars and of structs, and of the tables nested in
    /// it, or 1 if it has none. A vector's items aren't padded, so they don't
    /// count.
    pub fn table_alignment(&self, definition: &Definition) -> u8 {
        let mut alignments = vec![None; self.definitions.len()];
        self.alignment_within(definition, &mut alignments)
    }

    /// A table's alignment, with those of the tables nested in it by index
    /// in `alignments` once they're known, since a table may nest another in
    /// many fields, and that one the next, and so on
    fn alignment_within(&self, definition: &Definition, alignments: &mut [Option<u8>]) -> u8 {
        let mut largest = 1;
        for field in &definition.fields {
            let alignment = match field.field_type {
                FieldType::Array(scalar) => scalar.size(),
                FieldType::StructArray(index) => self.definition(index).struct_layout().alignment,
                // Tables nest in place only acyclically and at most 64 deep,
                // which the checker ensures, so this ends soon
                FieldType::Table(index) => {
                    if let Some(alignment) = alignments[index] {
                        alignment
                    } else {
                        let alignment = self.alignment_within(self.definition(index), alignments);
                        alignments[index] = Some(alignment);
                        alignment
                    }
                }
                FieldType::Scalar(_)
                | FieldType::String
                | FieldType::Struct(_)
                | FieldType::Vector(_) => 1,
            };
            largest = largest.max(alignment);
        }

        largest
    }
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
