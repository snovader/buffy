//! Rust for `tafla`. Messages and tables are `Copy` views that read
//! fields lazily, borrowing strings and arrays from the bytes; they're written
//! from structs of their fields, with tables nested from their builders.
//! Structs are plain structs read and written at fixed offsets.

use std::fmt::Display;

use super::writer::{CodeWriter, Style, call, text};
use super::{GeneratedFile, Options};
use crate::model::{Definition, FieldType, Kind, Layout, OffsetSize, Scalar, Schema};

/// The runtime's crate, unless [`Options::runtime`] names another
const RUNTIME: &str = "tafla";

const STYLE: Style = Style {
    width: 100,
    indent: "    ",
    trailing_comma: true,
    hug_arguments: false,
    max_arguments_width: Some(FN_CALL_WIDTH),
};

/// rustfmt's `chain_width`, past which it puts each call of a chain on a line
const CHAIN_WIDTH: usize = 60;

/// rustfmt's `fn_call_width`, past which it breaks a call's arguments
const FN_CALL_WIDTH: usize = 60;

/// The widest `use` statement with a list that rustfmt keeps on one line, two
/// columns less than its `max_width`
const USE_LIST_WIDTH: usize = 98;

/// Strict and reserved keywords, which names take as raw identifiers
const KEYWORDS: [&str; 48] = [
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return",
    "static", "struct", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
    "virtual", "where", "while", "yield",
];

/// Keywords that can't be raw identifiers, which names take a `_` after
const PATH_KEYWORDS: [&str; 3] = ["crate", "self", "super"];

/// The method that messages and tables define beside their getters, which
/// names take a `_` after too. Fields and methods don't share names, so the
/// other methods don't clash with fields.
const MEMBERS: [&str; 1] = ["new"];

pub fn generate(
    schema: &Schema,
    stem: &str,
    header: &str,
    options: &Options,
) -> Vec<GeneratedFile> {
    let runtime = options.runtime.as_deref().unwrap_or(RUNTIME);
    let generator = Generator {
        schema,
        borrows: borrowing(schema),
    };

    let mut writer = CodeWriter::new(STYLE.indent);
    writer.line(format!("// {header}"));
    writer.blank();
    generator.imports(&mut writer, runtime);

    for (index, definition) in schema.definitions.iter().enumerate() {
        writer.blank();
        match definition.kind {
            Kind::Message(layout) => generator.message(&mut writer, definition, layout),
            Kind::Table => generator.table(&mut writer, definition),
            Kind::Struct => generator.structure(&mut writer, definition),
        }
        if definition.kind != Kind::Struct {
            writer.blank();
            generator.fields_struct(&mut writer, index);
        }
    }

    vec![GeneratedFile {
        path: format!("{stem}.rs"),
        contents: writer.finish(),
    }]
}

struct Generator<'a> {
    schema: &'a Schema,
    /// Whether each definition's fields struct borrows strings or arrays, by
    /// its index
    borrows: Vec<bool>,
}

impl Generator<'_> {
    fn imports(&self, writer: &mut CodeWriter, runtime: &str) {
        let definitions = &self.schema.definitions;
        // A schema without definitions imports nothing
        if definitions.is_empty() {
            return;
        }

        let has_tables = definitions
            .iter()
            .any(|definition| definition.kind != Kind::Struct);
        let has_array_view = definitions
            .iter()
            .flat_map(|definition| &definition.fields)
            .any(|field| {
                matches!(field.field_type, FieldType::Array(scalar) if scalar != Scalar::Uint8)
            });
        let has_offset_size = definitions.iter().any(|definition| {
            definition
                .layout()
                .is_some_and(|layout| layout.offset_size != OffsetSize::U32)
        });
        let has_structs = definitions
            .iter()
            .any(|definition| definition.kind == Kind::Struct);

        let mut names = Vec::new();
        if has_structs {
            names.push("Alignment");
        }
        if has_array_view {
            names.push("ArrayView");
        }
        if has_tables {
            names.push("BuildError");
        }
        if has_tables {
            names.push("ByteBuffer");
        }
        names.push("DecodeError");
        if has_offset_size {
            names.push("OffsetSize");
        }
        if has_tables {
            names.extend(["TableBuilder", "TableLayout"]);
        }

        let statement = format!("use {runtime}::{{{}}};", names.join(", "));
        if statement.len() <= USE_LIST_WIDTH {
            writer.line(statement);
        } else {
            writer.open(format!("use {runtime}::{{"));
            writer.line(format!("{},", names.join(", ")));
            writer.close("};");
        }
    }

    fn message(&self, writer: &mut CodeWriter, definition: &Definition, layout: Layout) {
        let name = &definition.name;
        let versioned = layout.version.is_some();
        let generics = if versioned { "<true>" } else { "" };

        rust_doc(writer, &definition.docs);
        writer.line("#[derive(Clone, Copy, Debug)]");
        writer.open(format!("pub struct {name}<'a> {{"));
        if versioned {
            writer.line("buffer: ByteBuffer<'a, true>,");
        } else {
            writer.line("buffer: ByteBuffer<'a>,");
        }
        writer.close("}");
        writer.blank();

        writer.open(format!("impl<'a> {name}<'a> {{"));
        if let Some(version) = layout.version {
            rust_doc(
                writer,
                &[
                    "The version of the format, which the header carries and [`Self::new`] checks"
                        .to_owned(),
                ],
            );
            writer.line(format!(
                "pub const FORMAT_VERSION: u32 = {};",
                integer(&version)
            ));
            writer.blank();
        }
        layout_constant(writer, layout, generics);
        writer.blank();

        if versioned {
            let mut docs = vec![format!("Reads {} from `bytes`", article(name))];
            docs.extend(errors_doc("if `bytes` are in another format version"));
            rust_doc(writer, &docs);
            writer.open("pub fn new(bytes: &'a [u8]) -> Result<Self, DecodeError> {");
            writer.line("let buffer = ByteBuffer::new(bytes, Self::LAYOUT);");
            writer.blank();
            writer.line("let version = buffer.version()?;");
            writer.open("if version != Self::FORMAT_VERSION {");
            writer.open("return Err(DecodeError::UnsupportedVersion {");
            writer.line("version,");
            writer.line("supported: Self::FORMAT_VERSION,");
            writer.close("});");
            writer.close("}");
            writer.blank();
            writer.line("Ok(Self { buffer })");
            writer.close("}");
        } else {
            rust_doc(writer, &[format!("Reads {} from `bytes`", article(name))]);
            writer.open("pub const fn new(bytes: &'a [u8]) -> Self {");
            writer.open("Self {");
            writer.line("buffer: ByteBuffer::new(bytes, Self::LAYOUT),");
            writer.close("}");
            writer.close("}");
        }

        self.getters(writer, definition, true);
        writer.close("}");
    }

    fn table(&self, writer: &mut CodeWriter, definition: &Definition) {
        let name = &definition.name;

        rust_doc(writer, &definition.docs);
        writer.line("#[derive(Clone, Copy, Debug)]");
        writer.open(format!("pub struct {name}<'a> {{"));
        writer.line("buffer: ByteBuffer<'a>,");
        writer.line("position: usize,");
        writer.close("}");
        writer.blank();

        writer.open(format!("impl<'a> {name}<'a> {{"));
        rust_doc(
            writer,
            &[
                "Reads the table at `position` in `bytes`, as its parent's getter finds it"
                    .to_owned(),
            ],
        );
        writer.open("const fn new(bytes: &'a [u8], position: usize) -> Self {");
        writer.open("Self {");
        writer.line("buffer: ByteBuffer::new(bytes, TableLayout::DEFAULT),");
        writer.line("position,");
        writer.close("}");
        writer.close("}");

        self.getters(writer, definition, false);
        writer.close("}");
    }

    #[allow(clippy::too_many_lines)]
    fn structure(&self, writer: &mut CodeWriter, definition: &Definition) {
        let name = &definition.name;
        let size = self.schema.struct_size(definition);
        // Each field, its range of bytes in the struct, and the type that
        // reads it from them
        let mut offset = 0;
        let fields: Vec<(String, usize, usize, String)> = definition
            .fields
            .iter()
            .map(|field| {
                let (end, read) = match field.field_type {
                    FieldType::Scalar(scalar) => {
                        (offset + usize::from(scalar.size()), scalar_type(scalar))
                    }
                    FieldType::Struct(index) => {
                        let inner = self.schema.definition(index);
                        (offset + self.schema.struct_size(inner), inner.name.as_str())
                    }
                    _ => unreachable!("structs hold scalars and structs only"),
                };
                let field = (identifier(&field.name), offset, end, read.to_owned());
                offset = end;
                field
            })
            .collect();

        rust_doc(writer, &definition.docs);
        writer.line("#[derive(Clone, Copy, Debug, PartialEq)]");
        writer.open(format!("pub struct {name} {{"));
        for field in &definition.fields {
            rust_doc(writer, &field.docs);
            writer.line(format!(
                "pub {}: {},",
                identifier(&field.name),
                self.input_type(field.field_type)
            ));
        }
        writer.close("}");
        writer.blank();

        writer.open(format!("impl {name} {{"));
        rust_doc(writer, &[format!("The byte length of {}", article(name))]);
        writer.line(format!("pub const SIZE: usize = {};", integer(&size)));
        writer.blank();
        rust_doc(
            writer,
            &[format!(
                "The alignment of an array of {}: its largest scalar's size",
                article(name)
            )],
        );
        writer.line(format!(
            "pub const ALIGNMENT: Alignment = Alignment::{};",
            alignment_variant(self.schema.struct_alignment(definition))
        ));
        writer.blank();

        let mut docs = vec![format!("Reads {} from the start of `bytes`", article(name))];
        docs.extend(errors_doc("if `bytes` are shorter than [`Self::SIZE`]"));
        rust_doc(writer, &docs);
        writer.open("pub fn read(bytes: &[u8]) -> Result<Self, DecodeError> {");
        writer.open("match bytes.first_chunk() {");
        writer.line("Some(bytes) => Ok(Self::from_bytes(bytes)),");
        writer.open("None => Err(DecodeError::OutOfRange {");
        writer.line("position: 0,");
        writer.line(format!("byte_length: {},", integer(&size)));
        writer.line("available: bytes.len(),");
        writer.close("}),");
        writer.close("}");
        writer.close("}");
        writer.blank();

        rust_doc(
            writer,
            &[format!(
                "Reads {} from its bytes, such as those of an array of them",
                article(name)
            )],
        );
        writer.line("#[must_use]");
        // By reference, as an array of structs iterates over them, however
        // small
        if size <= 8 {
            writer.line("#[allow(clippy::trivially_copy_pass_by_ref)]");
        }
        writer.open(format!(
            "pub fn from_bytes(bytes: &[u8; {}]) -> Self {{",
            integer(&size)
        ));
        self.struct_literal(writer, "Self {", definition, 0);
        writer.close("}");
        writer.blank();

        writer.line("#[must_use]");
        writer.open(format!(
            "pub fn to_bytes(self) -> [u8; {}] {{",
            integer(&size)
        ));
        writer.line("let mut bytes = [0; Self::SIZE];");
        for (field, start, end, read) in &fields {
            let bytes = if read.starts_with(char::is_uppercase) {
                format!("self.{field}.to_bytes()")
            } else {
                format!("self.{field}.to_le_bytes()")
            };
            writer.line(format!(
                "bytes[{}..{}].copy_from_slice(&{bytes});",
                integer(start),
                integer(end)
            ));
        }
        writer.blank();
        writer.line("bytes");
        writer.close("}");
        writer.blank();

        rust_doc(
            writer,
            &[format!(
                "Writes [`{name}`]s one after another, as an array of them holds them"
            )],
        );
        writer.line("#[must_use]");
        writer.open("pub fn array_to_bytes(items: &[Self]) -> Vec<u8> {");
        writer.line("let mut bytes = Vec::with_capacity(items.len() * Self::SIZE);");
        writer.open("for item in items {");
        writer.line("bytes.extend_from_slice(&item.to_bytes());");
        writer.close("}");
        writer.blank();
        writer.line("bytes");
        writer.close("}");
        writer.close("}");
    }

    /// A struct of the scalars at `offset` in `bytes`, with a struct within it
    /// as a literal of its own, since an array only defaults up to 32 bytes.
    /// Each range of a scalar lies within the array, so converting it never
    /// fails.
    fn struct_literal(
        &self,
        writer: &mut CodeWriter,
        opening: &str,
        definition: &Definition,
        offset: usize,
    ) {
        writer.open(opening);
        let mut start = offset;
        for field in &definition.fields {
            let name = identifier(&field.name);
            match field.field_type {
                FieldType::Scalar(scalar) => {
                    let end = start + usize::from(scalar.size());
                    writer.line(format!(
                        "{name}: {}::from_le_bytes(bytes[{}..{}].try_into().unwrap_or_default()),",
                        scalar_type(scalar),
                        integer(&start),
                        integer(&end)
                    ));
                    start = end;
                }
                FieldType::Struct(index) => {
                    let inner = self.schema.definition(index);
                    self.struct_literal(
                        writer,
                        &format!("{name}: {} {{", inner.name),
                        inner,
                        start,
                    );
                    start += self.schema.struct_size(inner);
                }
                _ => unreachable!("structs hold scalars and structs only"),
            }
        }
        if opening == "Self {" {
            writer.close("}");
        } else {
            writer.close("},");
        }
    }

    /// The struct of the values that write a definition, with the method that
    /// writes them
    fn fields_struct(&self, writer: &mut CodeWriter, index: usize) {
        let definition = self.schema.definition(index);
        let name = &definition.name;
        let borrows = self.borrows[index];
        let generics = if borrows { "<'a>" } else { "" };

        rust_doc(
            writer,
            &[format!("The fields of {}, to write", article(name))],
        );
        writer.line("#[derive(Clone, Debug)]");
        writer.open(format!("pub struct {name}Fields{generics} {{"));
        for field in &definition.fields {
            rust_doc(writer, &field.docs);
            writer.line(format!(
                "pub {}: {},",
                identifier(&field.name),
                self.input_type(field.field_type)
            ));
        }
        writer.close("}");
        writer.blank();

        if let Kind::Message(layout) = definition.kind {
            if borrows {
                writer.open(format!("impl {name}Fields<'_> {{"));
            } else {
                writer.open(format!("impl {name}Fields {{"));
            }
            let mut docs = vec![format!("Writes {} with every field", article(name))];
            docs.extend(error_doc(
                "a [`BuildError`]",
                &format!(
                    "if it's longer than its {}-bit offsets address",
                    layout.offset_size.bytes() * 8
                ),
            ));
            rust_doc(writer, &docs);
            writer.open("pub fn serialize(&self) -> Result<Vec<u8>, BuildError> {");
            let (constructor, arguments) = if layout.version.is_some() {
                let arguments = vec![
                    text(integer(&definition.fields.len())),
                    text(format!("{name}::LAYOUT")),
                    text(format!("{name}::FORMAT_VERSION")),
                ];
                ("TableBuilder::versioned", arguments)
            } else {
                let arguments = vec![
                    text(integer(&definition.fields.len())),
                    text(format!("{name}::LAYOUT")),
                ];
                ("TableBuilder::new", arguments)
            };
            writer.statement(
                &STYLE,
                "let mut builder = ",
                &call(constructor, arguments),
                "?;",
            );
            Self::setters(self.schema, writer, definition);
            writer.blank();
            writer.line("builder.to_bytes()");
        } else {
            writer.open(format!("impl{generics} {name}Fields{generics} {{"));
            let mut docs = vec![
                "Returns a builder with every field set, which a parent writes straight into"
                    .to_owned(),
                "its own bytes".to_owned(),
            ];
            docs.extend(error_doc(
                "a [`BuildError`]",
                "if the table is longer than its 32-bit offsets address",
            ));
            rust_doc(writer, &docs);
            let lifetime = if borrows { "'a" } else { "'static" };
            writer.open(format!(
                "pub fn builder(&self) -> Result<TableBuilder<{lifetime}>, BuildError> {{"
            ));
            let arguments = vec![
                text(integer(&definition.fields.len())),
                text("TableLayout::DEFAULT"),
            ];
            writer.statement(
                &STYLE,
                "let mut builder = ",
                &call("TableBuilder::new", arguments),
                "?;",
            );
            Self::setters(self.schema, writer, definition);
            writer.blank();
            writer.line("Ok(builder)");
        }
        writer.close("}");
        writer.close("}");
    }

    fn setters(schema: &Schema, writer: &mut CodeWriter, definition: &Definition) {
        for field in &definition.fields {
            let value = format!("self.{}", identifier(&field.name));
            let (method, argument) = match field.field_type {
                FieldType::Scalar(scalar) => (format!("set_{}", scalar_type(scalar)), value),
                FieldType::Array(scalar) => (format!("set_{}_array", scalar_type(scalar)), value),
                FieldType::String => ("set_str".to_owned(), value),
                FieldType::Struct(_) => (
                    "set_struct".to_owned(),
                    format!("{value}.to_bytes().to_vec()"),
                ),
                FieldType::StructArray(index) => {
                    let structure = &schema.definition(index).name;
                    writer.statement(
                        &STYLE,
                        "",
                        &call(
                            "builder.set_struct_array",
                            vec![
                                call(format!("{structure}::array_to_bytes"), vec![text(value)]),
                                text(format!("{structure}::SIZE")),
                                text(format!("{structure}::ALIGNMENT")),
                            ],
                        ),
                        "?;",
                    );
                    continue;
                }
                FieldType::Table(_) => ("set_table".to_owned(), format!("{value}.builder()?")),
                FieldType::Vector(_) => {
                    // Collecting the fallible builders into a `Result` would
                    // grow the vector from nothing, since its size hint
                    // starts at zero. Reserving it took 7% less time to write
                    // 100 items, with 5 fewer allocations.
                    writer.line(format!(
                        "let mut items = Vec::with_capacity({value}.len());"
                    ));
                    writer.open(format!("for item in {value} {{"));
                    writer.line("items.push(item.builder()?);");
                    writer.close("}");
                    writer.line("builder.set_vector(items)?;");
                    continue;
                }
            };
            writer.statement(
                &STYLE,
                "",
                &call(format!("builder.{method}"), vec![text(argument)]),
                "?;",
            );
        }
    }

    fn getters(&self, writer: &mut CodeWriter, definition: &Definition, is_root: bool) {
        for (index, field) in definition.fields.iter().enumerate() {
            let offset = if is_root {
                format!("self.buffer.root_offset({})?", integer(&index))
            } else {
                format!(
                    "self.buffer.field_offset(self.position, {})?",
                    integer(&index)
                )
            };

            let mut docs = field.docs.clone();
            docs.extend(errors_doc("if the field lies outside the bytes"));
            if field.docs.is_empty() {
                docs.remove(0);
            }

            writer.blank();
            rust_doc(writer, &docs);
            let name = identifier(&field.name);
            let result = format!("Result<{}, DecodeError>", self.value_type(field.field_type));
            let signature = format!("pub fn {name}(&self) -> {result} {{");
            // rustfmt puts the parameters of a signature wider than its
            // `max_width` on lines of their own
            if STYLE.indent.len() + signature.len() <= STYLE.width {
                writer.open(signature);
            } else {
                writer.line(format!("pub fn {name}("));
                writer.indent();
                writer.line("&self,");
                writer.dedent();
                writer.open(format!(") -> {result} {{"));
            }
            writer.line(format!("let position = {offset};"));
            match field.field_type {
                FieldType::Scalar(scalar) => {
                    writer.line(format!("self.buffer.get_{}(position)", scalar_type(scalar)));
                }
                FieldType::Array(scalar) => {
                    writer.line(format!(
                        "self.buffer.get_{}_array(position)",
                        scalar_type(scalar)
                    ));
                }
                FieldType::String => writer.line("self.buffer.get_str(position)"),
                FieldType::Table(table) => {
                    let table = &self.schema.definition(table).name;
                    let read = format!("{table}::new(self.buffer.bytes(), position)");

                    // rustfmt breaks a call wider than its `fn_call_width`
                    // as `Ok`'s argument
                    if read.len() <= FN_CALL_WIDTH {
                        writer.line(format!("Ok({read})"));
                    } else {
                        writer.line(format!("Ok({table}::new("));
                        writer.indent();
                        writer.line("self.buffer.bytes(),");
                        writer.line("position,");
                        writer.dedent();
                        writer.line("))");
                    }
                }
                FieldType::Struct(index) => {
                    let structure = &self.schema.definition(index).name;
                    writer.line(format!(
                        "let bytes = self.buffer.get_struct(position, {structure}::SIZE)?;"
                    ));
                    writer.blank();
                    writer.line(format!("{structure}::read(bytes)"));
                }
                FieldType::StructArray(index) => {
                    let structure = &self.schema.definition(index).name;
                    writer.statement(
                        &STYLE,
                        "let bytes = ",
                        &call(
                            "self.buffer.get_struct_array",
                            vec![text("position"), text(format!("{structure}::SIZE"))],
                        ),
                        "?;",
                    );
                    // The bytes are a whole number of structs
                    writer.line(format!(
                        "let (structs, _) = bytes.as_chunks::<{{ {structure}::SIZE }}>();"
                    ));
                    writer.blank();
                    writer.line(format!("Ok(structs.iter().map({structure}::from_bytes))"));
                }
                FieldType::Vector(table) => {
                    let table = &self.schema.definition(table).name;
                    writer.line("let vector = self.buffer.get_vector(position)?;");
                    // Collecting into a `Result` would grow the items from
                    // nothing, since its size hint starts at zero
                    writer.line("let mut items = Vec::with_capacity(vector.len());");
                    writer.open("for item in vector {");
                    // Each item is a table of its own bytes
                    writer.line(format!("items.push({table}::new(item?, 0));"));
                    writer.close("}");
                    writer.blank();
                    writer.line("Ok(items)");
                }
            }
            writer.close("}");
        }
    }

    fn value_type(&self, field_type: FieldType) -> String {
        match field_type {
            FieldType::Scalar(scalar) => scalar_type(scalar).to_owned(),
            FieldType::Array(Scalar::Uint8) => "&'a [u8]".to_owned(),
            FieldType::Array(scalar) => format!("ArrayView<'a, {}>", scalar_type(scalar)),
            FieldType::String => "&'a str".to_owned(),
            FieldType::Struct(index) => self.schema.definition(index).name.clone(),
            FieldType::StructArray(index) => format!(
                "impl ExactSizeIterator<Item = {}> + use<'a>",
                self.schema.definition(index).name
            ),
            FieldType::Table(index) => format!("{}<'a>", self.schema.definition(index).name),
            FieldType::Vector(index) => format!("Vec<{}<'a>>", self.schema.definition(index).name),
        }
    }

    fn input_type(&self, field_type: FieldType) -> String {
        let fields = |index: usize| {
            let definition = self.schema.definition(index);
            let generics = if self.borrows[index] { "<'a>" } else { "" };
            format!("{}Fields{generics}", definition.name)
        };

        match field_type {
            FieldType::Scalar(scalar) => scalar_type(scalar).to_owned(),
            FieldType::Array(scalar) => format!("&'a [{}]", scalar_type(scalar)),
            FieldType::String => "&'a str".to_owned(),
            FieldType::Struct(index) => self.schema.definition(index).name.clone(),
            FieldType::StructArray(index) => {
                format!("&'a [{}]", self.schema.definition(index).name)
            }
            FieldType::Table(index) => fields(index),
            FieldType::Vector(index) => format!("&'a [{}]", fields(index)),
        }
    }
}

/// Whether each definition's fields struct borrows strings or arrays, which
/// it does if it holds any, or nests a table that does. Each definition is
/// visited once, from a stack rather than by recursion, so that neither
/// tables nested in many fields nor long chains of them take long.
fn borrowing(schema: &Schema) -> Vec<bool> {
    let definitions = &schema.definitions;
    let mut borrows: Vec<Option<bool>> = vec![None; definitions.len()];
    let mut stack = Vec::new();

    for root in 0..definitions.len() {
        stack.push(root);
        while let Some(&index) = stack.last() {
            if borrows[index].is_some() {
                stack.pop();
                continue;
            }

            // Nested tables come first, and the checker rejects cycles of them
            let unknown: Vec<usize> = nested_tables(&definitions[index])
                .filter(|&nested| borrows[nested].is_none())
                .collect();
            if !unknown.is_empty() {
                stack.extend(unknown);
                continue;
            }

            let borrowing = definitions[index]
                .fields
                .iter()
                .any(|field| match field.field_type {
                    FieldType::Scalar(_) | FieldType::Struct(_) => false,
                    FieldType::Array(_)
                    | FieldType::String
                    | FieldType::StructArray(_)
                    | FieldType::Vector(_) => true,
                    FieldType::Table(nested) => borrows[nested] == Some(true),
                });
            borrows[index] = Some(borrowing);
            stack.pop();
        }
    }

    borrows.into_iter().map(Option::unwrap_or_default).collect()
}

fn nested_tables(definition: &Definition) -> impl Iterator<Item = usize> + '_ {
    definition
        .fields
        .iter()
        .filter_map(|field| match field.field_type {
            FieldType::Table(index) => Some(index),
            _ => None,
        })
}

/// The layout constant, broken as rustfmt breaks a long chain
fn layout_constant(writer: &mut CodeWriter, layout: Layout, generics: &str) {
    let mut calls = Vec::new();
    if layout.sized {
        calls.push(".sized()".to_owned());
    }
    if layout.version.is_some() {
        calls.push(".versioned()".to_owned());
    }
    match layout.offset_size {
        OffsetSize::U8 => calls.push(".with_offset_size(OffsetSize::U8)".to_owned()),
        OffsetSize::U16 => calls.push(".with_offset_size(OffsetSize::U16)".to_owned()),
        OffsetSize::U32 => {}
    }

    let prefix = format!("const LAYOUT: TableLayout{generics} = ");
    let chain = format!("TableLayout::DEFAULT{}", calls.concat());
    if chain.len() <= CHAIN_WIDTH {
        writer.line(format!("{prefix}{chain};"));
        return;
    }

    writer.line(format!("{prefix}TableLayout::DEFAULT"));
    writer.indent();
    let last = calls.len() - 1;
    for (index, call) in calls.iter().enumerate() {
        writer.line(if index == last {
            format!("{call};")
        } else {
            call.clone()
        });
    }
    writer.dedent();
}

fn rust_doc(writer: &mut CodeWriter, docs: &[String]) {
    for line in docs {
        writer.line(format!("/// {line}").trim_end());
    }
}

fn errors_doc(condition: &str) -> Vec<String> {
    error_doc("a [`DecodeError`]", condition)
}

fn error_doc(error: &str, condition: &str) -> Vec<String> {
    vec![
        String::new(),
        "# Errors".to_owned(),
        String::new(),
        format!("Returns {error} {condition}."),
    ]
}

fn identifier(name: &str) -> String {
    if KEYWORDS.contains(&name) {
        format!("r#{name}")
    } else if PATH_KEYWORDS.contains(&name) || MEMBERS.contains(&name) {
        format!("{name}_")
    } else {
        name.to_owned()
    }
}

/// An integer literal, with its digits in groups of three when it has more
/// than four, as clippy's `unreadable_literal` asks
fn integer(value: &impl Display) -> String {
    let digits = value.to_string();
    if digits.len() <= 4 {
        return digits;
    }

    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('_');
        }
        grouped.push(digit);
    }
    grouped
}

fn article(name: &str) -> String {
    let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    format!("{article} [`{name}`]")
}

/// The Rust type, which also names the crate's getters and setters
const fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Int8 => "i8",
        Scalar::Uint8 => "u8",
        Scalar::Int16 => "i16",
        Scalar::Uint16 => "u16",
        Scalar::Int32 => "i32",
        Scalar::Uint32 => "u32",
        Scalar::Int64 => "i64",
        Scalar::Uint64 => "u64",
        Scalar::Float32 => "f32",
        Scalar::Float64 => "f64",
    }
}

/// The variant of the runtime's `Alignment` of `bytes`
fn alignment_variant(bytes: u8) -> &'static str {
    match bytes {
        1 => "One",
        2 => "Two",
        4 => "Four",
        _ => "Eight",
    }
}
