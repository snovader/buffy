//! Properties of the generator over arbitrary input: parsing never panics,
//! every schema that the checker accepts generates code for every target, its
//! files stay in the output directory, and the CLI writes what the library
//! generates.

mod common;

use std::fmt::Write;
use std::process::Command;

use buffy_gen::{Diagnostic, GeneratedFile, Options, compile, parse};
use common::targets;
use proptest::prelude::*;
use proptest::sample::{Index, subsequence};

const SCALARS: [&str; 10] = [
    "int8", "uint8", "int16", "uint16", "int32", "uint32", "int64", "uint64", "float32", "float64",
];

/// Field names, including keywords of the targets and the names of the
/// generated code's own locals and members
const FIELD_NAMES: [&str; 24] = [
    "value", "x", "name", "values", "items", "class", "default", "in", "match", "self", "type",
    "async", "yield", "null", "crate", "offsets", "i", "buffer", "position", "bytes", "version",
    "fields", "size", "count",
];

/// The tokens of the schema language and some that aren't, for input that
/// gets further into the parser and the checker than arbitrary text does
const TOKENS: [&str; 38] = [
    "message",
    "table",
    "struct",
    "Point",
    "Pair",
    "Node",
    "point",
    "x",
    "y",
    "next",
    "float64",
    "uint8",
    "string",
    "flaot64",
    "[",
    "]",
    "{",
    "}",
    "(",
    ")",
    ":",
    ";",
    ",",
    "sized",
    "version",
    "offset_size",
    "8",
    "16",
    "32",
    "4294967296",
    "/// doc\n",
    "// comment\n",
    "\n",
    " ",
    "é",
    "@",
    "/",
    "🦀",
];

proptest! {
    #[test]
    fn arbitrary_text_parses_or_fails_with_diagnostics(source in any::<String>()) {
        check_parse(&source)?;
    }

    #[test]
    fn arbitrary_tokens_parse_or_fail_with_diagnostics(
        tokens in prop::collection::vec(prop::sample::select(&TOKENS[..]), 0..60)
    ) {
        check_parse(&tokens.concat())?;
    }

    #[test]
    fn every_valid_schema_generates_code_for_every_target(schema in schema()) {
        let source = schema.render();
        parse(&source).map_err(|diagnostics| {
            TestCaseError::fail(Diagnostic::render_all(&diagnostics, "schema.buffy", &source))
        })?;

        for (name, target) in targets() {
            let files = compile("schemas/schema.buffy", &source, target, &Options::default())
                .unwrap();
            let again = compile("schemas/schema.buffy", &source, target, &Options::default())
                .unwrap();

            // Java has a file per class, and the others one per schema
            let expected_files = if name == "java" { schema.definitions.len() } else { 1 };
            prop_assert_eq!(files.len(), expected_files);
            prop_assert_eq!(&files, &again, "{} isn't deterministic", name);
            for file in &files {
                check_file(file)?;
            }
        }
    }

    #[test]
    fn generated_files_stay_in_the_output_directory(
        file_name in r"[ab./\\:]{0,10}",
        package in r"[ab1_$./\\:-]{0,10}",
    ) {
        // Most names aren't Java packages, and generate in the default one
        let options = Options { runtime: None, package: package.parse().ok() };

        for (_, target) in targets() {
            let files = compile(&file_name, "message Point { x: uint8; }", target, &options);
            for file in files.unwrap() {
                check_path(&file.path)?;
            }
        }
    }
}

proptest! {
    // Each case runs the CLI once per target
    #![proptest_config(ProptestConfig::with_cases(24))]

    #[test]
    fn the_cli_writes_what_the_library_generates(
        schema in schema(),
        runtime in prop::option::of(prop::sample::select(&["runtime", "./runtime.js"][..])),
        package in prop::option::of(prop::sample::select(&["com.example.tables", "tables"][..])),
    ) {
        let options = Options {
            runtime: runtime.map(str::to_owned),
            package: package.map(|name| name.parse().unwrap()),
        };
        check_cli(&schema.render(), &options)?;
    }

    #[test]
    fn the_cli_prints_the_errors_that_the_library_finds(
        tokens in prop::collection::vec(prop::sample::select(&TOKENS[..]), 1..40)
    ) {
        check_cli(&tokens.concat(), &Options::default())?;
    }
}

fn check_parse(source: &str) -> Result<(), TestCaseError> {
    let Err(diagnostics) = parse(source) else {
        return Ok(());
    };

    prop_assert!(!diagnostics.is_empty());
    for diagnostic in &diagnostics {
        let span = diagnostic.span;
        prop_assert!(span.start <= span.end && span.end <= source.len());
        prop_assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
        prop_assert!(
            diagnostic
                .render("schema.buffy", source)
                .starts_with("error: ")
        );
    }

    Ok(())
}

/// A generated file has a path within the output directory and ends with one
/// line break
fn check_file(file: &GeneratedFile) -> Result<(), TestCaseError> {
    check_path(&file.path)?;
    prop_assert!(
        file.contents
            .contains("Generated by buffy-gen from schemas/schema.buffy")
    );
    prop_assert!(file.contents.ends_with('\n') && !file.contents.ends_with("\n\n"));
    Ok(())
}

/// A path within the output directory on any system: names separated by `/`,
/// none of them empty, `.` or `..`, and none holding Windows' `\` or a
/// drive's `:`
fn check_path(path: &str) -> Result<(), TestCaseError> {
    for name in path.split('/') {
        prop_assert!(!matches!(name, "" | "." | ".."), "{}", path);
        prop_assert!(!name.contains(['\\', ':']), "{}", path);
    }
    Ok(())
}

/// Runs the CLI on `source` for every target, and compares what it writes and
/// prints with what the library generates or finds
fn check_cli(source: &str, options: &Options) -> Result<(), TestCaseError> {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("schema.buffy"), source).unwrap();

    for (name, target) in targets() {
        let out = directory.path().join(name);
        let output = run_cli(directory.path(), name, &out, options);
        let stderr = String::from_utf8(output.stderr).unwrap();

        match compile("schema.buffy", source, target, options) {
            Ok(files) => {
                prop_assert!(output.status.success(), "{}", stderr);
                prop_assert_eq!(common::read_tree(&out), tree(&files));
            }
            Err(diagnostics) => {
                prop_assert!(!output.status.success());
                prop_assert_eq!(
                    stderr,
                    Diagnostic::render_all(&diagnostics, "schema.buffy", source)
                );
                prop_assert!(!out.exists());
            }
        }
    }

    Ok(())
}

fn run_cli(
    directory: &std::path::Path,
    target: &str,
    out: &std::path::Path,
    options: &Options,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_buffy-gen"));
    command
        .current_dir(directory)
        .args([target, "schema.buffy", "--out"])
        .arg(out);
    if let Some(runtime) = &options.runtime {
        command.args(["--runtime", runtime]);
    }
    if let Some(package) = &options.package {
        command.args(["--package", package.as_str()]);
    }
    command.output().unwrap()
}

fn tree(files: &[GeneratedFile]) -> std::collections::BTreeMap<String, String> {
    files
        .iter()
        .map(|file| (file.path.clone(), file.contents.clone()))
        .collect()
}

/// A schema that the checker accepts, by construction: unique names, tables
/// nested only in definitions before them, so that none contains itself,
/// tables only in messages of 32-bit offsets, structs of scalars only, and
/// versions that fit in their offsets
#[derive(Debug)]
struct Schema {
    definitions: Vec<Definition>,
}

#[derive(Debug)]
struct Definition {
    kind: Kind,
    docs: Vec<String>,
    fields: Vec<Field>,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Message {
        sized: bool,
        /// Scaled to the largest version that the offsets hold
        version: Option<Index>,
        offset_bits: u8,
        /// Whether the attributes end with a comma
        trailing_comma: bool,
    },
    Table,
    Struct,
}

#[derive(Debug)]
struct Field {
    name: &'static str,
    docs: Vec<String>,
    of_type: FieldType,
}

#[derive(Clone, Copy, Debug)]
enum FieldType {
    Scalar(Index),
    Array(Index),
    String,
    /// One of the tables after the definition, or a scalar if there's none
    Table(Index),
    /// Any of the tables, or a string if there's none
    Vector(Index),
}

fn schema() -> impl Strategy<Value = Schema> {
    prop::collection::vec(definition(), 0..6).prop_map(|definitions| Schema { definitions })
}

fn definition() -> impl Strategy<Value = Definition> {
    let kind = prop_oneof![
        (
            any::<bool>(),
            prop::option::of(any::<Index>()),
            prop::sample::select(&[8_u8, 16, 32][..]),
            any::<bool>(),
        )
            .prop_map(
                |(sized, version, offset_bits, trailing_comma)| Kind::Message {
                    sized,
                    version,
                    offset_bits,
                    trailing_comma,
                }
            ),
        Just(Kind::Table),
        Just(Kind::Struct),
    ];
    let fields = subsequence(&FIELD_NAMES[..], 1..6)
        .prop_shuffle()
        .prop_flat_map(|names| {
            let count = names.len();
            (
                Just(names),
                prop::collection::vec((docs(), field_type()), count),
            )
        })
        .prop_map(|(names, fields)| {
            names
                .into_iter()
                .zip(fields)
                .map(|(name, (docs, of_type))| Field {
                    name,
                    docs,
                    of_type,
                })
                .collect()
        });

    (kind, docs(), fields).prop_map(|(kind, docs, fields)| Definition { kind, docs, fields })
}

fn field_type() -> impl Strategy<Value = FieldType> {
    prop_oneof![
        any::<Index>().prop_map(FieldType::Scalar),
        any::<Index>().prop_map(FieldType::Array),
        Just(FieldType::String),
        any::<Index>().prop_map(FieldType::Table),
        any::<Index>().prop_map(FieldType::Vector),
    ]
}

/// Lines of any text, including what ends a comment or string in some
/// target language
fn docs() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[^\n]{0,30}", 0..3)
}

impl Schema {
    fn render(&self) -> String {
        let tables: Vec<usize> = (0..self.definitions.len())
            .filter(|&index| matches!(self.definitions[index].kind, Kind::Table))
            .collect();

        let mut source = String::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            for line in &definition.docs {
                writeln!(source, "///{line}").unwrap();
            }
            match definition.kind {
                Kind::Message {
                    sized,
                    version,
                    offset_bits,
                    trailing_comma,
                } => {
                    let mut attributes = Vec::new();
                    if sized {
                        attributes.push("sized".to_owned());
                    }
                    if let Some(version) = version {
                        let max = (1_u64 << offset_bits) - 1;
                        let version =
                            version.index(usize::try_from(max).unwrap().saturating_add(1));
                        attributes.push(format!("version: {version}"));
                    }
                    attributes.push(format!("offset_size: {offset_bits}"));
                    let comma = if trailing_comma { "," } else { "" };
                    writeln!(
                        source,
                        "message {} ({}{comma}) {{",
                        name(index),
                        attributes.join(", ")
                    )
                    .unwrap();
                }
                Kind::Table => writeln!(source, "table {} {{", name(index)).unwrap(),
                Kind::Struct => writeln!(source, "struct {} {{", name(index)).unwrap(),
            }

            for field in &definition.fields {
                for line in &field.docs {
                    writeln!(source, "    /// {line}").unwrap();
                }
                let field_type = self.resolve(index, field.of_type, &tables);
                writeln!(source, "    {}: {field_type};", field.name).unwrap();
            }
            source.push_str("}\n\n");
        }

        source
    }

    /// The type of a field, as the definition at `index` can hold it
    fn resolve(&self, index: usize, field_type: FieldType, tables: &[usize]) -> String {
        let kind = self.definitions[index].kind;
        let holds_tables = match kind {
            Kind::Message { offset_bits, .. } => offset_bits == 32,
            Kind::Table => true,
            Kind::Struct => false,
        };
        let later_tables: Vec<usize> = tables
            .iter()
            .copied()
            .filter(|&table| table > index)
            .collect();

        match field_type {
            FieldType::Scalar(scalar) => SCALARS[scalar.index(SCALARS.len())].to_owned(),
            _ if matches!(kind, Kind::Struct) => "uint8".to_owned(),
            FieldType::Array(scalar) => format!("[{}]", SCALARS[scalar.index(SCALARS.len())]),
            FieldType::Table(table) if holds_tables && !later_tables.is_empty() => {
                name(later_tables[table.index(later_tables.len())])
            }
            FieldType::Vector(table) if holds_tables && !tables.is_empty() => {
                format!("[{}]", name(tables[table.index(tables.len())]))
            }
            FieldType::Table(_) => "float64".to_owned(),
            FieldType::String | FieldType::Vector(_) => "string".to_owned(),
        }
    }
}

/// Unique names, which mix letters and digits as `PascalCase` allows
fn name(index: usize) -> String {
    format!("Definition{index}X")
}
