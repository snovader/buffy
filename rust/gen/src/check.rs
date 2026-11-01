//! Resolves and checks parsed declarations into a [`Schema`].

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use crate::diagnostic::Diagnostic;
use crate::model::{Definition, Field, FieldType, Kind, Layout, OffsetSize, Scalar, Schema};
use crate::naming::camel_case;
use crate::parser::{Attribute, Declaration, FieldDeclaration, Keyword, Name};

/// Checks every declaration and returns all the errors found, in source order
pub fn check(declarations: &[Declaration]) -> Result<Schema, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let indices = index_names(declarations, &mut errors);

    let definitions: Vec<Definition> = declarations
        .iter()
        .map(|declaration| check_declaration(declaration, declarations, &indices, &mut errors))
        .collect();

    if errors.is_empty() {
        check_cycles(&definitions, declarations, &mut errors);
        check_nested_offsets(&definitions, declarations, &mut errors);
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
                Kind::Struct
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
            if kind == Kind::Struct && !matches!(field_type, FieldType::Scalar(_)) {
                errors.push(Diagnostic::new(
                    "struct fields must be scalars",
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

fn check_layout(attributes: &[Attribute], errors: &mut Vec<Diagnostic>) -> Layout {
    let mut layout = Layout {
        sized: false,
        version: None,
        offset_size: OffsetSize::U32,
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

        match (name, attribute.value) {
            ("sized", None) => layout.sized = true,
            ("version", Some(value)) => version = Some(value),
            ("offset_size", Some((bits, span))) => {
                layout.offset_size = match bits {
                    8 => OffsetSize::U8,
                    16 => OffsetSize::U16,
                    32 => OffsetSize::U32,
                    _ => {
                        errors.push(Diagnostic::new("offsets are 8, 16 or 32 bits wide", span));
                        OffsetSize::U32
                    }
                };
            }
            ("sized", Some((_, span))) => {
                errors.push(Diagnostic::new("`sized` takes no value", span));
            }
            ("version", None) => errors.push(
                Diagnostic::new("`version` needs a value", attribute.name.span)
                    .with_help("such as `version: 1`"),
            ),
            ("offset_size", None) => errors.push(
                Diagnostic::new("`offset_size` needs a value", attribute.name.span)
                    .with_help("such as `offset_size: 16`"),
            ),
            (other, _) => errors.push(
                Diagnostic::new(format!("unknown attribute `{other}`"), attribute.name.span)
                    .with_help("expected `sized`, `version` or `offset_size`"),
            ),
        }
    }

    // Checked once the offset size is known, whichever attribute comes first
    if let Some((value, span)) = version {
        let max = layout.offset_size.max_value();
        match u32::try_from(value) {
            Ok(value) if u64::from(value) <= max => layout.version = Some(value),
            _ => errors.push(
                Diagnostic::new(
                    format!(
                        "version {value} doesn't fit in {}-bit offsets",
                        layout.offset_size.bytes() * 8
                    ),
                    span,
                )
                .with_help(format!("the largest is {max}")),
            ),
        }
    }

    layout
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
        Keyword::Struct => {
            errors.push(Diagnostic::new(
                format!("`{}` is a struct, which tables can't hold yet", name.text),
                name.span,
            ));
            None
        }
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
/// every field is required and such a table would never end
fn check_cycles(
    definitions: &[Definition],
    declarations: &[Declaration],
    errors: &mut Vec<Diagnostic>,
) {
    for (start, definition) in definitions.iter().enumerate() {
        let mut stack: Vec<usize> = nested_tables(definition).collect();
        let mut seen = vec![false; definitions.len()];

        while let Some(index) = stack.pop() {
            if index == start {
                errors.push(
                    Diagnostic::new(
                        format!("`{}` contains itself", definition.name),
                        declarations[start].name.span,
                    )
                    .with_help("only a vector can nest a table in itself"),
                );
                break;
            }
            if !std::mem::replace(&mut seen[index], true) {
                stack.extend(nested_tables(&definitions[index]));
            }
        }
    }
}

/// Rejects tables nested in messages with narrower offsets, since readers
/// read a nested table's offsets as wide as its message's, while tables are
/// written with 32-bit offsets
fn check_nested_offsets(
    definitions: &[Definition],
    declarations: &[Declaration],
    errors: &mut Vec<Diagnostic>,
) {
    for (definition, declaration) in definitions.iter().zip(declarations) {
        let Kind::Message(layout) = definition.kind else {
            continue;
        };
        if layout.offset_size == OffsetSize::U32 {
            continue;
        }

        for (field, field_declaration) in definition.fields.iter().zip(&declaration.fields) {
            if matches!(field.field_type, FieldType::Table(_) | FieldType::Vector(_)) {
                errors.push(
                    Diagnostic::new(
                        format!(
                            "`{}` can't nest tables, since its offsets are {} bits wide",
                            definition.name,
                            layout.offset_size.bytes() * 8
                        ),
                        field_declaration.type_name.span,
                    )
                    .with_help("nested tables need 32-bit offsets"),
                );
            }
        }
    }
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
