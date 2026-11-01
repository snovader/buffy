//! TypeScript, and JavaScript with `JSDoc` types, for `@buffy/core`. Messages
//! and tables are classes that read fields lazily from their bytes, and are
//! written from interfaces of their fields. Structs are read eagerly.

use super::writer::{CodeWriter, Expr, Style, call, camel_case, comment_text, doc_comment, text};
use super::{GeneratedFile, Options};
use crate::model::{Definition, FieldType, Kind, Layout, OffsetSize, Scalar, Schema};

/// The runtime's module, unless [`Options::runtime`] names another
const RUNTIME: &str = "@buffy/core";

const STYLE: Style = Style {
    width: 80,
    indent: "    ",
    trailing_comma: false,
    hug_arguments: false,
    max_arguments_width: None,
};

/// Members that classes define or inherit, which names take a `_` after: the
/// constructor and the other methods of `Object.prototype`, which a getter
/// would replace, and the writer that structs keep among the private names of
/// their fields
const MEMBERS: [&str; 8] = [
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
    "writer",
];

pub fn generate(
    schema: &Schema,
    stem: &str,
    header: &str,
    options: &Options,
    typed: bool,
) -> Vec<GeneratedFile> {
    let generator = Generator { schema, typed };
    let runtime = options.runtime.as_deref().unwrap_or(RUNTIME);

    let mut writer = CodeWriter::new(STYLE.indent);
    writer.line(format!("// {header}"));
    writer.blank();
    generator.imports(&mut writer, runtime);

    for definition in &schema.definitions {
        writer.blank();
        match definition.kind {
            Kind::Message(layout) => generator.message(&mut writer, definition, layout),
            Kind::Table => generator.table(&mut writer, definition),
            Kind::Struct => generator.structure(&mut writer, definition),
        }
        writer.blank();
        generator.fields_type(&mut writer, definition);
    }

    let extension = if typed { "ts" } else { "js" };
    vec![GeneratedFile {
        path: format!("{}.{extension}", camel_case(stem)),
        contents: writer.finish(),
    }]
}

struct Generator<'a> {
    schema: &'a Schema,
    /// TypeScript, rather than JavaScript with `JSDoc`
    typed: bool,
}

impl Generator<'_> {
    fn imports(&self, writer: &mut CodeWriter, runtime: &str) {
        let kinds = |predicate: fn(&Kind) -> bool| {
            self.schema
                .definitions
                .iter()
                .any(|definition| predicate(&definition.kind))
        };
        let has_message = kinds(|kind| matches!(kind, Kind::Message(_)));
        let has_table = kinds(|kind| matches!(kind, Kind::Message(_) | Kind::Table));
        let has_struct = kinds(|kind| matches!(kind, Kind::Struct));
        let versioned = kinds(|kind| {
            matches!(
                kind,
                Kind::Message(Layout {
                    version: Some(_),
                    ..
                })
            )
        });

        // (name, whether it's only a type here)
        let mut names = Vec::new();
        if has_table || has_struct {
            names.push(("ByteBuffer", !(has_message || has_struct)));
        }
        if versioned {
            names.push(("DecodeError", false));
        }
        if has_struct {
            names.push(("StructWriter", false));
        }
        if has_table {
            names.push(("TableBuilder", false));
        }
        if has_message {
            names.push(("TableLayout", false));
        }

        let mut values = Vec::new();
        let mut types = Vec::new();
        for (name, type_only) in names {
            match (type_only, self.typed) {
                (false, _) => values.push(name.to_owned()),
                (true, true) => values.push(format!("type {name}")),
                (true, false) => types.push(name),
            }
        }

        if !types.is_empty() {
            writer.line(format!(
                "/** @import {{ {} }} from '{runtime}' */",
                types.join(", ")
            ));
        }
        // A schema without definitions imports nothing
        if values.is_empty() {
            return;
        }
        let statement = format!("import {{ {} }} from '{runtime}';", values.join(", "));
        if statement.len() <= STYLE.width {
            writer.line(statement);
        } else {
            writer.open("import {");
            let last = values.len() - 1;
            for (index, value) in values.iter().enumerate() {
                writer.line(if index < last {
                    format!("{value},")
                } else {
                    value.clone()
                });
            }
            writer.close(format!("}} from '{runtime}';"));
        }
    }

    fn message(&self, writer: &mut CodeWriter, definition: &Definition, layout: Layout) {
        let name = &definition.name;

        writer.lines(&doc_comment(&definition.docs));
        writer.open(format!("export class {name} {{"));

        if let Some(version) = layout.version {
            writer.lines(&doc_comment(&[
                "The version of the format, which the header carries and the".to_owned(),
                "constructor checks".to_owned(),
            ]));
            writer.line(format!(
                "{}FORMAT_VERSION = {version};",
                self.static_readonly()
            ));
            writer.blank();
        }

        writer.statement(
            &STYLE,
            &format!("{}#LAYOUT = ", self.static_readonly()),
            &layout_expression(layout),
            ";",
        );
        writer.blank();
        self.private_field(writer, "buffer", "ByteBuffer");
        writer.blank();

        let mut tags = vec!["@param {Uint8Array} bytes".to_owned()];
        if layout.version.is_some() {
            tags.push("@throws {DecodeError} If `bytes` are in another format version.".to_owned());
        }
        self.doc(writer, &[], &tags);
        writer.open(format!(
            "constructor(bytes{}) {{",
            self.annotation("Uint8Array")
        ));
        writer.statement(
            &STYLE,
            "this.#buffer = ",
            &call(
                "new ByteBuffer",
                vec![text("bytes"), text(format!("{name}.#LAYOUT"))],
            ),
            ";",
        );
        if layout.version.is_some() {
            writer.blank();
            writer.line("const version = this.#buffer.getVersion();");
            writer.open(format!("if (version !== {name}.FORMAT_VERSION) {{"));
            let message = format!(
                "`Expected format version ${{{name}.FORMAT_VERSION}} of {name}, got ${{version}}`"
            );
            writer.statement(
                &STYLE,
                "throw ",
                &call("new DecodeError", vec![text(message)]),
                ";",
            );
            writer.close("}");
        }
        writer.close("}");
        writer.blank();

        self.doc(
            writer,
            &[format!("Writes a `{name}` with every field")],
            &[
                format!("@param {{{name}Fields}} fields"),
                "@returns {Uint8Array}".to_owned(),
            ],
        );
        writer.open(format!(
            "static serialize(fields{}){} {{",
            self.annotation(&format!("{name}Fields")),
            self.annotation("Uint8Array")
        ));
        let mut arguments = vec![
            text(definition.fields.len().to_string()),
            text(format!("{name}.#LAYOUT")),
        ];
        if layout.version.is_some() {
            arguments.push(text(format!("{name}.FORMAT_VERSION")));
        }
        writer.statement(
            &STYLE,
            "const builder = ",
            &call("new TableBuilder", arguments),
            ";",
        );
        self.setters(writer, definition);
        writer.blank();
        writer.line("return builder.bytes();");
        writer.close("}");

        self.getters(writer, definition, true);
        writer.close("}");
    }

    fn table(&self, writer: &mut CodeWriter, definition: &Definition) {
        let name = &definition.name;

        writer.lines(&doc_comment(&definition.docs));
        writer.open(format!("export class {name} {{"));
        self.private_field(writer, "buffer", "ByteBuffer");
        self.private_field(writer, "position", "number");
        writer.blank();

        self.doc(
            writer,
            &["Reads the table at `position`, as its parent's getter finds it".to_owned()],
            &[
                "@param {ByteBuffer} buffer".to_owned(),
                "@param {number} position".to_owned(),
            ],
        );
        writer.open(format!(
            "constructor(buffer{}, position{}) {{",
            self.annotation("ByteBuffer"),
            self.annotation("number")
        ));
        writer.line("this.#buffer = buffer;");
        writer.line("this.#position = position;");
        writer.close("}");
        writer.blank();

        self.doc(
            writer,
            &[
                "Returns a builder with every field set, which a parent writes".to_owned(),
                "straight into its own bytes".to_owned(),
            ],
            &[
                format!("@param {{{name}Fields}} fields"),
                "@returns {TableBuilder}".to_owned(),
            ],
        );
        writer.open(format!(
            "static builder(fields{}){} {{",
            self.annotation(&format!("{name}Fields")),
            self.annotation("TableBuilder")
        ));
        writer.line(format!(
            "const builder = new TableBuilder({});",
            definition.fields.len()
        ));
        self.setters(writer, definition);
        writer.blank();
        writer.line("return builder;");
        writer.close("}");

        self.getters(writer, definition, false);
        writer.close("}");
    }

    #[allow(clippy::too_many_lines)]
    fn structure(&self, writer: &mut CodeWriter, definition: &Definition) {
        let name = &definition.name;

        writer.lines(&doc_comment(&definition.docs));
        writer.open(format!("export class {name} {{"));
        writer.lines(&doc_comment(&[format!("The byte length of a `{name}`")]));
        writer.line(format!(
            "{}SIZE = {};",
            self.static_readonly(),
            definition.struct_size()
        ));
        writer.blank();
        writer.statement(
            &STYLE,
            &format!("{}#writer = ", self.static_readonly()),
            &call(
                "new StructWriter",
                vec![text(format!("new Uint8Array({name}.SIZE)"))],
            ),
            ";",
        );
        writer.blank();
        for field in &definition.fields {
            self.private_field(
                writer,
                &identifier(&field.name),
                &value_type(field.field_type, self.schema),
            );
        }
        writer.blank();

        self.doc(
            writer,
            &[
                format!("Reads a `{name}` from the start of `bytes`, eagerly, so that it"),
                "doesn't keep them alive".to_owned(),
            ],
            &[
                "@param {Uint8Array} bytes".to_owned(),
                format!("@throws {{DecodeError}} If `bytes` are shorter than `{name}.SIZE`."),
            ],
        );
        writer.open(format!(
            "constructor(bytes{}) {{",
            self.annotation("Uint8Array")
        ));
        writer.line("const buffer = new ByteBuffer(bytes);");
        let mut offset = 0;
        for field in &definition.fields {
            let FieldType::Scalar(scalar) = field.field_type else {
                unreachable!("structs hold scalars only")
            };
            writer.statement(
                &STYLE,
                &format!("this.#{} = ", identifier(&field.name)),
                &call(
                    format!("buffer.get{}", scalar_method(scalar)),
                    vec![text(offset.to_string())],
                ),
                ";",
            );
            offset += usize::from(scalar.size());
        }
        writer.close("}");
        writer.blank();

        self.doc(
            writer,
            &[format!("Writes a `{name}`")],
            &[
                format!("@param {{{name}Fields}} fields"),
                "@returns {Uint8Array}".to_owned(),
            ],
        );
        writer.open(format!(
            "static serialize(fields{}){} {{",
            self.annotation(&format!("{name}Fields")),
            self.annotation("Uint8Array")
        ));
        writer.line(format!("const writer = {name}.#writer;"));
        // The writer is shared, and a previous call whose value threw, such
        // as a number for an int64, left it within that struct
        writer.line("writer.reset();");
        for field in &definition.fields {
            let FieldType::Scalar(scalar) = field.field_type else {
                unreachable!("structs hold scalars only")
            };
            writer.line(format!(
                "writer.set{}(fields.{});",
                scalar_method(scalar),
                identifier(&field.name)
            ));
        }
        writer.blank();
        writer.line("return writer.bytes();");
        writer.close("}");

        for field in &definition.fields {
            let field_name = identifier(&field.name);
            writer.blank();
            self.getter_doc(
                writer,
                &field.docs,
                &value_type(field.field_type, self.schema),
            );
            writer.open(format!(
                "{field_name}(){} {{",
                self.annotation(&value_type(field.field_type, self.schema))
            ));
            writer.line(format!("return this.#{field_name};"));
            writer.close("}");
        }
        writer.close("}");
    }

    /// The interface, or `JSDoc` typedef, of the values that write a definition
    fn fields_type(&self, writer: &mut CodeWriter, definition: &Definition) {
        let name = format!("{}Fields", definition.name);
        let summary = format!("The fields of a `{}`, to write", definition.name);

        if self.typed {
            writer.lines(&doc_comment(&[summary]));
            writer.open(format!("export interface {name} {{"));
            for field in &definition.fields {
                writer.lines(&doc_comment(&field.docs));
                writer.line(format!(
                    "{}: {};",
                    identifier(&field.name),
                    input_type(field.field_type, self.schema)
                ));
            }
            writer.close("}");
        } else {
            let mut lines = vec![
                summary,
                String::new(),
                format!("@typedef {{object}} {name}"),
            ];
            for field in &definition.fields {
                let description = field.docs.join(" ");
                let property = format!(
                    "@property {{{}}} {} {description}",
                    input_type(field.field_type, self.schema),
                    identifier(&field.name)
                );
                lines.push(property.trim_end().to_owned());
            }
            writer.lines(&doc_comment(&lines));
        }
    }

    fn setters(&self, writer: &mut CodeWriter, definition: &Definition) {
        for field in &definition.fields {
            let value = format!("fields.{}", identifier(&field.name));
            let (method, argument) = match field.field_type {
                FieldType::Scalar(scalar) => (format!("set{}", scalar_method(scalar)), text(value)),
                FieldType::Array(scalar) => {
                    (format!("set{}Array", scalar_method(scalar)), text(value))
                }
                FieldType::String => ("setString".to_owned(), text(value)),
                FieldType::Table(index) => {
                    let table = &self.schema.definition(index).name;
                    (
                        "setTable".to_owned(),
                        call(format!("{table}.builder"), vec![text(value)]),
                    )
                }
                FieldType::Vector(index) => {
                    let table = &self.schema.definition(index).name;
                    let builders = call(
                        format!("{value}.map"),
                        vec![text(format!("{table}.builder"))],
                    );
                    ("setVector".to_owned(), builders)
                }
            };
            writer.statement(
                &STYLE,
                "",
                &call(format!("builder.{method}"), vec![argument]),
                ";",
            );
        }
    }

    fn getters(&self, writer: &mut CodeWriter, definition: &Definition, is_root: bool) {
        for (index, field) in definition.fields.iter().enumerate() {
            let offset = if is_root {
                call("this.#buffer.rootOffset", vec![text(index.to_string())])
            } else {
                call(
                    "this.#buffer.fieldOffset",
                    vec![text("this.#position"), text(index.to_string())],
                )
            };
            let return_type = value_type(field.field_type, self.schema);
            let field_name = identifier(&field.name);

            writer.blank();
            self.getter_doc(writer, &field.docs, &return_type);
            writer.open(format!(
                "{field_name}(){} {{",
                self.annotation(&return_type)
            ));

            match field.field_type {
                FieldType::Scalar(scalar) => {
                    let get = call(
                        format!("this.#buffer.get{}", scalar_method(scalar)),
                        vec![offset],
                    );
                    writer.statement(&STYLE, "return ", &get, ";");
                }
                FieldType::Array(scalar) => {
                    let get = call(
                        format!("this.#buffer.get{}Array", scalar_method(scalar)),
                        vec![offset],
                    );
                    writer.statement(&STYLE, "return ", &get, ";");
                }
                FieldType::String => {
                    writer.statement(
                        &STYLE,
                        "return ",
                        &call("this.#buffer.getString", vec![offset]),
                        ";",
                    );
                }
                FieldType::Table(table) => {
                    let table = &self.schema.definition(table).name;
                    let read = call(format!("new {table}"), vec![text("this.#buffer"), offset]);
                    writer.statement(&STYLE, "return ", &read, ";");
                }
                FieldType::Vector(table) => {
                    let table = &self.schema.definition(table).name;
                    let array = if self.typed {
                        format!("new Array<{table}>(offsets.length)")
                    } else {
                        "new Array(offsets.length)".to_owned()
                    };

                    writer.statement(
                        &STYLE,
                        "const offsets = ",
                        &call("this.#buffer.getVectorOffsets", vec![offset]),
                        ";",
                    );
                    // The locals aren't named after the field, whose name may
                    // be a reserved word
                    writer.line(format!("const items = {array};"));
                    writer.blank();
                    writer.open("for (let i = 0; i < offsets.length; i++) {");
                    writer.statement(
                        &STYLE,
                        "items[i] = ",
                        &call(
                            format!("new {table}"),
                            vec![text("this.#buffer"), text("offsets[i]")],
                        ),
                        ";",
                    );
                    writer.close("}");
                    writer.blank();
                    writer.line("return items;");
                }
            }

            writer.close("}");
        }
    }

    /// A getter's documentation, with its return type in JavaScript
    fn getter_doc(&self, writer: &mut CodeWriter, docs: &[String], return_type: &str) {
        if self.typed {
            writer.lines(&doc_comment(docs));
            return;
        }

        // `@returns {type} description` on one line when it fits
        if let [line] = docs {
            let folded = format!("/** @returns {{{return_type}}} {} */", comment_text(line));
            if writer_fits(writer, &folded) {
                writer.line(folded);
                return;
            }
        }
        let tags = [format!("@returns {{{return_type}}}")];
        self.doc(writer, docs, &tags);
    }

    /// Documentation with `JSDoc` type tags, which TypeScript leaves out
    fn doc(&self, writer: &mut CodeWriter, docs: &[String], tags: &[String]) {
        let tags: Vec<&String> = tags
            .iter()
            .filter(|tag| !self.typed || tag.starts_with("@throws"))
            .collect();

        let mut lines = docs.to_vec();
        if !lines.is_empty() && !tags.is_empty() {
            lines.push(String::new());
        }
        lines.extend(tags.into_iter().cloned());
        writer.lines(&doc_comment(&lines));
    }

    fn private_field(&self, writer: &mut CodeWriter, name: &str, field_type: &str) {
        if self.typed {
            writer.line(format!("readonly #{name}: {field_type};"));
        } else {
            writer.line(format!("/** @type {{{field_type}}} */"));
            writer.line(format!("#{name};"));
        }
    }

    fn static_readonly(&self) -> &'static str {
        if self.typed {
            "static readonly "
        } else {
            "static "
        }
    }

    fn annotation(&self, type_name: &str) -> String {
        if self.typed {
            format!(": {type_name}")
        } else {
            String::new()
        }
    }
}

fn writer_fits(_writer: &CodeWriter, line: &str) -> bool {
    // Getters are indented once, in a class
    STYLE.indent.len() + line.len() <= STYLE.width
}

fn identifier(name: &str) -> String {
    let name = camel_case(name);
    if MEMBERS.contains(&name.as_str()) {
        format!("{name}_")
    } else {
        name
    }
}

fn layout_expression(layout: Layout) -> Expr {
    let mut entries = Vec::new();
    if layout.sized {
        entries.push(("sized".to_owned(), "true".to_owned()));
    }
    if layout.version.is_some() {
        entries.push(("versioned".to_owned(), "true".to_owned()));
    }
    if layout.offset_size != OffsetSize::U32 {
        entries.push((
            "offsetSize".to_owned(),
            layout.offset_size.bytes().to_string(),
        ));
    }

    if entries.is_empty() {
        text("TableLayout.DEFAULT")
    } else {
        call("TableLayout.create", vec![Expr::Object(entries)])
    }
}

/// `Float32` in `getFloat32()`
fn scalar_method(scalar: Scalar) -> String {
    let name = scalar.name();
    name[..1].to_ascii_uppercase() + &name[1..]
}

/// The type that a getter returns
fn value_type(field_type: FieldType, schema: &Schema) -> String {
    match field_type {
        FieldType::Scalar(scalar) => scalar_type(scalar).to_owned(),
        FieldType::Array(scalar) => typed_array(scalar).to_owned(),
        FieldType::String => "string".to_owned(),
        FieldType::Table(index) => schema.definition(index).name.clone(),
        FieldType::Vector(index) => format!("{}[]", schema.definition(index).name),
    }
}

/// The type that a field's setter takes
fn input_type(field_type: FieldType, schema: &Schema) -> String {
    match field_type {
        FieldType::Scalar(scalar) => scalar_type(scalar).to_owned(),
        FieldType::Array(scalar) => {
            format!(
                "ArrayLike<{}> | {}",
                scalar_type(scalar),
                typed_array(scalar)
            )
        }
        FieldType::String => "string".to_owned(),
        FieldType::Table(index) => format!("{}Fields", schema.definition(index).name),
        FieldType::Vector(index) => format!("readonly {}Fields[]", schema.definition(index).name),
    }
}

const fn scalar_type(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Int64 | Scalar::Uint64 => "bigint",
        _ => "number",
    }
}

const fn typed_array(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Int8 => "Int8Array",
        Scalar::Uint8 => "Uint8Array",
        Scalar::Int16 => "Int16Array",
        Scalar::Uint16 => "Uint16Array",
        Scalar::Int32 => "Int32Array",
        Scalar::Uint32 => "Uint32Array",
        Scalar::Int64 => "BigInt64Array",
        Scalar::Uint64 => "BigUint64Array",
        Scalar::Float32 => "Float32Array",
        Scalar::Float64 => "Float64Array",
    }
}
