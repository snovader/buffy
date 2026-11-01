//! The `buffy-gen` binary: it writes the generated files under `--out`,
//! prints errors as the library renders them, and fails on wrong usage.

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{DIAGNOSTICS, Golden, REPOSITORY, read_tree, targets};

fn buffy_gen(directory: &str, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_buffy-gen"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn writes_the_golden_files_of_an_example_schema() {
    let out = tempfile::tempdir().unwrap();

    for (name, _) in targets() {
        let golden = Golden::of("shapes", name);
        let directory = out.path().join(name);
        let mut arguments = vec![
            name,
            "schemas/shapes.buffy",
            "--out",
            directory.to_str().unwrap(),
        ];
        if let Some(package) = &golden.options.package {
            arguments.extend(["--package", package]);
        }
        let output = buffy_gen(REPOSITORY, &arguments);

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
        assert_eq!(
            read_tree(&directory.join(&golden.prefix)),
            read_tree(&golden.directory),
            "{name}"
        );
    }
}

#[test]
fn writes_every_schema_it_is_given() {
    let out = tempfile::tempdir().unwrap();

    let output = buffy_gen(
        REPOSITORY,
        &[
            "py",
            "schemas/shapes.buffy",
            "--out",
            out.path().to_str().unwrap(),
            "schemas/layouts.buffy",
        ],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let files: Vec<String> = read_tree(out.path()).into_keys().collect();
    assert_eq!(files, ["layouts.py", "shapes.py"]);
}

#[test]
fn accepts_abbreviated_targets() {
    for (abbreviation, name) in [
        ("ts", "typescript"),
        ("js", "javascript"),
        ("py", "python"),
        ("rs", "rust"),
    ] {
        let out = tempfile::tempdir().unwrap();
        let output = buffy_gen(
            REPOSITORY,
            &[
                abbreviation,
                "schemas/shapes.buffy",
                "--out",
                out.path().to_str().unwrap(),
            ],
        );

        assert!(output.status.success(), "{}", stderr(&output));
        let golden = Golden::of("shapes", name);
        assert_eq!(
            read_tree(out.path()),
            read_tree(&golden.directory),
            "{abbreviation}"
        );
    }
}

#[test]
fn prints_the_errors_of_an_invalid_schema_and_fails() {
    let out = tempfile::tempdir().unwrap();

    let output = buffy_gen(
        DIAGNOSTICS,
        &[
            "rust",
            "unknown_types.buffy",
            "--out",
            out.path().to_str().unwrap(),
        ],
    );

    assert_eq!(output.status.code(), Some(1));
    let expected = std::fs::read_to_string(Path::new(DIAGNOSTICS).join("unknown_types.stderr"));
    assert_eq!(stderr(&output), expected.unwrap());
    assert!(read_tree(out.path()).is_empty());
}

#[test]
fn writes_the_valid_schemas_and_prints_the_errors_of_the_others() {
    let out = tempfile::tempdir().unwrap();
    let diagnostics = Path::new(DIAGNOSTICS);
    let schemas = [
        diagnostics.join("cycles.buffy"),
        Path::new(REPOSITORY).join("schemas/shapes.buffy"),
        diagnostics.join("naming.buffy"),
    ];

    let mut arguments = vec!["rust", "--out", out.path().to_str().unwrap()];
    arguments.extend(schemas.iter().map(|path| path.to_str().unwrap()));
    let output = buffy_gen(REPOSITORY, &arguments);

    assert_eq!(output.status.code(), Some(1));
    // Each schema's errors name it by its path, after a blank line
    let stderr = stderr(&output);
    assert!(stderr.contains("cycles.buffy:3:7\n"), "{stderr}");
    assert!(
        stderr.contains("in itself\n\nerror: `point` should be PascalCase\n"),
        "{stderr}"
    );
    let files: Vec<String> = read_tree(out.path()).into_keys().collect();
    assert_eq!(files, ["shapes.rs"]);
}

#[test]
fn prints_its_usage_without_a_target_or_a_schema() {
    for arguments in [&[][..], &["--help"], &["go", "schema.buffy"], &["rust"]] {
        let output = buffy_gen(REPOSITORY, arguments);

        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            stderr(&output),
            "usage: buffy-gen <typescript|javascript|python|java|rust> <schema>... \
             [--out <dir>] [--runtime <module>] [--package <package>]\n",
            "{arguments:?}"
        );
    }
}

#[test]
fn reports_a_schema_it_cannot_read() {
    let output = buffy_gen(REPOSITORY, &["rust", "schemas/missing.buffy"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("schemas/missing.buffy: "));
}

#[test]
fn reports_an_option_without_a_value() {
    let output = buffy_gen(REPOSITORY, &["rust", "schemas/shapes.buffy", "--out"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "--out needs a value\n");
}
