//! Files that the tests compare the generator's output with: the example
//! schemas at the root of the repository, their generated code in
//! `tests/golden` and the invalid schemas in `tests/diagnostics`.
//!
//! Setting `BUFFY_GEN_UPDATE=1` rewrites the expected files from what the
//! generator writes, rather than comparing them.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use buffy_gen::{GeneratedFile, Target};

/// The root of the repository, where the generator's tests run the CLI so
/// that it names the example schemas as `schemas/<name>.buffy`
pub const REPOSITORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

pub const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden");

pub const DIAGNOSTICS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/diagnostics");

pub fn update() -> bool {
    std::env::var_os("BUFFY_GEN_UPDATE").is_some_and(|value| value == "1")
}

/// An example schema at the root of the repository
pub struct Schema {
    /// The file name without its extension
    pub name: String,
    /// The path from the root of the repository, which generated files name
    pub file_name: String,
    pub source: String,
}

pub fn example_schemas() -> Vec<Schema> {
    let directory = Path::new(REPOSITORY).join("schemas");
    files_with_extension(&directory, "buffy")
        .into_iter()
        .map(|(name, path)| Schema {
            file_name: format!("schemas/{name}.buffy"),
            source: read(&path),
            name,
        })
        .collect()
}

/// The targets that this build generates, with their full names
pub fn targets() -> Vec<(&'static str, Target)> {
    Target::names()
        .map(|name| (name, Target::from_name(name).unwrap()))
        .collect()
}

/// The files with `extension` in `directory`, by name without the extension
pub fn files_with_extension(directory: &Path, extension: &str) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, PathBuf)> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|found| found == extension))
        .map(|path| (path.file_stem().unwrap().to_str().unwrap().to_owned(), path))
        .collect();
    files.sort();
    files
}

/// Every file under `directory`, by its path from there with `/`
/// separators, or nothing if the directory doesn't exist
pub fn read_tree(directory: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    if directory.exists() {
        read_tree_into(directory, "", &mut files);
    }
    files
}

fn read_tree_into(directory: &Path, prefix: &str, files: &mut BTreeMap<String, String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        let relative = format!("{prefix}{name}");

        if path.is_dir() {
            read_tree_into(&path, &format!("{relative}/"), files);
        } else {
            files.insert(relative, read(&path));
        }
    }
}

/// Compares generated files with the files under `directory`, or replaces
/// those with them when updating. Returns a description of each difference.
pub fn compare_tree(directory: &Path, files: &[GeneratedFile]) -> Vec<String> {
    let generated: BTreeMap<String, String> = files
        .iter()
        .map(|file| (file.path.clone(), file.contents.clone()))
        .collect();

    if update() {
        if directory.exists() {
            fs::remove_dir_all(directory).unwrap();
        }
        for (path, contents) in &generated {
            write(&directory.join(path), contents);
        }
        return Vec::new();
    }

    let expected = read_tree(directory);
    let mut differences = Vec::new();
    for (path, contents) in &generated {
        match expected.get(path) {
            None => differences.push(format!("{}/{path} is missing", directory.display())),
            Some(expected) => {
                if let Some(difference) = first_difference(expected, contents) {
                    differences.push(format!("{}/{path}: {difference}", directory.display()));
                }
            }
        }
    }
    for path in expected.keys() {
        if !generated.contains_key(path) {
            differences.push(format!(
                "{}/{path} isn't generated any more",
                directory.display()
            ));
        }
    }

    differences
}

/// Compares a file with what was expected, or replaces it when updating
pub fn compare_file(path: &Path, actual: &str) -> Option<String> {
    if update() {
        write(path, actual);
        return None;
    }

    let expected = fs::read_to_string(path).unwrap_or_default();
    first_difference(&expected, actual)
        .map(|difference| format!("{}: {difference}", path.display()))
}

/// The first line where `actual` differs from `expected`, if it does
pub fn first_difference(expected: &str, actual: &str) -> Option<String> {
    if expected == actual {
        return None;
    }

    let mut expected_lines = expected.split('\n');
    let mut actual_lines = actual.split('\n');
    for line in 1.. {
        let (expected_line, actual_line) = (expected_lines.next(), actual_lines.next());
        if expected_line != actual_line {
            return Some(format!(
                "line {line} is {actual_line:?} rather than {expected_line:?}"
            ));
        }
    }
    unreachable!("the strings differ")
}

/// Fails with every difference, and how to accept them
pub fn assert_no_differences(differences: &[String]) {
    assert!(
        differences.is_empty(),
        "{}\n\nRun the tests with BUFFY_GEN_UPDATE=1 to accept the generator's output",
        differences.join("\n")
    );
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
