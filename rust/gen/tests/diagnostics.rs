//! Each invalid schema in `tests/diagnostics` fails with the errors in the
//! `.stderr` file beside it, rendered as the CLI prints them.
//! `TAFLA_GEN_UPDATE=1` rewrites them.

mod common;

use std::path::Path;

use common::{DIAGNOSTICS, assert_no_differences, compare_file, files_with_extension, update};
use tafla_gen::{Diagnostic, parse};

#[test]
fn invalid_schemas_fail_with_the_expected_errors() {
    let mut differences = Vec::new();

    for (name, path) in files_with_extension(Path::new(DIAGNOSTICS), "tafla") {
        let source = std::fs::read_to_string(&path).unwrap();
        let file_name = format!("{name}.tafla");

        match parse(&source) {
            Ok(_) => differences.push(format!("{file_name} has no errors")),
            Err(diagnostics) => {
                let rendered = Diagnostic::render_all(&diagnostics, &file_name, &source);
                let expected = path.with_extension("stderr");
                differences.extend(compare_file(&expected, &rendered));
            }
        }
    }

    assert_no_differences(&differences);
}

#[test]
fn every_expected_output_has_a_schema() {
    let directory = Path::new(DIAGNOSTICS);
    let schemas = files_with_extension(directory, "tafla");
    let stale: Vec<_> = files_with_extension(directory, "stderr")
        .into_iter()
        .filter(|(name, _)| !schemas.iter().any(|(schema, _)| schema == name))
        .map(|(_, path)| path)
        .collect();

    if update() {
        for path in &stale {
            std::fs::remove_file(path).unwrap();
        }
        return;
    }
    assert!(stale.is_empty(), "outputs without a schema: {stale:?}");
}

#[test]
fn a_syntax_error_is_the_only_error() {
    // The duplicate field would be an error too, but the checks don't run
    let source = "message Point { x: float64; x: float64; y float64; }";

    let diagnostics = parse(source).unwrap_err();

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "expected `:`, found `float64`");
}

#[test]
fn errors_are_in_source_order() {
    let source = "message B { x: Unknown; }\ntable a { y: float64; }\ntable C { c: C; }";

    let diagnostics = parse(source).unwrap_err();
    let starts: Vec<usize> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.span.start)
        .collect();

    let mut sorted = starts.clone();
    sorted.sort_unstable();
    assert_eq!(starts, sorted);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
}

#[test]
fn cycles_and_nested_offsets_are_checked_once_the_rest_is_valid() {
    // A cycle is only reported once every type resolves
    let source = "table Node { next: Node; other: Missing; }";

    let diagnostics = parse(source).unwrap_err();

    let messages: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(messages, ["unknown type `Missing`"]);
}
