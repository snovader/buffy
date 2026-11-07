//! Parses and checks arbitrary text as a schema with tafla-gen, and generates
//! every target's code from each schema that it accepts.
//!
//! The generator also runs as WebAssembly, where a panic aborts its host, so
//! any panic is a finding, as is a hang or an allocation that libFuzzer's
//! limits catch. A schema that fails must fail with diagnostics that point
//! into it, and a generated file must lie within the output directory.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tafla_gen::{Diagnostic, GeneratedFile, Options, Target, parse};

const FILE_NAME: &str = "schemas/fuzz.tafla";

fuzz_target!(|bytes: &[u8]| {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return;
    };

    match parse(source) {
        Ok(_) => generate(source),
        Err(diagnostics) => check_diagnostics(source, &diagnostics),
    }
});

/// Generates every target's code, each of which the parser and checker
/// accept, as they did when parsing alone
fn generate(source: &str) {
    let options = Options::default();

    for name in Target::names() {
        let target = Target::from_name(name).expect("Every target has a name");
        let files = tafla_gen::compile(FILE_NAME, source, target, &options)
            .expect("A schema that parses compiles");

        files.iter().for_each(check_file);
    }
}

fn check_diagnostics(source: &str, diagnostics: &[Diagnostic]) {
    assert_ne!(diagnostics, []);

    for diagnostic in diagnostics {
        let span = diagnostic.span;
        assert!(span.start <= span.end && span.end <= source.len());
        assert!(source.is_char_boundary(span.start) && source.is_char_boundary(span.end));
        assert!(diagnostic.render(FILE_NAME, source).starts_with("error: "));
    }
}

/// A generated file names its schema, ends with one line break, and has a
/// path within the output directory on any system
fn check_file(file: &GeneratedFile) {
    assert!(file.contents.contains(FILE_NAME));
    assert!(file.contents.ends_with('\n') && !file.contents.ends_with("\n\n"));

    for name in file.path.split('/') {
        assert!(!matches!(name, "" | "." | ".."), "{}", file.path);
        assert!(!name.contains(['\\', ':']), "{}", file.path);
    }
}
