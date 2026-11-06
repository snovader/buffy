//! Resolves and checks parsed declarations into a [`Schema`].

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use crate::diagnostic::Diagnostic;
use crate::model::{
    Definition, Field, FieldType, Kind, Layout, OffsetSize, Scalar, Schema, StructLayout,
};
use crate::naming::camel_case;
use crate::parser::{Attribute, AttributeValue, Declaration, FieldDeclaration, Keyword, Name};

/// Checks every declaration and returns all the errors found, in source order
pub fn check(declarations: &[Declaration]) -> Result<Schema, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let indices = index_names(declarations, &mut errors);

    let mut definitions: Vec<Definition> = declarations
        .iter()
        .map(|declaration| check_declaration(declaration, declarations, &indices, &mut errors))
        .collect();

    check_view_declarations(&definitions, declarations, &indices, &mut errors);

    if errors.is_empty() {
        check_cycles(&definitions, declarations, &mut errors);
        check_nested_offsets(&definitions, declarations, &mut errors);
    }

    // Every struct has a size once none contains itself
    if errors.is_empty() {
        measure_structs(&mut definitions, declarations, &mut errors);
    }

    if errors.is_empty() {
        Ok(Schema { definitions })
    } else {
        errors.sort_by_key(|error| error.span.start);
        Err(errors)
    }
}

fn index_names<'a>(
    declarations: &'a [Declaration],
    errors: &mut Vec<Diagnostic>,
) -> HashMap<&'a str, usize> {
    let mut indices = HashMap::new();

    for (index, declaration) in declarations.iter().enumerate() {
        let name = &declaration.name;
        if !is_pascal_case(&name.text) {
            errors.push(Diagnostic::new(
                format!("`{}` should be PascalCase", name.text),
                name.span,
            ));
        }
        if Scalar::from_name(&name.text).is_some() || name.text == "string" {
            errors.push(Diagnostic::new(
                format!("`{}` is a built-in type", name.text),
                name.span,
            ));
        }
        if indices.insert(name.text.as_str(), index).is_some() {
            errors.push(Diagnostic::new(
                format!("`{}` is declared twice", name.text),
                name.span,
            ));
        }
    }

    indices
}

fn check_declaration(
    declaration: &Declaration,
    declarations: &[Declaration],
    indices: &HashMap<&str, usize>,
    errors: &mut Vec<Diagnostic>,
) -> Definition {
    let kind = match declaration.keyword {
        Keyword::Message => Kind::Message(check_layout(&declaration.attributes, errors)),
        Keyword::Table | Keyword::Struct => {
            if let Some(attribute) = declaration.attributes.first() {
                errors.push(Diagnostic::new(
                    "only messages have attributes",
                    attribute.name.span,
                ));
            }
            if declaration.keyword == Keyword::Table {
                Kind::Table
            } else {
                // Measured by measure_structs() once every struct is resolved
                Kind::Struct(StructLayout {
                    size: 0,
                    alignment: 1,
                })
            }
        }
    };

    // A definition without fields holds no data, and would be written as an
    // empty class or struct in each language
    if declaration.fields.is_empty() {
        errors.push(
            Diagnostic::new(
                format!("`{}` has no fields", declaration.name.text),
                declaration.name.span,
            )
            .with_help("messages, tables and structs hold at least one field"),
        );
    }

    check_field_names(declaration, errors);

    let mut fields = Vec::with_capacity(declaration.fields.len());

    for field in &declaration.fields {
        if let Some(field_type) = resolve_type(field, declarations, indices, errors) {
            if matches!(kind, Kind::Struct(_))
                && !matches!(field_type, FieldType::Scalar(_) | FieldType::Struct(_))
            {
                errors.push(Diagnostic::new(
                    "struct fields must be scalars or structs",
                    field.type_name.span,
                ));
            }

            fields.push(Field {
                name: field.name.text.clone(),
                docs: field.docs.clone(),
                field_type,
            });
        }
    }

    check_view_names(declaration, &fields, errors);
    check_java_parameters(declaration, &fields, errors);

    Definition {
        name: declaration.name.text.clone(),
        docs: declaration.docs.clone(),
        kind,
        fields,
    }
}

/// Checks that a definition's fields are `snake_case` and declared once, and
/// that no two share a name in `camelCase`, which TypeScript, JavaScript and
/// Java name them in
fn check_field_names(declaration: &Declaration, errors: &mut Vec<Diagnostic>) {
    let mut names = HashSet::new();
    // Each camelCase name, with the field that takes it first
    let mut camel_case_names = HashMap::new();

    for field in &declaration.fields {
        let name = &field.name;
        let snake_case = is_snake_case(&name.text);
        if !snake_case {
            errors.push(Diagnostic::new(
                format!("`{}` should be snake_case", name.text),
                name.span,
            ));
        }
        if !names.insert(name.text.as_str()) {
            errors.push(Diagnostic::new(
                format!(
                    "`{}` is declared twice in `{}`",
                    name.text, declaration.name.text
                ),
                name.span,
            ));
            continue;
        }

        // A name that isn't snake_case has its error already
        if !snake_case {
            continue;
        }
        match camel_case_names.entry(camel_case(&name.text)) {
            Entry::Vacant(entry) => {
                entry.insert(name.text.as_str());
            }
            Entry::Occupied(entry) => errors.push(
                Diagnostic::new(
                    format!(
                        "`{}` and `{}` are both `{}` in camelCase",
                        name.text,
                        entry.get(),
                        entry.key()
                    ),
                    name.span,
                )
                .with_help("TypeScript, JavaScript and Java name fields in camelCase"),
            ),
        }
    }
}

/// Checks that no field takes the name of the view of a vector or an array
/// of structs, `<name>_view`, which the generated code reads it lazily with,
/// in `camelCase` too, as [`check_field_names()`] compares names
fn check_view_names(declaration: &Declaration, fields: &[Field], errors: &mut Vec<Diagnostic>) {
    for field in fields {
        if !matches!(
            field.field_type,
            FieldType::Vector(_) | FieldType::StructArray(_)
        ) {
            continue;
        }

        let view = format!("{}View", camel_case(&field.name));
        let clashes = declaration.fields.iter().filter(|other| {
            is_snake_case(&other.name.text) && camel_case(&other.name.text) == view
        });
        for other in clashes {
            errors.push(
                Diagnostic::new(
                    format!(
                        "`{}` clashes with the view of `{}`",
                        other.name.text, field.name
                    ),
                    other.name.span,
                )
                .with_help(format!(
                    "the generated code reads `{}` lazily with `{}_view()`, or `{view}()` in camelCase",
                    field.name, field.name
                )),
            );
        }
    }
}

/// The most parameter slots that a Java constructor takes: the JVM's 255,
/// less the one that `this` takes
const MAX_JAVA_PARAMETER_SLOTS: usize = 254;

/// Checks that Java can declare a record of a definition's fields: it reads a
/// struct into a record of its fields, and writes a message or table from a
/// `Fields` record of them, whose constructors take every field
fn check_java_parameters(
    declaration: &Declaration,
    fields: &[Field],
    errors: &mut Vec<Diagnostic>,
) {
    let slots: usize = fields
        .iter()
        .map(|field| java_parameter_slots(field.field_type))
        .sum();

    if slots > MAX_JAVA_PARAMETER_SLOTS {
        errors.push(
            Diagnostic::new(
                format!(
                    "`{}` has too many fields for a Java record",
                    declaration.name.text
                ),
                declaration.name.span,
            )
            .with_help(format!(
                "its fields take {slots} of a Java constructor's {MAX_JAVA_PARAMETER_SLOTS} parameter slots"
            )),
        );
    }
}

/// The parameter slots of a field in Java, where a `long` or a `double` takes
/// two: 64-bit scalars, and `uint32`, which Java widens to a `long`
const fn java_parameter_slots(field_type: FieldType) -> usize {
    match field_type {
        FieldType::Scalar(Scalar::Uint32 | Scalar::Int64 | Scalar::Uint64 | Scalar::Float64) => 2,
        _ => 1,
    }
}

/// Checks that no declaration takes the name of the view of a vector or an
/// array of structs, `<Table>Vector` or `<Struct>Array`, which the generated
/// code declares beside the tables and structs that they hold
fn check_view_declarations(
    definitions: &[Definition],
    declarations: &[Declaration],
    indices: &HashMap<&str, usize>,
    errors: &mut Vec<Diagnostic>,
) {
    let mut views = HashSet::new();
    let items = definitions
        .iter()
        .flat_map(|definition| &definition.fields)
        .filter_map(|field| match field.field_type {
            FieldType::Vector(index) => Some(("Vector", "a vector", index)),
            FieldType::StructArray(index) => Some(("Array", "an array", index)),
            _ => None,
        });

    for (suffix, kind, index) in items {
        let item = &definitions[index].name;
        let view = format!("{item}{suffix}");
        let Some(&clash) = indices.get(view.as_str()) else {
            continue;
        };
        if !views.insert(clash) {
            continue;
        }

        errors.push(
            Diagnostic::new(
                format!("`{view}` clashes with the view of {kind} of `{item}`"),
                declarations[clash].name.span,
            )
            .with_help(format!(
                "the generated code declares `{view}` to read {kind} of `{item}` lazily"
            )),
        );
    }
}

fn check_layout(attributes: &[Attribute], errors: &mut Vec<Diagnostic>) -> Layout {
    let mut layout = Layout {
        sized: false,
        version: None,
        offset_size: OffsetSize::Uint32,
    };
    let mut version = None;
    let mut given = HashSet::new();

    for attribute in attributes {
        let name = attribute.name.text.as_str();
        if !given.insert(name) {
            errors.push(Diagnostic::new(
                format!("`{name}` is given twice"),
                attribute.name.span,
            ));
            continue;
        }

        match (name, &attribute.value) {
            ("sized", None) => layout.sized = true,
            ("version", Some(AttributeValue::Integer(value, span))) => {
                version = Some((*value, *span));
            }
            ("offset_type", Some(value)) => {
                layout.offset_size = offset_type(value).unwrap_or_else(|error| {
                    errors.push(error);
                    OffsetSize::Uint32
                });
            }
            ("sized", Some(value)) => {
                errors.push(Diagnostic::new("`sized` takes no value", value.span()));
            }
            ("version", Some(AttributeValue::Name(value))) => errors.push(
                Diagnostic::new("`version` takes an integer", value.span)
                    .with_help("such as `version: 1`"),
            ),
            ("version", None) => errors.push(
                Diagnostic::new("`version` needs a value", attribute.name.span)
                    .with_help("such as `version: 1`"),
            ),
            ("offset_type", None) => errors.push(
                Diagnostic::new("`offset_type` needs a value", attribute.name.span)
                    .with_help("such as `offset_type: uint16`"),
            ),
            ("offset_size", value) => errors.push(
                Diagnostic::new("unknown attribute `offset_size`", attribute.name.span)
                    .with_help(offset_size_help(value.as_ref())),
            ),
            (other, _) => errors.push(
                Diagnostic::new(format!("unknown attribute `{other}`"), attribute.name.span)
                    .with_help("expected `sized`, `version` or `offset_type`"),
            ),
        }
    }

    // Checked once the offset type is known, whichever attribute comes first
    if let Some((value, span)) = version {
        let max = layout.offset_size.max_value();
        match u32::try_from(value) {
            Ok(value) if u64::from(value) <= max => layout.version = Some(value),
            _ => errors.push(
                Diagnostic::new(
                    format!(
                        "version {value} doesn't fit in the offset type `{}`",
                        layout.offset_size.type_name()
                    ),
                    span,
                )
                .with_help(format!("the largest is {max}")),
            ),
        }
    }

    layout
}

/// The offset size of an `offset_type` attribute's value, which names one of
/// the unsigned integer types
fn offset_type(value: &AttributeValue) -> Result<OffsetSize, Diagnostic> {
    let found = match value {
        AttributeValue::Name(name) => {
            if let Some(offset_size) = OffsetSize::from_type_name(&name.text) {
                return Ok(offset_size);
            }
            name.text.clone()
        }
        AttributeValue::Integer(value, _) => value.to_string(),
    };

    Err(
        Diagnostic::new(format!("`{found}` isn't an offset type"), value.span())
            .with_help("the offset types are `uint8`, `uint16` and `uint32`"),
    )
}

/// Points `offset_size`, which took the offsets' width in bits, to
/// `offset_type`, with the type of that width when it's one
fn offset_size_help(value: Option<&AttributeValue>) -> String {
    match value {
        Some(AttributeValue::Integer(bits @ (8 | 16 | 32), _)) => {
            format!("`offset_type` sets the offsets' width, such as `offset_type: uint{bits}`")
        }
        _ => "`offset_type` sets the offsets' width: `uint8`, `uint16` or `uint32`".to_owned(),
    }
}

fn resolve_type(
    field: &FieldDeclaration,
    declarations: &[Declaration],
    indices: &HashMap<&str, usize>,
    errors: &mut Vec<Diagnostic>,
) -> Option<FieldType> {
    let name = &field.type_name;

    if let Some(scalar) = Scalar::from_name(&name.text) {
        return Some(if field.is_array {
            FieldType::Array(scalar)
        } else {
            FieldType::Scalar(scalar)
        });
    }

    if name.text == "string" {
        if field.is_array {
            errors.push(Diagnostic::new(
                "arrays of strings aren't supported",
                name.span,
            ));
            return None;
        }
        return Some(FieldType::String);
    }

    let Some(&index) = indices.get(name.text.as_str()) else {
        errors.push(unknown_type(name, indices));
        return None;
    };

    match declarations[index].keyword {
        Keyword::Table => Some(if field.is_array {
            FieldType::Vector(index)
        } else {
            FieldType::Table(index)
        }),
        Keyword::Message => {
            errors.push(
                Diagnostic::new(
                    format!("`{}` is a message, which can't be nested", name.text),
                    name.span,
                )
                .with_help("declare it as a table to nest it"),
            );
            None
        }
        Keyword::Struct => Some(if field.is_array {
            FieldType::StructArray(index)
        } else {
            FieldType::Struct(index)
        }),
    }
}

/// The most edits that a misspelled type is suggested across
const MAX_SUGGESTION_DISTANCE: usize = 2;

fn unknown_type(name: &Name, indices: &HashMap<&str, usize>) -> Diagnostic {
    let builtins = Scalar::ALL
        .iter()
        .map(|scalar| scalar.name())
        .chain(["string"]);
    let suggestion = indices
        .keys()
        .copied()
        .chain(builtins)
        // At least as far apart as their lengths, which spares comparing long
        // names character by character
        .filter(|candidate| candidate.len().abs_diff(name.text.len()) <= MAX_SUGGESTION_DISTANCE)
        .map(|candidate| (edit_distance(&name.text, candidate), candidate))
        .filter(|(distance, _)| *distance <= MAX_SUGGESTION_DISTANCE)
        .min();

    let error = Diagnostic::new(format!("unknown type `{}`", name.text), name.span);
    match suggestion {
        Some((_, candidate)) => error.with_help(format!("did you mean `{candidate}`?")),
        None => error,
    }
}

/// Rejects tables that nest themselves without a vector in between, since
/// every field is required and such a table would never end, and structs that
/// contain themselves, which would have no size
fn check_cycles(
    definitions: &[Definition],
    declarations: &[Declaration],
    errors: &mut Vec<Diagnostic>,
) {
    for (start, definition) in definitions.iter().enumerate() {
        let mut stack: Vec<usize> = nested_in_place(definition).collect();
        let mut seen = vec![false; definitions.len()];

        while let Some(index) = stack.pop() {
            if index == start {
                let help = if definition.is_struct() {
                    "a struct can't contain itself, even within another struct"
                } else {
                    "only a vector can nest a table in itself"
                };
                errors.push(
                    Diagnostic::new(
                        format!("`{}` contains itself", definition.name),
                        declarations[start].name.span,
                    )
                    .with_help(help),
                );
                break;
            }
            if !std::mem::replace(&mut seen[index], true) {
                stack.extend(nested_in_place(&definitions[index]));
            }
        }
    }
}

/// The most bytes that a struct takes. The generated code reads and writes
/// each of a struct's scalars in place, and a schema of a few lines could
/// otherwise declare a struct of any size, by nesting structs in pairs.
const MAX_STRUCT_SIZE: usize = 65_536;

/// Measures each struct's size and alignment once, from those of the structs
/// in it, and rejects structs larger than [`MAX_STRUCT_SIZE`]
fn measure_structs(
    definitions: &mut [Definition],
    declarations: &[Declaration],
    errors: &mut Vec<Diagnostic>,
) {
    let mut layouts = vec![None; definitions.len()];
    for (index, definition) in definitions.iter().enumerate() {
        if definition.is_struct() {
            measure_struct(index, definitions, &mut layouts);
        }
    }

    let too_large = |index: usize| {
        layouts[index].is_some_and(|layout: StructLayout| layout.size > MAX_STRUCT_SIZE)
    };
    for (index, definition) in definitions.iter().enumerate() {
        // The error points at the structs that grow too large, rather than
        // at every struct that holds one of them
        let holds_too_large = definition
            .fields
            .iter()
            .any(|field| matches!(field.field_type, FieldType::Struct(inner) if too_large(inner)));
        if too_large(index) && !holds_too_large {
            errors.push(
                Diagnostic::new(
                    format!("`{}` is too large for a struct", definition.name),
                    declarations[index].name.span,
                )
                .with_help(format!("structs take at most {MAX_STRUCT_SIZE} bytes")),
            );
        }
    }

    for (definition, layout) in definitions.iter_mut().zip(layouts) {
        if let Some(layout) = layout {
            definition.kind = Kind::Struct(layout);
        }
    }
}

/// The layout of the struct at `index`, kept in `layouts` once measured, so
/// that a struct nested in several others is measured once. Its size
/// saturates rather than overflowing, since a few lines can double it many
/// times over.
fn measure_struct(
    index: usize,
    definitions: &[Definition],
    layouts: &mut [Option<StructLayout>],
) -> StructLayout {
    if let Some(layout) = layouts[index] {
        return layout;
    }

    let mut layout = StructLayout {
        size: 0,
        alignment: 1,
    };
    for field in &definitions[index].fields {
        let (size, alignment) = match field.field_type {
            FieldType::Scalar(scalar) => (usize::from(scalar.size()), scalar.size()),
            FieldType::Struct(inner) => {
                let inner = measure_struct(inner, definitions, layouts);
                (inner.size, inner.alignment)
            }
            _ => unreachable!("structs hold scalars and structs only"),
        };
        layout.size = layout.size.saturating_add(size);
        layout.alignment = layout.alignment.max(alignment);
    }

    layouts[index] = Some(layout);
    layout
}

/// Rejects tables nested in messages of a narrower offset type, since readers
/// read a nested table's offsets as of its message's offset type, while
/// tables are written with `uint32` offsets
fn check_nested_offsets(
    definitions: &[Definition],
    declarations: &[Declaration],
    errors: &mut Vec<Diagnostic>,
) {
    for (definition, declaration) in definitions.iter().zip(declarations) {
        let Kind::Message(layout) = definition.kind else {
            continue;
        };
        if layout.offset_size == OffsetSize::Uint32 {
            continue;
        }

        for (field, field_declaration) in definition.fields.iter().zip(&declaration.fields) {
            if matches!(field.field_type, FieldType::Table(_) | FieldType::Vector(_)) {
                errors.push(
                    Diagnostic::new(
                        format!(
                            "`{}` can't nest tables, since its offset type is `{}`",
                            definition.name,
                            layout.offset_size.type_name()
                        ),
                        field_declaration.type_name.span,
                    )
                    .with_help("nested tables need the offset type `uint32`"),
                );
            }
        }
    }
}

/// The tables and structs that a definition holds in place, rather than in a
/// vector or an array, which can be empty
fn nested_in_place(definition: &Definition) -> impl Iterator<Item = usize> + '_ {
    definition
        .fields
        .iter()
        .filter_map(|field| match field.field_type {
            FieldType::Table(index) | FieldType::Struct(index) => Some(index),
            _ => None,
        })
}

fn is_pascal_case(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_uppercase())
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn is_snake_case(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_lowercase())
        && !name.ends_with('_')
        && !name.contains("__")
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

/// The optimal string alignment distance: how many characters to insert,
/// delete, replace or swap with their neighbour to turn `a` into `b`, for
/// suggesting a type that was misspelled
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    // The distances from `a`'s prefixes of two characters ago, one ago and
    // this one to each of `b`'s prefixes
    let mut before = vec![0; b.len() + 1];
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];

    for i in 1..=a.len() {
        current[0] = i;
        for j in 1..=b.len() {
            let replacement = previous[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            let mut distance = replacement.min(previous[j] + 1).min(current[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                distance = distance.min(before[j - 2] + 1);
            }
            current[j] = distance;
        }

        std::mem::swap(&mut before, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }

    previous[b.len()]
}
