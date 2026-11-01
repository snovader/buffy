//! The code generated from each example schema in `schemas/`, for each
//! target, matches the files checked in under
//! `tests/golden/<schema>/<target>/`. `BUFFY_GEN_UPDATE=1` rewrites them.

mod common;

use std::fs;
use std::path::Path;

use buffy_gen::{Options, compile};
use common::{GOLDEN, assert_no_differences, compare_tree, example_schemas, targets, update};

/// Every target's directory, including those of targets that a build without
/// their features doesn't generate
const TARGETS: [&str; 5] = ["typescript", "javascript", "python", "java", "rust"];

#[test]
fn generated_code_matches_the_golden_files() {
    let mut differences = Vec::new();

    for schema in example_schemas() {
        for (target_name, target) in targets() {
            let files = compile(
                &schema.file_name,
                &schema.source,
                target,
                &Options::default(),
            )
            .unwrap_or_else(|_| panic!("{} has errors", schema.file_name));
            let directory = Path::new(GOLDEN).join(&schema.name).join(target_name);

            differences.extend(compare_tree(&directory, &files));
        }
    }

    assert_no_differences(&differences);
}

#[test]
fn every_golden_directory_has_a_schema_and_a_target() {
    let schemas: Vec<String> = example_schemas()
        .into_iter()
        .map(|schema| schema.name)
        .collect();
    let mut stale = Vec::new();

    for schema in fs::read_dir(GOLDEN).unwrap() {
        let schema = schema.unwrap().path();
        let schema_name = schema.file_name().unwrap().to_str().unwrap().to_owned();
        if !schemas.contains(&schema_name) {
            stale.push(schema);
            continue;
        }

        for target in fs::read_dir(&schema).unwrap() {
            let target = target.unwrap().path();
            let target_name = target.file_name().unwrap().to_str().unwrap();
            if !TARGETS.contains(&target_name) {
                stale.push(target);
            }
        }
    }

    if update() {
        for path in &stale {
            fs::remove_dir_all(path).unwrap();
        }
        return;
    }
    assert!(stale.is_empty(), "stale golden directories: {stale:?}");
}
