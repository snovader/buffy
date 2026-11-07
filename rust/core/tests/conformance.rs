//! Reads and rebuilds the tables of tafla's conformance suite, which the
//! TypeScript runtime writes. See `conformance/README.md` at the root of the
//! repository for the format of their descriptions in `cases.json`.

mod common;

use std::{fs, path::Path};

use common::{
    cases::table_of,
    description::Table,
    read::{ReadError, read_root_field, read_table},
};
use serde_json::Value;

const CONFORMANCE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance");

struct ValidCase {
    name: String,
    bytes: Vec<u8>,
    table: Table,
}

fn suite() -> Value {
    let json = fs::read_to_string(Path::new(CONFORMANCE).join("cases.json")).unwrap();
    serde_json::from_str(&json).unwrap()
}

fn read_file(case: &Value) -> Vec<u8> {
    fs::read(Path::new(CONFORMANCE).join(case["file"].as_str().unwrap())).unwrap()
}

fn valid_cases(suite: &Value) -> Vec<ValidCase> {
    let cases = suite["valid"].as_array().unwrap();

    cases
        .iter()
        .map(|case| ValidCase {
            name: case["name"].as_str().unwrap().to_owned(),
            bytes: read_file(case),
            table: table_of(&case["table"]),
        })
        .collect()
}

/// Returns the description of the valid case that an equivalent or invalid
/// case names as its `case`
fn table_of_case<'a>(valid: &'a [ValidCase], case: &Value) -> &'a Table {
    let name = case["case"].as_str().unwrap();
    let valid = valid.iter().find(|valid| valid.name == name);
    let valid = valid.unwrap_or_else(|| panic!("There's no valid case {name}"));

    &valid.table
}

#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_reads_every_valid_table() {
    for case in valid_cases(&suite()) {
        assert_eq!(
            read_table(&case.bytes, &case.table),
            Ok(case.table),
            "{}",
            case.name
        );
    }
}

#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_rebuilds_every_valid_table_byte_for_byte() {
    for case in valid_cases(&suite()) {
        assert_eq!(case.table.to_bytes(), Ok(case.bytes), "{}", case.name);
    }
}

/// Reads every equivalent table, whose bytes differ from its valid case's,
/// as the valid case's description, header included
#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_reads_equivalent_tables_as_their_valid_case() {
    let suite = suite();
    let valid = valid_cases(&suite);

    for case in suite["equivalent"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let table = table_of_case(&valid, case);

        assert_eq!(
            read_table(&read_file(case), table).as_ref(),
            Ok(table),
            "{name}"
        );
    }
}

/// Reads the root fields before the invalid case's `field`, which must hold
/// their described values, and then `field`, which must fail with a decode
/// error
#[test]
#[cfg_attr(miri, ignore = "reads files, which Miri isolates")]
fn test_reads_invalid_tables_up_to_the_field_that_fails() {
    let suite = suite();
    let valid = valid_cases(&suite);

    for case in suite["invalid"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let field = usize::try_from(case["field"].as_u64().unwrap()).unwrap();
        let table = table_of_case(&valid, case);
        let bytes = read_file(case);

        for (index, expected) in table.fields[..field].iter().enumerate() {
            let read = read_root_field(&bytes, table.layout, index, expected);
            assert_eq!(read.as_ref(), Ok(expected), "{name}/{index}");
        }

        let read = read_root_field(&bytes, table.layout, field, &table.fields[field]);
        assert!(
            matches!(read, Err(ReadError::Decode(_))),
            "{name}/{field}: {read:?}"
        );
    }
}
