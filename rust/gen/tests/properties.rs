//! Properties of the generator over arbitrary input: parsing never panics,
//! every schema that the checker accepts generates code for every target,
//! structs that contain themselves or grow too large and chains of tables or
//! structs that nest too deep fail with the checker's errors, its files stay
//! in the output directory, and the CLI writes what the library generates.

mod common;

use std::collections::HashSet;
use std::fmt::Write;
use std::process::Command;

use common::targets;
use proptest::prelude::*;
use proptest::sample::{Index, subsequence};
use tafla_gen::{Diagnostic, GeneratedFile, Options, compile, parse};

/// The most bytes that a struct takes
const MAX_STRUCT_SIZE: usize = 65_536;

/// The most tables, and the most structs, that nest in place one within
/// another, the outermost included
const MAX_NESTING_DEPTH: usize = 64;

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
const TOKENS: [&str; 42] = [
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
    "offset_type",
    "offset_size",
    "uint8",
    "uint16",
    "uint32",
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
            TestCaseError::fail(Diagnostic::render_all(&diagnostics, "schema.tafla", &source))
        })?;

        for (name, target) in targets() {
            let files = compile("schemas/schema.tafla", &source, target, &Options::default())
                .unwrap();
            let again = compile("schemas/schema.tafla", &source, target, &Options::default())
                .unwrap();

            // Java has a file per class, a definition's and the view of a
            // vector or an array of it, and the others one per schema
            let expected_files = if name == "java" {
                schema.definitions.len() + schema.items().len()
            } else {
                1
            };
            prop_assert_eq!(files.len(), expected_files);
            prop_assert_eq!(&files, &again, "{} isn't deterministic", name);
            for file in &files {
                check_file(file)?;
            }
        }
    }

    #[test]
    fn structs_that_contain_themselves_fail_with_diagnostics(
        schema in schema_with_struct(),
        chosen in any::<Index>(),
        back in any::<Index>(),
    ) {
        // A field that closes a cycle: from a struct to one that contains it,
        // or to itself
        let structs: Vec<usize> = schema.structs().collect();
        let start = structs[chosen.index(structs.len())];
        let containing: Vec<usize> = structs
            .iter()
            .copied()
            .filter(|&outer| schema.contains(outer, start))
            .collect();
        let end = containing[back.index(containing.len())];
        let source = schema.render_with(Some((start, &format!("cycle: {}", name(end)))));

        // Each struct on the cycle, rather than those that hold one of them
        let expected: Vec<(String, String)> = structs
            .into_iter()
            .filter(|&index| schema.contains(end, index) && schema.contains(index, start))
            .map(|index| (format!("`{}` contains itself", name(index)), name(index)))
            .collect();
        check_errors(&source, &expected)?;
    }

    #[test]
    fn structs_larger_than_65536_bytes_fail_with_diagnostics(
        schema in schema_with_struct(),
        chosen in any::<Index>(),
    ) {
        // A struct that holds one of 65,536 bytes beside its own fields,
        // which makes it and every struct that contains it too large
        let structs: Vec<usize> = schema.structs().collect();
        let large = structs[chosen.index(structs.len())];
        let mut source = schema.render_with(Some((large, "large: Large16")));
        source.push_str(&large_structs());

        // The struct that grows too large, rather than those that hold it
        let expected = [(format!("`{}` is too large for a struct", name(large)), name(large))];
        check_errors(&source, &expected)?;
    }

    #[test]
    fn structs_and_tables_nest_at_most_64_deep(chain in chain()) {
        let source = chain.render();

        if chain.depth <= MAX_NESTING_DEPTH {
            for (_, target) in targets() {
                compile("schema.tafla", &source, target, &Options::default()).map_err(
                    |diagnostics| {
                        TestCaseError::fail(Diagnostic::render_all(
                            &diagnostics,
                            "schema.tafla",
                            &source,
                        ))
                    },
                )?;
            }
        } else {
            // The link that grows too deep, rather than those that hold it
            let link = link_name(MAX_NESTING_DEPTH);
            check_errors(&source, &[(format!("`{link}` nests too deeply"), link)])?;
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
                .render("schema.tafla", source)
                .starts_with("error: ")
        );
    }

    Ok(())
}

/// Checks that a schema fails with the errors in `expected`, each a message
/// and the text that its span points at, in source order
fn check_errors(source: &str, expected: &[(String, String)]) -> Result<(), TestCaseError> {
    check_parse(source)?;

    let diagnostics = parse(source).err().unwrap_or_default();
    let errors: Vec<(String, String)> = diagnostics
        .into_iter()
        .map(|diagnostic| {
            let span = diagnostic.span;
            (diagnostic.message, source[span.start..span.end].to_owned())
        })
        .collect();
    prop_assert_eq!(errors, expected, "{}", source);
    Ok(())
}

/// A generated file has a path within the output directory and ends with one
/// line break
fn check_file(file: &GeneratedFile) -> Result<(), TestCaseError> {
    check_path(&file.path)?;
    prop_assert!(
        file.contents
            .contains("Generated by tafla-gen from schemas/schema.tafla")
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
    std::fs::write(directory.path().join("schema.tafla"), source).unwrap();

    for (name, target) in targets() {
        let out = directory.path().join(name);
        let output = run_cli(directory.path(), name, &out, options);
        let stderr = String::from_utf8(output.stderr).unwrap();

        match compile("schema.tafla", source, target, options) {
            Ok(files) => {
                prop_assert!(output.status.success(), "{}", stderr);
                prop_assert_eq!(common::read_tree(&out), tree(&files));
            }
            Err(diagnostics) => {
                prop_assert!(!output.status.success());
                prop_assert_eq!(
                    stderr,
                    Diagnostic::render_all(&diagnostics, "schema.tafla", source)
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
    let mut command = Command::new(env!("CARGO_BIN_EXE_tafla-gen"));
    command
        .current_dir(directory)
        .args([target, "schema.tafla", "--out"])
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
/// tables only in messages of `uint32` offsets, structs of scalars and of the
/// structs after them, which can share a struct, and versions that fit in
/// their offset type. A struct of `n` fields nests only structs of at most
/// 65,536 / `n` bytes, so that it takes at most 65,536 itself. Of at most 11
/// definitions, tables and structs nest at most 11 deep.
#[derive(Debug)]
struct Schema {
    definitions: Vec<Definition>,
    /// The bytes that each struct takes, by its index, and 0 for the others
    struct_sizes: Vec<usize>,
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
    /// Any of the structs, or in a struct one of the structs after it, or a
    /// scalar if there's none
    Struct(Index),
    /// Any of the structs, or an array of scalars if there's none
    StructArray(Index),
    /// One of the tables after the definition, or a scalar if there's none
    Table(Index),
    /// Any of the tables, or a string if there's none
    Vector(Index),
}

fn schema() -> impl Strategy<Value = Schema> {
    prop::collection::vec(definition(), 0..12).prop_map(Schema::new)
}

/// A schema with at least one struct, for a property to break
fn schema_with_struct() -> impl Strategy<Value = Schema> {
    schema().prop_filter("a schema without structs", |schema| {
        schema.structs().next().is_some()
    })
}

fn definition() -> impl Strategy<Value = Definition> {
    // As many structs as messages and tables, so that schemas often hold
    // several, which nest and share each other
    let kind = prop_oneof![
        1 => (
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
        1 => Just(Kind::Table),
        2 => Just(Kind::Struct),
    ];

    kind.prop_flat_map(|kind| {
        let field_type = if matches!(kind, Kind::Struct) {
            struct_field_type().boxed()
        } else {
            field_type().boxed()
        };
        (Just(kind), docs(), fields(field_type))
    })
    .prop_map(|(kind, docs, fields)| Definition { kind, docs, fields })
}

fn fields(field_type: BoxedStrategy<FieldType>) -> impl Strategy<Value = Vec<Field>> {
    subsequence(&FIELD_NAMES[..], 1..13)
        .prop_shuffle()
        .prop_flat_map(move |names| {
            let count = names.len();
            (
                Just(names),
                prop::collection::vec((docs(), field_type.clone()), count),
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
        })
}

/// The type of a message's or table's field
fn field_type() -> impl Strategy<Value = FieldType> {
    prop_oneof![
        any::<Index>().prop_map(FieldType::Scalar),
        any::<Index>().prop_map(FieldType::Array),
        Just(FieldType::String),
        any::<Index>().prop_map(FieldType::Struct),
        any::<Index>().prop_map(FieldType::StructArray),
        any::<Index>().prop_map(FieldType::Table),
        any::<Index>().prop_map(FieldType::Vector),
    ]
}

/// The type of a struct's field, which is a scalar or a struct. A struct
/// field is a scalar too in the structs without structs after them.
fn struct_field_type() -> impl Strategy<Value = FieldType> {
    prop_oneof![
        1 => any::<Index>().prop_map(FieldType::Scalar),
        2 => any::<Index>().prop_map(FieldType::Struct),
    ]
}

/// Lines of any text, including what ends a comment or string in some
/// target language
fn docs() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[^\n]{0,30}", 0..3)
}

impl Schema {
    /// The schema of `definitions`, with the size of each struct, measured
    /// from the last, since a struct nests only the structs after it
    fn new(definitions: Vec<Definition>) -> Self {
        let mut schema = Self {
            struct_sizes: vec![0; definitions.len()],
            definitions,
        };

        let structs: Vec<usize> = schema.structs().collect();
        for index in structs.into_iter().rev() {
            let size = schema.definitions[index]
                .fields
                .iter()
                .map(|field| match field.of_type {
                    FieldType::Scalar(scalar) => scalar_size(scalar),
                    FieldType::Struct(chosen) => schema
                        .nested_struct(index, chosen)
                        .map_or_else(|| scalar_size(chosen), |nested| schema.struct_sizes[nested]),
                    _ => unreachable!("structs hold scalars and structs"),
                })
                .sum();
            schema.struct_sizes[index] = size;
        }

        schema
    }

    fn render(&self) -> String {
        self.render_with(None)
    }

    /// Renders the schema with `added`, a field's declaration, after the
    /// fields of the definition at its index
    fn render_with(&self, added: Option<(usize, &str)>) -> String {
        let tables = self.tables();

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
                    attributes.push(format!("offset_type: uint{offset_bits}"));
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
            if let Some((_, field)) = added.filter(|&(at, _)| at == index) {
                writeln!(source, "    {field};").unwrap();
            }
            source.push_str("}\n\n");
        }

        source
    }

    fn tables(&self) -> Vec<usize> {
        (0..self.definitions.len())
            .filter(|&index| matches!(self.definitions[index].kind, Kind::Table))
            .collect()
    }

    /// The names of the tables that vectors hold and the structs that arrays
    /// hold, as the fields resolve
    fn items(&self) -> HashSet<String> {
        let tables = self.tables();

        self.definitions
            .iter()
            .enumerate()
            .flat_map(|(index, definition)| {
                definition
                    .fields
                    .iter()
                    .map(move |field| (index, field.of_type))
            })
            .filter_map(|(index, of_type)| {
                let field_type = self.resolve(index, of_type, &tables);
                field_type
                    .strip_prefix('[')
                    .and_then(|item| item.strip_suffix(']'))
                    .filter(|item| item.starts_with("Definition"))
                    .map(str::to_owned)
            })
            .collect()
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
            FieldType::Scalar(scalar) => scalar_name(scalar).to_owned(),
            FieldType::Array(scalar) => format!("[{}]", scalar_name(scalar)),
            FieldType::Struct(chosen) => self
                .nested_struct(index, chosen)
                .map_or_else(|| scalar_name(chosen).to_owned(), name),
            FieldType::StructArray(chosen) => {
                let element = self
                    .nested_struct(index, chosen)
                    .map_or_else(|| scalar_name(chosen).to_owned(), name);
                format!("[{element}]")
            }
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

    /// The indices of the structs
    fn structs(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.definitions.len())
            .filter(|&index| matches!(self.definitions[index].kind, Kind::Struct))
    }

    /// The struct that a field of the definition at `index` holds, if there's
    /// one: any of the structs, or in a struct one after it, so that none
    /// contains itself, of at most its share of the bytes that a struct takes
    fn nested_struct(&self, index: usize, chosen: Index) -> Option<usize> {
        let definition = &self.definitions[index];
        let in_struct = matches!(definition.kind, Kind::Struct);
        let share = MAX_STRUCT_SIZE / definition.fields.len();
        let structs: Vec<usize> = self
            .structs()
            .filter(|&other| !in_struct || (other > index && self.struct_sizes[other] <= share))
            .collect();

        (!structs.is_empty()).then(|| structs[chosen.index(structs.len())])
    }

    /// Whether the struct at `outer` is the one at `inner` or contains it,
    /// even within another struct
    fn contains(&self, outer: usize, inner: usize) -> bool {
        outer == inner
            || self.definitions[outer]
                .fields
                .iter()
                .filter_map(|field| match field.of_type {
                    FieldType::Struct(chosen) => self.nested_struct(outer, chosen),
                    _ => None,
                })
                .any(|nested| self.contains(nested, inner))
    }
}

fn scalar_name(scalar: Index) -> &'static str {
    SCALARS[scalar.index(SCALARS.len())]
}

/// The bytes of a scalar, from the bits in its name
fn scalar_size(scalar: Index) -> usize {
    let bits: usize = scalar_name(scalar)
        .trim_start_matches(char::is_alphabetic)
        .parse()
        .unwrap();
    bits / 8
}

/// Unique names, which mix letters and digits as `PascalCase` allows
fn name(index: usize) -> String {
    format!("Definition{index}X")
}

/// Structs that double in size, from `Large0` of one byte to `Large16` of
/// 65,536, the most that a struct takes
fn large_structs() -> String {
    let mut source = "struct Large0 { value: uint8; }\n".to_owned();
    for level in 1..=16 {
        let half = level - 1;
        writeln!(
            source,
            "struct Large{level} {{ left: Large{half}; right: Large{half}; }}"
        )
        .unwrap();
    }
    source
}

/// A chain of tables or of structs, each of which holds the one before it in
/// place, `depth` of them in all, declared in any order
#[derive(Debug)]
struct Chain {
    is_struct: bool,
    depth: usize,
    /// How many fields each link has besides its scalar: a table's each hold
    /// the link before it, and a struct's all but the first are scalars,
    /// since each struct that held the one before it twice would double
    width: usize,
    /// The order of the links' declarations, by their depth less one
    order: Vec<usize>,
}

fn chain() -> impl Strategy<Value = Chain> {
    // Depths around the most that nest, as often as any others
    let depth = prop_oneof![
        1..=MAX_NESTING_DEPTH + 16,
        MAX_NESTING_DEPTH - 2..=MAX_NESTING_DEPTH + 2,
    ];

    (any::<bool>(), depth, 1..=4_usize).prop_flat_map(|(is_struct, depth, width)| {
        Just((0..depth).collect::<Vec<usize>>())
            .prop_shuffle()
            .prop_map(move |order| Chain {
                is_struct,
                depth,
                width,
                order,
            })
    })
}

impl Chain {
    fn render(&self) -> String {
        let keyword = if self.is_struct { "struct" } else { "table" };

        let mut source = String::new();
        for &link in &self.order {
            let mut fields = vec!["value: uint8".to_owned()];
            for field in 0..self.width {
                let nests = link > 0 && (field == 0 || !self.is_struct);
                fields.push(if nests {
                    format!("inner_{field}: {}", link_name(link - 1))
                } else {
                    format!("value_{field}: float64")
                });
            }
            writeln!(
                source,
                "{keyword} {} {{ {}; }}",
                link_name(link),
                fields.join("; ")
            )
            .unwrap();
        }

        source
    }
}

/// The name of the link that nests `index` others
fn link_name(index: usize) -> String {
    format!("Link{index}")
}
