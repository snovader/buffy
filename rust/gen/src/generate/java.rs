//! Java for `io.github.snovader.tafla`, one file per class. Messages
//! and tables read fields lazily from their bytes and are written from
//! records of their fields, with tables nested from their builders. Structs
//! are records, read and written in place with `VarHandle`s.

use std::fmt::Write;

use super::java_package::KEYWORDS;
use super::writer::{CodeWriter, Style, call, doc_comment, text};
use super::{GeneratedFile, JavaPackage, Options};
use crate::model::{Definition, FieldType, Kind, Layout, OffsetSize, Scalar, Schema};
use crate::naming::camel_case;

/// The runtime's package, unless [`Options::runtime`] names another
const RUNTIME: &str = "io.github.snovader.tafla";

/// The package of the generated classes, unless [`Options::package`] names
/// another. It's a placeholder that names no organization, since a project
/// puts generated classes in a package of its own.
const PACKAGE: &str = "generated";

const STYLE: Style = Style {
    width: 100,
    indent: "    ",
    trailing_comma: false,
    hug_arguments: false,
    max_arguments_width: None,
};

/// Names that getters and record components can't take, which names take a
/// `_` after too: the methods of `Object` without parameters, which they would
/// override, the methods of structs' records, and the parameters that they
/// write their components with
const MEMBERS: [&str; 12] = [
    "bytes",
    "clone",
    "finalize",
    "getClass",
    "hashCode",
    "notify",
    "notifyAll",
    "offset",
    "serialize",
    "toString",
    "wait",
    "write",
];

pub fn generate(schema: &Schema, header: &str, options: &Options) -> Vec<GeneratedFile> {
    let runtime = options.runtime.as_deref().unwrap_or(RUNTIME);
    let package = options
        .package
        .as_ref()
        .map_or(PACKAGE, JavaPackage::as_str);

    schema
        .definitions
        .iter()
        .map(|definition| {
            let mut writer = CodeWriter::new(STYLE.indent);
            writer.line(format!("// {header}"));
            writer.line(format!("package {package};"));
            writer.blank();

            match definition.kind {
                Kind::Message(layout) => {
                    imports(&mut writer, runtime, definition, Some(layout));
                    message(&mut writer, schema, definition, layout);
                }
                Kind::Table => {
                    imports(&mut writer, runtime, definition, None);
                    table(&mut writer, schema, definition);
                }
                Kind::Struct => {
                    writer.line(format!("import {runtime}.DecodeException;"));
                    writer.blank();
                    writer.line("import java.lang.invoke.MethodHandles;");
                    writer.line("import java.lang.invoke.VarHandle;");
                    writer.line("import java.nio.ByteBuffer;");
                    writer.line("import java.nio.ByteOrder;");
                    writer.blank();
                    structure(&mut writer, schema, definition);
                }
            }

            GeneratedFile {
                path: format!("{}/{}.java", package.replace('.', "/"), definition.name),
                contents: writer.finish(),
            }
        })
        .collect()
}

fn imports(
    writer: &mut CodeWriter,
    runtime: &str,
    definition: &Definition,
    layout: Option<Layout>,
) {
    let has = |predicate: fn(FieldType) -> bool| {
        definition
            .fields
            .iter()
            .any(|field| predicate(field.field_type))
    };
    let has_vector = has(|field_type| matches!(field_type, FieldType::Vector(_)));

    let mut names = Vec::new();
    if layout.is_some_and(|layout| layout.offset_size != OffsetSize::U32) {
        names.push("AddressableSpace");
    }
    names.push("ByteBuffer");
    if layout.is_some_and(|layout| layout.version.is_some()) {
        names.push("DecodeException");
    }
    names.push("TableBuilder");
    if layout.is_some() {
        names.push("TableLayout");
    }
    for name in names {
        writer.line(format!("import {runtime}.{name};"));
    }

    if has_vector {
        writer.blank();
        writer.line("import java.util.Arrays;");
        writer.line("import java.util.List;");
    }
    writer.blank();
}

fn message(writer: &mut CodeWriter, schema: &Schema, definition: &Definition, layout: Layout) {
    let name = &definition.name;

    writer.lines(&javadoc(&definition.docs));
    writer.open(format!("public final class {name} {{"));
    if let Some(version) = layout.version {
        writer.lines(&doc_comment(&[
            "The version of the format, which the header carries and the constructor checks"
                .to_owned(),
        ]));
        // A long, as the runtime reads and writes versions, since a version
        // of 32-bit offsets may not fit in an int
        let suffix = if i32::try_from(version).is_ok() {
            ""
        } else {
            "L"
        };
        writer.line(format!(
            "public static final long FORMAT_VERSION = {version}{suffix};"
        ));
        writer.blank();
    }
    layout_constant(writer, layout);
    writer.blank();
    writer.line("private final ByteBuffer buffer;");
    writer.blank();

    if layout.version.is_some() {
        writer.lines(&doc_comment(&[
            "@throws DecodeException if {@code bytes} are in another format version".to_owned(),
        ]));
    }
    writer.open(format!("public {name}(byte[] bytes) {{"));
    writer.line("this.buffer = new ByteBuffer(bytes, LAYOUT);");
    if layout.version.is_some() {
        writer.blank();
        writer.line("long version = buffer.getVersion();");
        writer.open("if (version != FORMAT_VERSION) {");
        let message = format!(
            "\"Expected format version \" + FORMAT_VERSION + \" of {name}, got \" + version"
        );
        writer.statement(
            &STYLE,
            "throw ",
            &call("new DecodeException", vec![text(message)]),
            ";",
        );
        writer.close("}");
    }
    writer.close("}");
    writer.blank();

    writer.lines(&javadoc(&[format!(
        "Writes {} with every field",
        article(name)
    )]));
    writer.open("public static byte[] serialize(Fields fields) {");
    let mut arguments = vec![text(definition.fields.len().to_string()), text("LAYOUT")];
    if layout.version.is_some() {
        arguments.push(text("FORMAT_VERSION"));
    }
    writer.statement(
        &STYLE,
        "var builder = ",
        &call("new TableBuilder", arguments),
        ";",
    );
    setters(writer, schema, definition);
    writer.blank();
    writer.line("return builder.bytes();");
    writer.close("}");

    getters(writer, schema, definition, true);
    writer.blank();
    fields_record(writer, schema, definition);
    writer.close("}");
}

/// The layout, from the runtime's builder of layouts
fn layout_constant(writer: &mut CodeWriter, layout: Layout) {
    let mut options = Vec::new();
    if layout.sized {
        options.push(".sized(true)".to_owned());
    }
    if layout.version.is_some() {
        options.push(".versioned(true)".to_owned());
    }
    if layout.offset_size != OffsetSize::U32 {
        options.push(format!(
            ".offsetSize(AddressableSpace.SIZEOF_{})",
            layout.offset_size.bytes() * 8
        ));
    }

    if options.is_empty() {
        writer.line("private static final TableLayout LAYOUT = TableLayout.DEFAULT;");
        return;
    }
    writer.line("private static final TableLayout LAYOUT = TableLayout.builder()");
    writer.indent();
    for option in options {
        writer.line(option);
    }
    writer.line(".build();");
    writer.dedent();
}

fn table(writer: &mut CodeWriter, schema: &Schema, definition: &Definition) {
    let name = &definition.name;

    writer.lines(&javadoc(&definition.docs));
    writer.open(format!("public final class {name} {{"));
    writer.line("private final ByteBuffer buffer;");
    writer.line("private final int position;");
    writer.blank();
    writer.open(format!("{name}(ByteBuffer buffer, int position) {{"));
    writer.line("this.buffer = buffer;");
    writer.line("this.position = position;");
    writer.close("}");
    writer.blank();

    writer.lines(&javadoc(&[
        "Returns a builder with every field set, which a parent writes straight into its own"
            .to_owned(),
        "bytes".to_owned(),
    ]));
    writer.open("public static TableBuilder builder(Fields fields) {");
    writer.line(format!(
        "var builder = new TableBuilder({});",
        definition.fields.len()
    ));
    setters(writer, schema, definition);
    writer.blank();
    writer.line("return builder;");
    writer.close("}");

    getters(writer, schema, definition, false);
    writer.blank();
    fields_record(writer, schema, definition);
    writer.close("}");
}

#[allow(clippy::too_many_lines)]
fn structure(writer: &mut CodeWriter, schema: &Schema, definition: &Definition) {
    let name = &definition.name;
    // Each field, its offset in the struct, and its own struct's size
    let mut offset = 0;
    let fields: Vec<(String, FieldType, usize)> = definition
        .fields
        .iter()
        .map(|field| {
            let at = offset;
            offset += match field.field_type {
                FieldType::Scalar(scalar) => usize::from(scalar.size()),
                FieldType::Struct(index) => schema.struct_size(schema.definition(index)),
                _ => unreachable!("structs hold scalars and structs only"),
            };
            (identifier(&field.name), field.field_type, at)
        })
        .collect();

    writer.lines(&javadoc(&definition.docs));
    let components = fields
        .iter()
        .map(|(field, field_type, _)| text(format!("{} {field}", value_type(*field_type, schema))))
        .collect();
    writer.statement(
        &STYLE,
        "public ",
        &call(format!("record {name}"), components),
        " {",
    );
    writer.indent();
    writer.lines(&javadoc(&[format!("The byte length of {}", article(name))]));
    writer.line(format!(
        "public static final int SIZE = {};",
        schema.struct_size(definition)
    ));
    writer.blank();
    writer.lines(&javadoc(&[format!(
        "The alignment of an array of {}: its largest scalar's size",
        article(name)
    )]));
    writer.line(format!(
        "public static final int ALIGNMENT = {};",
        schema.struct_alignment(definition)
    ));
    writer.blank();

    let mut handles: Vec<&str> = Vec::new();
    for (_, field_type, _) in &fields {
        if let FieldType::Scalar(scalar) = field_type
            && let Some((handle, array)) = var_handle(*scalar)
            && !handles.contains(&handle)
        {
            if handles.is_empty() {
                writer.line(
                    "// The fields lie at fixed offsets, so they're read and written in place, \
                     little-endian",
                );
            }
            handles.push(handle);
            writer.line(format!("private static final VarHandle {handle} ="));
            writer.indent();
            writer.line(format!(
                "MethodHandles.byteArrayViewVarHandle({array}.class, ByteOrder.LITTLE_ENDIAN);"
            ));
            writer.dedent();
        }
    }
    if !handles.is_empty() {
        writer.blank();
    }

    writer.lines(&javadoc(&[
        format!("Reads {} at `offset` in `bytes`", article(name)),
        String::new(),
        "@throws DecodeException if `bytes` end before it does".to_owned(),
    ]));
    writer.open(format!(
        "public static {name} read(byte[] bytes, int offset) {{"
    ));
    writer.open("if (offset < 0 || offset > bytes.length - SIZE) {");
    writer.line("throw new DecodeException(");
    writer.indent();
    writer.line("String.format(");
    writer.indent();
    writer.line("\"Read exceeds the supplied buffer: %d bytes at %d of %d\",");
    writer.line("SIZE,");
    writer.line("offset,");
    writer.line("bytes.length");
    writer.dedent();
    writer.line(")");
    writer.dedent();
    writer.line(");");
    writer.close("}");
    writer.blank();
    let reads = fields
        .iter()
        .map(|(_, field_type, at)| {
            let at = offset_expression("offset", *at);
            text(match field_type {
                FieldType::Scalar(scalar) => struct_read(*scalar, &at),
                FieldType::Struct(index) => {
                    format!("{}.read(bytes, {at})", schema.definition(*index).name)
                }
                _ => unreachable!("structs hold scalars and structs only"),
            })
        })
        .collect();
    writer.statement(&STYLE, "return ", &call(format!("new {name}"), reads), ";");
    writer.close("}");
    writer.blank();

    writer.lines(&javadoc(&[
        format!(
            "Reads {} at `offset` in a little-endian view, such as of an array of them",
            article(name)
        ),
        String::new(),
        "@throws IndexOutOfBoundsException if the view ends before it does".to_owned(),
    ]));
    writer.open(format!(
        "public static {name} read(ByteBuffer view, int offset) {{"
    ));
    let reads = fields
        .iter()
        .map(|(_, field_type, at)| {
            let at = offset_expression("offset", *at);
            text(match field_type {
                FieldType::Scalar(scalar) => view_read(*scalar, &at),
                FieldType::Struct(index) => {
                    format!("{}.read(view, {at})", schema.definition(*index).name)
                }
                _ => unreachable!("structs hold scalars and structs only"),
            })
        })
        .collect();
    writer.statement(&STYLE, "return ", &call(format!("new {name}"), reads), ";");
    writer.close("}");
    writer.blank();

    writer.lines(&javadoc(&[format!(
        "{{@return the bytes of `{name}`s one after another, as an array of them holds them}}"
    )]));
    writer.open(format!(
        "public static byte[] serializeArray({name}[] items) {{"
    ));
    writer.line("var bytes = new byte[items.length * SIZE];");
    writer.open("for (int i = 0; i < items.length; i++) {");
    writer.line("items[i].write(bytes, i * SIZE);");
    writer.close("}");
    writer.blank();
    writer.line("return bytes;");
    writer.close("}");
    writer.blank();

    writer.open("public byte[] serialize() {");
    writer.line("var bytes = new byte[SIZE];");
    writer.line("write(bytes, 0);");
    writer.blank();
    writer.line("return bytes;");
    writer.close("}");
    writer.blank();

    writer.lines(&javadoc(&[format!(
        "Writes the `{name}` at `offset` in `bytes`, which must hold it"
    )]));
    writer.open("public void write(byte[] bytes, int offset) {");
    for (field, field_type, at) in &fields {
        let at = offset_expression("offset", *at);
        match field_type {
            FieldType::Scalar(scalar) => {
                writer.line(format!("{};", struct_write(*scalar, &at, field)));
            }
            FieldType::Struct(_) => writer.line(format!("{field}.write(bytes, {at});")),
            _ => unreachable!("structs hold scalars and structs only"),
        }
    }
    writer.close("}");
    writer.close("}");
}

fn fields_record(writer: &mut CodeWriter, schema: &Schema, definition: &Definition) {
    writer.lines(&javadoc(&[format!(
        "The fields of {}, to write",
        article(&definition.name)
    )]));
    let components = definition
        .fields
        .iter()
        .map(|field| {
            text(format!(
                "{} {}",
                input_type(field.field_type, schema),
                identifier(&field.name)
            ))
        })
        .collect();

    // A record of arrays compares them by identity, which doesn't matter for
    // values that are only written
    let has_array = definition.fields.iter().any(|field| {
        matches!(
            field.field_type,
            FieldType::Array(_) | FieldType::StructArray(_)
        )
    });
    if has_array {
        writer.line("@SuppressWarnings(\"ArrayRecordComponent\") // Written, not compared");
    }
    writer.statement(&STYLE, "public ", &call("record Fields", components), " {}");
}

fn setters(writer: &mut CodeWriter, schema: &Schema, definition: &Definition) {
    for field in &definition.fields {
        let value = format!("fields.{}()", identifier(&field.name));
        let (method, arguments) = match field.field_type {
            FieldType::Scalar(scalar) => (format!("set{}", method_name(scalar)), vec![text(value)]),
            FieldType::Array(scalar) => (
                format!("set{}Array", method_name(scalar)),
                vec![text(value)],
            ),
            FieldType::String => ("setString".to_owned(), vec![text(value)]),
            FieldType::Struct(_) => (
                "setStruct".to_owned(),
                vec![text(format!("{value}.serialize()"))],
            ),
            FieldType::StructArray(index) => {
                let structure = &schema.definition(index).name;
                (
                    "setStructArray".to_owned(),
                    vec![
                        call(format!("{structure}.serializeArray"), vec![text(value)]),
                        text(format!("{structure}.SIZE")),
                        text(format!("{structure}.ALIGNMENT")),
                    ],
                )
            }
            FieldType::Table(index) => {
                let table = &schema.definition(index).name;
                let builder = call(format!("{table}.builder"), vec![text(value)]);
                ("setTable".to_owned(), vec![builder])
            }
            FieldType::Vector(index) => {
                let table = &schema.definition(index).name;
                let builders = format!("{value}.stream().map({table}::builder).toList()");
                ("setVector".to_owned(), vec![text(builders)])
            }
        };
        writer.statement(
            &STYLE,
            "",
            &call(format!("builder.{method}"), arguments),
            ";",
        );
    }
}

fn getters(writer: &mut CodeWriter, schema: &Schema, definition: &Definition, is_root: bool) {
    for (index, field) in definition.fields.iter().enumerate() {
        let offset = if is_root {
            format!("buffer.rootOffset({index})")
        } else {
            format!("buffer.fieldOffset(position, {index})")
        };

        writer.blank();
        writer.lines(&javadoc(&field.docs));
        writer.open(format!(
            "public {} {}() {{",
            value_type(field.field_type, schema),
            identifier(&field.name)
        ));
        match field.field_type {
            FieldType::Scalar(scalar) => {
                writer.line(format!(
                    "return buffer.get{}({offset});",
                    method_name(scalar)
                ));
            }
            FieldType::Array(scalar) => {
                writer.line(format!(
                    "return buffer.get{}Array({offset});",
                    method_name(scalar)
                ));
            }
            FieldType::String => writer.line(format!("return buffer.getString({offset});")),
            FieldType::Struct(index) => {
                let structure = &schema.definition(index).name;
                writer.line(format!(
                    "return {structure}.read(buffer.bytes(), {offset});"
                ));
            }
            FieldType::StructArray(index) => {
                // An array, as arrays of scalars are, from a loop rather than
                // a stream to a list, which took 2.6 times as long to read
                // 100 structs
                let structure = &schema.definition(index).name;
                writer.line(format!(
                    "var view = buffer.getStructArray({offset}, {structure}.SIZE);"
                ));
                writer.line(format!(
                    "var items = new {structure}[view.remaining() / {structure}.SIZE];"
                ));
                writer.blank();
                writer.open("for (int i = 0; i < items.length; i++) {");
                writer.line(format!(
                    "items[i] = {structure}.read(view, i * {structure}.SIZE);"
                ));
                writer.close("}");
                writer.blank();
                writer.line("return items;");
            }
            FieldType::Table(table) => {
                let table = &schema.definition(table).name;
                writer.line(format!("return new {table}(buffer, {offset});"));
            }
            FieldType::Vector(table) => {
                let table = &schema.definition(table).name;
                writer.line(format!("var offsets = buffer.getVectorOffsets({offset});"));
                writer.blank();
                writer.line("return Arrays.stream(offsets)");
                writer.indent();
                writer.line(format!(".mapToObj(item -> new {table}(buffer, item))"));
                writer.line(".toList();");
                writer.dedent();
            }
        }
        writer.close("}");
    }
}

/// Javadoc, with Markdown's code spans as `{@code ...}`
fn javadoc(docs: &[String]) -> Vec<String> {
    // Java reads `\u` as a Unicode escape even in comments
    let docs: Vec<String> = docs
        .iter()
        .map(|line| code_spans(&line.replace("\\u", "&#92;u")))
        .collect();
    doc_comment(&docs)
}

fn code_spans(line: &str) -> String {
    let mut converted = String::new();
    for (index, part) in line.split('`').enumerate() {
        if index % 2 == 1 {
            let _ = write!(converted, "{{@code {part}}}");
        } else {
            converted.push_str(part);
        }
    }
    converted
}

fn identifier(name: &str) -> String {
    let name = camel_case(name);
    if KEYWORDS.contains(&name.as_str()) || MEMBERS.contains(&name.as_str()) {
        format!("{name}_")
    } else {
        name
    }
}

fn article(name: &str) -> String {
    let article = if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    format!("{article} `{name}`")
}

/// `Float32` in `getFloat32()`, as the runtime names its getters and setters
fn method_name(scalar: Scalar) -> String {
    let name = scalar.name();
    name[..1].to_ascii_uppercase() + &name[1..]
}

/// Unsigned values widen to the next signed type, as the runtime returns
/// them, except `uint64`, which is a `long` with its bits
const fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Int8 => "byte",
        Scalar::Uint8 | Scalar::Int16 => "short",
        Scalar::Uint16 | Scalar::Int32 => "int",
        Scalar::Uint32 | Scalar::Int64 | Scalar::Uint64 => "long",
        Scalar::Float32 => "float",
        Scalar::Float64 => "double",
    }
}

fn value_type(field_type: FieldType, schema: &Schema) -> String {
    match field_type {
        FieldType::Scalar(scalar) => scalar_type(scalar).to_owned(),
        FieldType::Array(scalar) => format!("{}[]", scalar_type(scalar)),
        FieldType::String => "String".to_owned(),
        FieldType::Struct(index) | FieldType::Table(index) => schema.definition(index).name.clone(),
        FieldType::StructArray(index) => format!("{}[]", schema.definition(index).name),
        FieldType::Vector(index) => format!("List<{}>", schema.definition(index).name),
    }
}

fn input_type(field_type: FieldType, schema: &Schema) -> String {
    match field_type {
        FieldType::Table(index) => format!("{}.Fields", schema.definition(index).name),
        FieldType::Vector(index) => format!("List<{}.Fields>", schema.definition(index).name),
        _ => value_type(field_type, schema),
    }
}

/// The `VarHandle` constant that reads a scalar, and the array type it views,
/// for those wider than a byte
const fn var_handle(scalar: Scalar) -> Option<(&'static str, &'static str)> {
    match scalar {
        Scalar::Int8 | Scalar::Uint8 => None,
        Scalar::Int16 | Scalar::Uint16 => Some(("INT16", "short[]")),
        Scalar::Int32 | Scalar::Uint32 => Some(("INT32", "int[]")),
        Scalar::Int64 | Scalar::Uint64 => Some(("INT64", "long[]")),
        Scalar::Float32 => Some(("FLOAT32", "float[]")),
        Scalar::Float64 => Some(("FLOAT64", "double[]")),
    }
}

fn offset_expression(base: &str, offset: usize) -> String {
    if offset == 0 {
        base.to_owned()
    } else {
        format!("{base} + {offset}")
    }
}

fn struct_read(scalar: Scalar, at: &str) -> String {
    match scalar {
        Scalar::Int8 => format!("bytes[{at}]"),
        Scalar::Uint8 => format!("(short) Byte.toUnsignedInt(bytes[{at}])"),
        Scalar::Int16 => format!("(short) INT16.get(bytes, {at})"),
        Scalar::Uint16 => format!("Short.toUnsignedInt((short) INT16.get(bytes, {at}))"),
        Scalar::Int32 => format!("(int) INT32.get(bytes, {at})"),
        Scalar::Uint32 => format!("Integer.toUnsignedLong((int) INT32.get(bytes, {at}))"),
        Scalar::Int64 | Scalar::Uint64 => format!("(long) INT64.get(bytes, {at})"),
        Scalar::Float32 => format!("(float) FLOAT32.get(bytes, {at})"),
        Scalar::Float64 => format!("(double) FLOAT64.get(bytes, {at})"),
    }
}

/// Reads a scalar from a little-endian view, as the runtime's getters widen
/// it
fn view_read(scalar: Scalar, at: &str) -> String {
    match scalar {
        Scalar::Int8 => format!("view.get({at})"),
        Scalar::Uint8 => format!("(short) Byte.toUnsignedInt(view.get({at}))"),
        Scalar::Int16 => format!("view.getShort({at})"),
        Scalar::Uint16 => format!("Short.toUnsignedInt(view.getShort({at}))"),
        Scalar::Int32 => format!("view.getInt({at})"),
        Scalar::Uint32 => format!("Integer.toUnsignedLong(view.getInt({at}))"),
        Scalar::Int64 | Scalar::Uint64 => format!("view.getLong({at})"),
        Scalar::Float32 => format!("view.getFloat({at})"),
        Scalar::Float64 => format!("view.getDouble({at})"),
    }
}

fn struct_write(scalar: Scalar, at: &str, field: &str) -> String {
    match scalar {
        Scalar::Int8 => format!("bytes[{at}] = {field}"),
        Scalar::Uint8 => format!("bytes[{at}] = (byte) {field}"),
        Scalar::Int16 => format!("INT16.set(bytes, {at}, {field})"),
        Scalar::Uint16 => format!("INT16.set(bytes, {at}, (short) {field})"),
        Scalar::Int32 => format!("INT32.set(bytes, {at}, {field})"),
        Scalar::Uint32 => format!("INT32.set(bytes, {at}, (int) {field})"),
        Scalar::Int64 | Scalar::Uint64 => format!("INT64.set(bytes, {at}, {field})"),
        Scalar::Float32 => format!("FLOAT32.set(bytes, {at}, {field})"),
        Scalar::Float64 => format!("FLOAT64.set(bytes, {at}, {field})"),
    }
}
